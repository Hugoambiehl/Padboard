"""Tests of scripts/lib/apk_certificate.py, on real apksigner outputs (an APK
signed with the Android debug key, and with a second test key).

    python3 -m unittest discover -s scripts/tests
"""

import os
import subprocess
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "lib"))
from apk_certificate import CertificateError, check, signer_digest  # noqa: E402

DEBUG = "5c10537c2a916d25e3799500d8978df2820fa4aa6ea64eeadc3c4316157f43df"
OTHER = "8551e6e7248a509cb1abc493796ce9d509b99c1e54ed80d6002c3877ff440fbf"
SCRIPT = os.path.join(os.path.dirname(__file__), "..", "lib", "apk_certificate.py")

# build-tools 34 to 36.
OLD_ONE = f"""\
Signer #1 certificate DN: C=US, O=Android, CN=Android Debug
Signer #1 certificate SHA-256 digest: {DEBUG}
Signer #1 certificate SHA-1 digest: fc04ee5ad670415c3adcc8aa463e006e2f242bbc
Signer #1 certificate MD5 digest: 058c6601d6d902883d3b77d573801b07
"""
# build-tools 37.
NEW_ONE = f"""\
V3.0 Signer: certificate DN: C=US, O=Android, CN=Android Debug
V3.0 Signer: certificate SHA-256 digest: {DEBUG}
V3.0 Signer: certificate SHA-1 digest: fc04ee5ad670415c3adcc8aa463e006e2f242bbc
V3.0 Signer: certificate MD5 digest: 058c6601d6d902883d3b77d573801b07
"""
OLD_TWO = OLD_ONE + f"""\
Signer #2 certificate DN: CN=Other test
Signer #2 certificate SHA-256 digest: {OTHER}
Signer #2 certificate SHA-1 digest: ac9fca0dd5472a6d152f20461fd58ca891a8beac
"""
NEW_TWO = f"""\
V2 Signer #1: certificate DN: C=US, O=Android, CN=Android Debug
V2 Signer #1: certificate SHA-256 digest: {DEBUG}
V2 Signer #2: certificate DN: CN=Other test
V2 Signer #2: certificate SHA-256 digest: {OTHER}
"""


class ApkCertificate(unittest.TestCase):
    def test_both_formats_with_one_signer(self):
        for output in (OLD_ONE, NEW_ONE):
            self.assertEqual(check(output, DEBUG), DEBUG)
            self.assertEqual(check(output, DEBUG.upper()), DEBUG)

    def test_the_same_signer_under_several_schemes(self):
        both = NEW_ONE + NEW_ONE.replace("V3.0", "V3.1")
        self.assertEqual(check(both, DEBUG), DEBUG)

    def test_another_certificate_is_refused(self):
        for output in (OLD_ONE, NEW_ONE):
            with self.assertRaisesRegex(CertificateError, "expected"):
                check(output, OTHER)

    def test_two_signers_are_refused_in_both_formats(self):
        for output in (OLD_TWO, NEW_TWO):
            with self.assertRaisesRegex(CertificateError, "more than one signer"):
                check(output, DEBUG)
        # Two certificates without signer numbers (e.g. a rotated V3.1 key).
        rotated = NEW_ONE + f"V3.1 Signer: certificate SHA-256 digest: {OTHER}\n"
        with self.assertRaisesRegex(CertificateError, "more than one signer certificate"):
            check(rotated, DEBUG)

    def test_unknown_or_empty_output_is_refused(self):
        for output in (
            "",
            "Verifies\n",
            f"Source Stamp Signer: certificate SHA-256 digest: {DEBUG}\n",
            f"Signer #1 certificate SHA-256 digest: {DEBUG[:-1]}\n",
            f"Signer certificate SHA-256 digest (new): {DEBUG}\n",
        ):
            with self.assertRaises(CertificateError, msg=output):
                signer_digest(output)

    def test_command_line(self):
        def run(output, expected):
            return subprocess.run(
                [sys.executable, "-I", SCRIPT, expected],
                input=output, capture_output=True, text=True,
            )

        ok = run(NEW_ONE, DEBUG)
        self.assertEqual(ok.returncode, 0, ok.stderr)
        self.assertIn(DEBUG, ok.stdout)
        self.assertEqual(run(NEW_TWO, DEBUG).returncode, 1)
        self.assertEqual(run(OLD_ONE, OTHER).returncode, 1)
        self.assertEqual(run(OLD_ONE, "not-a-digest").returncode, 2)


if __name__ == "__main__":
    unittest.main()
