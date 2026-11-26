#!/usr/bin/env python3
"""THROWAWAY PROTOTYPE (#20, step 1): mirrors this PC's screen to the
prototype app on the tablet (apps/android/mirrorproto) over USB, and
measures latency, smoothness and bitrate. Linux (GNOME Wayland) only:
capture through the ScreenCast portal (PipeWire), encoding with GStreamer.

    adb reverse tcp:47390 tcp:47390
    bash -c 'source scripts/lib/clean-snap-env.sh; python3 -I prototypes/mirror/sender.py \\
        --encoder va --size 1920x1200 --fps 60 --kbps 12000 --seconds 30 --csv out.csv'

--source test uses a moving test pattern instead of the screen (no portal).
--source virtual asks the portal for a new virtual monitor (extended desktop).

Stream to the tablet: "PMP1", u8 codec (0 = H.264, 1 = VP8), u32 width,
u32 height; frames: u32 length, u64 capture time (µs, CLOCK_MONOTONIC), u8
key, bytes. From the tablet: 'A', u64 capture time, i32 decode µs.
"""

import argparse
import collections
import csv
import os
import socket
import statistics
import struct
import threading
import time

import gi

gi.require_version("Gst", "1.0")
from gi.repository import Gio, GLib, Gst  # noqa: E402

PORT = 47390


def portal_stream(source, token_file):
    """Asks the ScreenCast portal for a stream; returns (fd, node, session)."""
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    sender = bus.get_unique_name()[1:].replace(".", "_")
    loop = GLib.MainLoop()
    counter = [0]

    def request(method, args, options):
        counter[0] += 1
        token = f"padboardproto{os.getpid()}_{counter[0]}"
        path = f"/org/freedesktop/portal/desktop/request/{sender}/{token}"
        result = {}

        def on_response(_conn, _sender, _path, _iface, _signal, params):
            code, results = params.unpack()
            result["code"] = code
            result["results"] = results
            loop.quit()

        sub = bus.signal_subscribe(
            "org.freedesktop.portal.Desktop",
            "org.freedesktop.portal.Request",
            "Response",
            path,
            None,
            Gio.DBusSignalFlags.NO_MATCH_RULE,
            on_response,
        )
        options = dict(options)
        options["handle_token"] = GLib.Variant("s", token)
        bus.call_sync(
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.ScreenCast",
            method,
            GLib.Variant(f"({args[0]}a{{sv}})", (*args[1:], options)),
            None,
            Gio.DBusCallFlags.NONE,
            -1,
            None,
        )
        loop.run()
        bus.signal_unsubscribe(sub)
        if result["code"] != 0:
            raise SystemExit(f"portal {method}: refused or cancelled ({result['code']})")
        return result["results"]

    session = request(
        "CreateSession",
        ("",),
        {"session_handle_token": GLib.Variant("s", f"padboardproto{os.getpid()}")},
    )["session_handle"]
    options = {
        "types": GLib.Variant("u", 4 if source == "virtual" else 1),
        "multiple": GLib.Variant("b", False),
        "cursor_mode": GLib.Variant("u", 2),  # embedded
        "persist_mode": GLib.Variant("u", 2),
    }
    if token_file and os.path.exists(token_file) and source != "virtual":
        options["restore_token"] = GLib.Variant("s", open(token_file).read().strip())
    request("SelectSources", ("o", session), options)
    started = request("Start", ("os", session, ""), {})
    if token_file and "restore_token" in started:
        with open(token_file, "w") as f:
            f.write(started["restore_token"])
    node = started["streams"][0][0]
    reply, fds = bus.call_with_unix_fd_list_sync(
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.ScreenCast",
        "OpenPipeWireRemote",
        GLib.Variant("(oa{sv})", (session, {})),
        GLib.VariantType("(h)"),
        Gio.DBusCallFlags.NONE,
        -1,
        None,
        None,
    )
    fd = fds.get(reply.unpack()[0])
    return fd, node, (bus, session)


def close_session(handle):
    bus, session = handle
    try:
        bus.call_sync(
            "org.freedesktop.portal.Desktop",
            session,
            "org.freedesktop.portal.Session",
            "Close",
            None,
            None,
            Gio.DBusCallFlags.NONE,
            -1,
            None,
        )
    except GLib.Error:
        pass


def pipeline_description(args, width, height, fd=None, node=None):
    if args.source == "test":
        src = "videotestsrc name=src is-live=true pattern=ball motion=sweep ! timeoverlay font-desc='Sans 48'"
    else:
        src = f"pipewiresrc name=src fd={fd} path={node} do-timestamp=true keepalive-time=100 always-copy=true"
    rate = f"videorate drop-only=true ! video/x-raw,framerate={args.fps}/1"
    if args.encoder == "va":
        convert = f"vapostproc ! video/x-raw(memory:VAMemory),format=NV12,width={width},height={height}"
        enc = (
            f"vah264enc name=enc rate-control=cbr bitrate={args.kbps} key-int-max={args.fps * 2} b-frames=0 "
            "target-usage=7 ! video/x-h264,profile=main ! h264parse config-interval=-1 ! "
            "video/x-h264,stream-format=byte-stream,alignment=au"
        )
    elif args.encoder == "x264":
        convert = f"videoconvert ! videoscale ! video/x-raw,format=I420,width={width},height={height}"
        enc = (
            f"x264enc name=enc tune=zerolatency speed-preset=ultrafast bitrate={args.kbps} "
            f"key-int-max={args.fps * 2} threads=4 ! video/x-h264,profile=main ! "
            "h264parse config-interval=-1 ! video/x-h264,stream-format=byte-stream,alignment=au"
        )
    else:  # vp8
        convert = f"videoconvert ! videoscale ! video/x-raw,format=I420,width={width},height={height}"
        enc = (
            f"vp8enc name=enc deadline=1 cpu-used=16 end-usage=cbr target-bitrate={args.kbps * 1000} "
            f"keyframe-max-dist={args.fps * 2} lag-in-frames=0 threads=4 error-resilient=default"
        )
    return (
        f"{src} ! queue max-size-buffers=2 leaky=downstream ! {rate} ! {convert} ! {enc} ! "
        "appsink name=sink emit-signals=false sync=false max-buffers=4 drop=false"
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", choices=["screen", "virtual", "test"], default="screen")
    parser.add_argument("--encoder", choices=["va", "x264", "vp8"], default="va")
    parser.add_argument("--size", default="1920x1200")
    parser.add_argument("--fps", type=int, default=60)
    parser.add_argument("--kbps", type=int, default=12000)
    parser.add_argument("--seconds", type=int, default=30)
    parser.add_argument("--csv")
    parser.add_argument("--token-file")
    args = parser.parse_args()
    width, height = (int(v) for v in args.size.split("x"))

    Gst.init(None)
    server = socket.socket()
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", PORT))
    server.listen(1)
    print(f"waiting for the tablet on 127.0.0.1:{PORT} (adb reverse tcp:{PORT} tcp:{PORT})", flush=True)
    conn, _ = server.accept()
    conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    print("tablet connected", flush=True)

    session = None
    fd = node = None
    if args.source != "test":
        fd, node, session = portal_stream(args.source, args.token_file)
        print(f"portal stream: node {node}", flush=True)
    description = pipeline_description(args, width, height, fd, node)
    print(description, flush=True)
    pipeline = Gst.parse_launch(description)
    # pipewiresrc offers PipeWire's clock; the latency is measured with
    # CLOCK_MONOTONIC (time.monotonic_ns), so the pipeline runs on it.
    clock = Gst.SystemClock.obtain()
    clock.set_property("clock-type", Gst.ClockType.MONOTONIC)
    pipeline.use_clock(clock)
    sink = pipeline.get_by_name("sink")
    # Capture times in frame order, taken where the raw frame enters the
    # encoder (encoders shift their output timestamps; no B-frames, so the
    # order is kept).
    captured = collections.deque()

    def on_raw(_pad, info):
        buf = info.get_buffer()
        if buf.pts != Gst.CLOCK_TIME_NONE:
            captured.append((pipeline.get_base_time() + buf.pts) // 1000)
        else:
            captured.append(time.monotonic_ns() // 1000)
        return Gst.PadProbeReturn.OK

    pipeline.get_by_name("enc").get_static_pad("sink").add_probe(Gst.PadProbeType.BUFFER, on_raw)
    codec = 1 if args.encoder == "vp8" else 0
    conn.sendall(b"PMP1" + struct.pack(">BII", codec, width, height))

    lock = threading.Lock()
    sent = {}  # capture µs -> (send monotonic ns, bytes, encode µs)
    acks = []  # (capture µs, latency µs, decode µs, size, encode µs)
    stop = threading.Event()

    def read_acks():
        buf = b""
        while not stop.is_set():
            try:
                data = conn.recv(4096)
            except OSError:
                break
            if not data:
                break
            buf += data
            while len(buf) >= 13:
                kind, ts, decode = struct.unpack(">cqi", buf[:13])
                buf = buf[13:]
                now = time.monotonic_ns() // 1000
                with lock:
                    info = sent.pop(ts, None)
                if kind == b"A" and info:
                    acks.append((ts, now - ts, decode, info[1], info[2], now))

    reader = threading.Thread(target=read_acks, daemon=True)
    reader.start()
    pipeline.set_state(Gst.State.PLAYING)
    start = time.monotonic()
    caps_shown = False
    frames = 0
    total = 0
    sizes = []
    last = start
    try:
        while time.monotonic() - start < args.seconds:
            sample = sink.emit("try-pull-sample", 200 * Gst.MSECOND)
            if sample is None:
                continue
            buffer = sample.get_buffer()
            ok, info = buffer.map(Gst.MapFlags.READ)
            data = bytes(info.data)
            buffer.unmap(info)
            now_us = time.monotonic_ns() // 1000
            capture_us = captured.popleft() if captured else now_us
            key = 0 if buffer.has_flags(Gst.BufferFlags.DELTA_UNIT) else 1
            with lock:
                sent[capture_us] = (now_us, len(data), now_us - capture_us)
            conn.sendall(struct.pack(">IqB", len(data), capture_us, key) + data)
            frames += 1
            total += len(data)
            sizes.append(len(data))
            t = time.monotonic()
            if not caps_shown and t - start > 2:
                caps_shown = True
                source = pipeline.get_by_name("src")
                caps = source.get_static_pad("src").get_current_caps() if source else None
                print(f"source caps: {caps.to_string() if caps else None}", flush=True)
            if t - last >= 1:
                lat = sorted(a[1] / 1000 for a in acks[-frames:]) or [0]
                print(
                    f"{t - start:5.1f}s sent {frames / (t - last):5.1f} fps "
                    f"{total * 8 / (t - last) / 1e6:5.1f} Mbit/s  "
                    f"latency p50 {statistics.median(lat):5.1f} ms",
                    flush=True,
                )
                frames = 0
                total = 0
                last = t
    finally:
        pipeline.set_state(Gst.State.NULL)
        time.sleep(0.5)
        stop.set()
        conn.close()
        server.close()
        if session:
            close_session(session)

    def pct(values, p):
        values = sorted(values)
        return values[min(len(values) - 1, int(len(values) * p))] if values else float("nan")

    latencies = [a[1] / 1000 for a in acks]
    decodes = [a[2] / 1000 for a in acks if a[2] >= 0]
    encodes = [a[4] / 1000 for a in acks]
    duration = (acks[-1][5] - acks[0][5]) / 1e6 if len(acks) > 1 else 0
    gaps = [(b[5] - a[5]) / 1000 for a, b in zip(acks, acks[1:])]
    print("\n=== summary", vars(args))
    print(f"frames displayed: {len(acks)} in {duration:.1f} s = {len(acks) / duration if duration else 0:.1f} fps")
    print(f"capture->display latency ms: p50 {pct(latencies, .5):.1f}  p95 {pct(latencies, .95):.1f}  max {max(latencies, default=0):.1f}")
    print(f"encode (capture->sent) ms:  p50 {pct(encodes, .5):.1f}  p95 {pct(encodes, .95):.1f}")
    print(f"decode on tablet ms:        p50 {pct(decodes, .5):.1f}  p95 {pct(decodes, .95):.1f}")
    print(f"gaps between frames ms:     p50 {pct(gaps, .5):.1f}  p95 {pct(gaps, .95):.1f}  over 50 ms: {sum(g > 50 for g in gaps)}")
    print(f"frame size kB: p50 {pct(sizes, .5) / 1000:.1f}  max {max(sizes, default=0) / 1000:.1f}; mean bitrate {sum(sizes) * 8 / max(duration, 1e-9) / 1e6:.1f} Mbit/s")
    if args.csv:
        with open(args.csv, "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["capture_us", "latency_us", "decode_us", "bytes", "encode_us", "ack_us"])
            w.writerows(acks)


if __name__ == "__main__":
    main()
