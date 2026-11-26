package io.github.hugoambiehl.padboard.mirrorproto

import android.app.Activity
import android.graphics.Color
import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.TextView
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong
import kotlin.concurrent.thread

/**
 * THROWAWAY PROTOTYPE (#20, step 1). Connects to 127.0.0.1:47390 (adb
 * reverse to the PC's sender, prototypes/mirror/sender.py), decodes the
 * stream with MediaCodec onto a SurfaceView, and acknowledges each frame
 * when it is handed to the display, so the PC measures the latency.
 *
 * Stream: "PMP1", u8 codec (0 = H.264, 1 = VP8), u32 width, u32 height;
 * then frames: u32 length, u64 PC timestamp (µs), u8 key, bytes.
 * Back to the PC: 'A', u64 PC timestamp, u32 µs from receipt to release
 * (decode time on the tablet).
 */
class MirrorActivity : Activity(), SurfaceHolder.Callback {
    private val tag = "MirrorProto"
    private lateinit var stats: TextView

    @Volatile private var running = false
    private var worker: Thread? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val surface = SurfaceView(this)
        surface.holder.addCallback(this)
        stats =
            TextView(this).apply {
                setTextColor(Color.YELLOW)
                setBackgroundColor(0x80000000.toInt())
                textSize = 14f
                setPadding(12, 6, 12, 6)
                text = "waiting for the PC (127.0.0.1:47390)…"
            }
        setContentView(
            FrameLayout(this).apply {
                addView(surface)
                addView(
                    stats,
                    FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.WRAP_CONTENT,
                        FrameLayout.LayoutParams.WRAP_CONTENT,
                        Gravity.TOP or Gravity.END
                    )
                )
            }
        )
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        running = true
        worker = thread(name = "mirror") { loop(holder) }
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) = Unit

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        running = false
        worker?.interrupt()
        worker?.join(2000)
    }

    private fun show(text: String) = runOnUiThread { stats.text = text }

    private fun loop(holder: SurfaceHolder) {
        while (running) {
            try {
                Socket().use { socket ->
                    socket.tcpNoDelay = true
                    socket.connect(InetSocketAddress("127.0.0.1", 47390), 1000)
                    session(socket, holder)
                }
            } catch (e: Exception) {
                if (running) {
                    show("waiting for the PC: ${e.javaClass.simpleName}")
                    Thread.sleep(1000)
                }
            }
        }
    }

    private fun session(socket: Socket, holder: SurfaceHolder) {
        val input = DataInputStream(BufferedInputStream(socket.getInputStream(), 1 shl 20))
        val output = DataOutputStream(BufferedOutputStream(socket.getOutputStream()))
        val magic = ByteArray(4).also { input.readFully(it) }
        require(String(magic) == "PMP1") { "bad magic" }
        val mime =
            when (input.readUnsignedByte()) {
                0 -> MediaFormat.MIMETYPE_VIDEO_AVC
                else -> MediaFormat.MIMETYPE_VIDEO_VP8
            }
        val width = input.readInt()
        val height = input.readInt()
        val format = MediaFormat.createVideoFormat(mime, width, height)
        if (Build.VERSION.SDK_INT >= 30) format.setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
        format.setInteger(MediaFormat.KEY_PRIORITY, 0)
        val codec = MediaCodec.createDecoderByType(mime)
        Log.i(tag, "decoder ${codec.name} for $mime ${width}x$height")
        codec.configure(format, holder.surface, null, 0)
        codec.start()

        val received = ConcurrentHashMap<Long, Long>()
        val rendered = AtomicInteger()
        val decodeMicros = AtomicLong()
        var lastReport = System.nanoTime()
        var bytes = 0L
        val outputThread =
            thread(name = "mirror-out") {
                val info = MediaCodec.BufferInfo()
                try {
                    while (running && !Thread.currentThread().isInterrupted) {
                        val index = codec.dequeueOutputBuffer(info, 10_000)
                        if (index < 0) continue
                        codec.releaseOutputBuffer(index, true)
                        val ts = info.presentationTimeUs
                        val start = received.remove(ts)
                        val micros = if (start != null) ((System.nanoTime() - start) / 1000) else -1
                        synchronized(output) {
                            output.writeByte('A'.code)
                            output.writeLong(ts)
                            output.writeInt(micros.toInt())
                            output.flush()
                        }
                        rendered.incrementAndGet()
                        if (micros >= 0) decodeMicros.addAndGet(micros)
                    }
                } catch (_: Exception) {
                }
            }
        try {
            while (running) {
                val length = input.readInt()
                val ts = input.readLong()
                input.readUnsignedByte()
                val frame = ByteArray(length).also { input.readFully(it) }
                bytes += length
                received[ts] = System.nanoTime()
                var index: Int
                do {
                    index = codec.dequeueInputBuffer(10_000)
                } while (index < 0 && running)
                if (index < 0) break
                codec.getInputBuffer(index)!!.apply {
                    clear()
                    put(frame)
                }
                codec.queueInputBuffer(index, 0, length, ts, 0)
                val now = System.nanoTime()
                if (now - lastReport > 1_000_000_000L) {
                    val seconds = (now - lastReport) / 1e9
                    val frames = rendered.getAndSet(0)
                    val fps = frames / seconds
                    val mbps = bytes * 8 / seconds / 1e6
                    val decode = if (frames > 0) decodeMicros.getAndSet(0) / frames / 1000.0 else 0.0
                    show(
                        "%s %dx%d  %.1f fps  %.1f Mbit/s  decode %.1f ms".format(
                            codec.name,
                            width,
                            height,
                            fps,
                            mbps,
                            decode
                        )
                    )
                    bytes = 0
                    lastReport = now
                }
            }
        } finally {
            outputThread.interrupt()
            outputThread.join(1000)
            runCatching { codec.stop() }
            codec.release()
        }
    }
}
