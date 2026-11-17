#!/usr/bin/env python3
"""Gives a release candidate's .deb a Debian version that sorts before the
release: Padboard's versions are SemVer (0.2.1-rc.1), but Debian reads
what follows "-" as a package revision, so 0.2.1-rc.1 sorts after 0.2.1 and
apt would not update an rc to the release. With a tilde, 0.2.1~rc.1 sorts
before 0.2.1 (and after 0.2.0).

    scripts/deb-version.py Padboard_0.2.1-rc.1_amd64.deb   # rewritten in place
    scripts/deb-version.py --print 0.2.1-rc.1              # 0.2.1~rc.1

Tauri writes the SemVer version into the package's `control` file and has
no option for another one: only that file's `Version:` line is rewritten.
The other members (`debian-binary`, `data.tar.*`) are copied byte for byte;
the file name is kept (the same version as the other release files). A
release version (no "-") is left as it is.
"""

import gzip
import io
import lzma
import re
import sys
import tarfile

AR_MAGIC = b"!<arch>\n"


def debian_version(semver):
    """0.2.1-rc.1 -> 0.2.1~rc.1; 0.2.1 -> 0.2.1."""
    if not re.fullmatch(r"\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?", semver):
        raise ValueError(f"not a SemVer version: {semver}")
    return semver.replace("-", "~", 1)


def read_ar(data):
    """[(header without size, payload)] of an ar archive."""
    if not data.startswith(AR_MAGIC):
        raise ValueError("not a .deb (ar archive)")
    members, pos = [], len(AR_MAGIC)
    while pos < len(data):
        header = data[pos : pos + 60]
        if len(header) != 60 or header[58:60] != b"`\n":
            raise ValueError("corrupt ar header")
        size = int(header[48:58])
        payload = data[pos + 60 : pos + 60 + size]
        members.append((header[:48], payload))
        pos += 60 + size + (size % 2)
    return members


def write_ar(members):
    out = bytearray(AR_MAGIC)
    for head, payload in members:
        out += head + str(len(payload)).encode().ljust(10) + b"`\n" + payload
        if len(payload) % 2:
            out += b"\n"
    return bytes(out)


def _decompress(name, payload):
    if name.endswith(".gz"):
        return gzip.decompress(payload), lambda raw: gzip.compress(raw, mtime=0)
    if name.endswith(".xz"):
        return lzma.decompress(payload), lambda raw: lzma.compress(raw, format=lzma.FORMAT_XZ)
    if name.endswith(".tar"):
        return payload, lambda raw: raw
    raise ValueError(f"unsupported control archive: {name}")


def rewrite_control(payload, name):
    """The control archive with `Version:` made Debian-sorted; (archive, old, new)."""
    raw, compress = _decompress(name, payload)
    source = tarfile.open(fileobj=io.BytesIO(raw), mode="r:")
    out = io.BytesIO()
    target = tarfile.open(fileobj=out, mode="w:", format=source.format)
    old = new = None
    for info in source.getmembers():
        content = source.extractfile(info).read() if info.isfile() else None
        if info.isfile() and info.name.lstrip("./") == "control":
            text = content.decode("utf-8")
            match = re.search(r"^Version: (\S+)$", text, re.M)
            if not match:
                raise ValueError("no Version in control")
            old = match.group(1)
            new = debian_version(old)
            content = text.replace(f"Version: {old}\n", f"Version: {new}\n", 1).encode()
            info.size = len(content)
        target.addfile(info, io.BytesIO(content) if content is not None else None)
    target.close()
    if old is None:
        raise ValueError("no control file in the package")
    return compress(out.getvalue()), old, new


def rewrite_deb(path):
    """Rewrites the .deb at `path`; returns (old, new) versions."""
    with open(path, "rb") as f:
        data = f.read()
    members = read_ar(data)
    for i, (head, payload) in enumerate(members):
        name = head[:16].decode().strip().rstrip("/")
        if name.startswith("control.tar"):
            archive, old, new = rewrite_control(payload, name)
            if old == new:
                return old, new
            members[i] = (head, archive)
            with open(path, "wb") as f:
                f.write(write_ar(members))
            return old, new
    raise ValueError("no control archive in the package")


def main(argv):
    if len(argv) == 2 and argv[0] == "--print":
        print(debian_version(argv[1]))
        return 0
    if len(argv) != 1:
        print("usage: deb-version.py <file.deb> | --print <version>", file=sys.stderr)
        return 2
    old, new = rewrite_deb(argv[0])
    print(f"{argv[0]}: Version {old}" + ("" if old == new else f" -> {new}"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
