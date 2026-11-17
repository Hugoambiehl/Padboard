"""scripts/deb-version.py: release candidates' .deb sort before the release
(issue #11).

    python3 -m unittest discover -s scripts/tests
"""

import importlib.util
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
SCRIPT = os.path.join(ROOT, "scripts", "deb-version.py")

spec = importlib.util.spec_from_file_location("deb_version", SCRIPT)
deb_version = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deb_version)

HAS_DPKG = shutil.which("dpkg") and shutil.which("dpkg-deb")


def dpkg_lt(a, b):
    return subprocess.run(["dpkg", "--compare-versions", a, "lt", b]).returncode == 0


class DebianVersionTest(unittest.TestCase):
    def test_mapping(self):
        self.assertEqual(deb_version.debian_version("0.2.1-rc.1"), "0.2.1~rc.1")
        self.assertEqual(deb_version.debian_version("0.2.1"), "0.2.1")
        self.assertEqual(deb_version.debian_version("0.3.0-dev"), "0.3.0~dev")
        with self.assertRaises(ValueError):
            deb_version.debian_version("0.2.1~rc.1")

    @unittest.skipUnless(HAS_DPKG, "dpkg not installed")
    def test_dpkg_order(self):
        # SemVer as is: dpkg puts the rc after the release (the bug).
        self.assertFalse(dpkg_lt("0.2.1-rc.1", "0.2.1"))
        ordered = ["0.2.0", "0.2.1-rc.1", "0.2.1-rc.2", "0.2.1-rc.10", "0.2.1", "0.3.0-rc.1"]
        debian = [deb_version.debian_version(v) for v in ordered]
        self.assertEqual(debian[1], "0.2.1~rc.1")
        self.assertTrue(dpkg_lt("0.2.1~rc.1", "0.2.1"))
        for lower, higher in zip(debian, debian[1:]):
            self.assertTrue(dpkg_lt(lower, higher), f"{lower} < {higher}")


@unittest.skipUnless(HAS_DPKG, "dpkg not installed")
class RewriteDebTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.dir)

    def build(self, version, compression):
        root = os.path.join(self.dir, f"pkg-{compression}")
        os.makedirs(os.path.join(root, "DEBIAN"))
        os.makedirs(os.path.join(root, "usr", "bin"))
        with open(os.path.join(root, "usr", "bin", "padboard-desktop"), "w") as f:
            f.write("#!/bin/sh\n")
        os.chmod(os.path.join(root, "usr", "bin", "padboard-desktop"), 0o755)
        with open(os.path.join(root, "DEBIAN", "control"), "w") as f:
            f.write(
                "Package: padboard\n"
                f"Version: {version}\n"
                "Architecture: amd64\n"
                "Maintainer: Test <test@example.org>\n"
                "Description: test\n"
            )
        deb = os.path.join(self.dir, f"Padboard_{version}_{compression}.deb")
        subprocess.run(
            ["dpkg-deb", "--root-owner-group", f"-Z{compression}", "-b", root, deb],
            check=True,
            capture_output=True,
        )
        return deb

    def field(self, deb, name):
        out = subprocess.run(["dpkg-deb", "-f", deb, name], check=True, capture_output=True)
        return out.stdout.decode().strip()

    def members(self, deb):
        with open(deb, "rb") as f:
            return deb_version.read_ar(f.read())

    def test_rc_package_gets_a_tilde(self):
        for compression in ("gzip", "xz"):
            with self.subTest(compression=compression):
                deb = self.build("0.2.1-rc.1", compression)
                before = self.members(deb)
                contents = subprocess.run(
                    ["dpkg-deb", "-c", deb], check=True, capture_output=True
                ).stdout
                self.assertEqual(deb_version.rewrite_deb(deb), ("0.2.1-rc.1", "0.2.1~rc.1"))
                self.assertEqual(self.field(deb, "Version"), "0.2.1~rc.1")
                self.assertEqual(self.field(deb, "Package"), "padboard")
                after = self.members(deb)
                # debian-binary and data.tar.* untouched, same member order.
                self.assertEqual([h[:16] for h, _ in after], [h[:16] for h, _ in before])
                self.assertEqual(after[0], before[0])
                self.assertEqual(after[2], before[2])
                self.assertEqual(
                    subprocess.run(
                        ["dpkg-deb", "-c", deb], check=True, capture_output=True
                    ).stdout,
                    contents,
                )
                # dpkg accepts the package as a whole.
                subprocess.run(["dpkg-deb", "--info", deb], check=True, capture_output=True)
                self.assertTrue(dpkg_lt(self.field(deb, "Version"), "0.2.1"))

    def test_release_package_is_left_alone(self):
        deb = self.build("0.2.1", "gzip")
        with open(deb, "rb") as f:
            original = f.read()
        self.assertEqual(deb_version.rewrite_deb(deb), ("0.2.1", "0.2.1"))
        with open(deb, "rb") as f:
            self.assertEqual(f.read(), original)

    def test_command_line(self):
        out = subprocess.run(
            ["python3", SCRIPT, "--print", "0.2.1-rc.1"], check=True, capture_output=True
        )
        self.assertEqual(out.stdout.decode().strip(), "0.2.1~rc.1")


if __name__ == "__main__":
    unittest.main()
