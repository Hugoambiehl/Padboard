"""Checks the signer of an APK from `apksigner verify --print-certs`.

Used by scripts/sign-release.sh:

    apksigner verify --print-certs app.apk | python3 -I scripts/lib/apk_certificate.py <sha256>

Exit code 0 only when the APK has exactly one signer and its certificate's
SHA-256 is the expected one. Both output formats are understood:

    Signer #1 certificate SHA-256 digest: <hex>          (build-tools <= 36)
    V3.0 Signer: certificate SHA-256 digest: <hex>       (build-tools 37)
    V2 Signer #1: certificate SHA-256 digest: <hex>      (build-tools 37)

Anything else that mentions a certificate digest is refused (fail closed):
a format change must be looked at, never let through. Tests:
scripts/tests/test_apk_certificate.py.
"""

import re
import sys

# The signer's label, then the digest. The label is "Signer", optionally
# preceded by the scheme ("V2", "V3.0", "V3.1") and followed by "#N".
DIGEST_LINE = re.compile(
    r"^(?P<label>(?:V\d+(?:\.\d+)? )?Signer(?: #(?P<number>\d+))?):? "
    r"certificate SHA-256 digest: (?P<digest>[0-9a-fA-F]{64})$"
)


class CertificateError(Exception):
    """The APK is not signed by exactly the expected certificate."""


def signer_digest(output: str) -> str:
    """The SHA-256 of the single signer's certificate (lowercase hex)."""
    digests = set()
    for raw in output.splitlines():
        line = raw.strip()
        if "SHA-256 digest" not in line:
            continue
        match = DIGEST_LINE.match(line)
        if not match:
            raise CertificateError(f"unknown apksigner output: {line!r}")
        if match["number"] is not None and int(match["number"]) != 1:
            raise CertificateError(f"more than one signer ({match['label']})")
        digests.add(match["digest"].lower())
    if not digests:
        raise CertificateError("no signer certificate in the apksigner output")
    if len(digests) > 1:
        raise CertificateError(f"more than one signer certificate: {sorted(digests)}")
    return digests.pop()


def check(output: str, expected: str) -> str:
    """The digest, when it is `expected`; raises otherwise."""
    digest = signer_digest(output)
    if digest != expected.lower():
        raise CertificateError(f"signed with {digest}, expected {expected.lower()}")
    return digest


def main() -> int:
    if len(sys.argv) != 2 or not re.fullmatch(r"[0-9a-fA-F]{64}", sys.argv[1]):
        print("usage: apksigner verify --print-certs X.apk | apk_certificate.py <sha256>", file=sys.stderr)
        return 2
    try:
        digest = check(sys.stdin.read(), sys.argv[1])
    except CertificateError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    print(f"APK certificate SHA-256: {digest} (one signer)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
