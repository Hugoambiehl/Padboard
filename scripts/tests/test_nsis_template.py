"""Padboard's NSIS template (apps/desktop/src-tauri/windows/installer.nsi) is
Tauri's own with a few changes: it must follow the Tauri CLI that builds it,
and keep the changes (an update never offers to delete the app data).

    python3 -m unittest discover -s scripts/tests
"""

import json
import os
import re
import unittest

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
DESKTOP = os.path.join(ROOT, "apps", "desktop")
TEMPLATE = os.path.join(DESKTOP, "src-tauri", "windows", "installer.nsi")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


class NsisTemplateTest(unittest.TestCase):
    def setUp(self):
        self.template = read(TEMPLATE)

    def test_based_on_the_tauri_cli_in_use(self):
        lock = json.loads(read(os.path.join(DESKTOP, "package-lock.json")))
        cli = lock["packages"]["node_modules/@tauri-apps/cli"]["version"]
        base = re.search(r"^; Base: tauri-cli (\S+)$", self.template, re.M)
        self.assertIsNotNone(base, "no '; Base: tauri-cli X' line")
        self.assertEqual(
            base.group(1),
            cli,
            "@tauri-apps/cli changed: start again from its installer.nsi and "
            "apply the '; Padboard:' changes (see the template's header)",
        )

    def test_used_by_the_build(self):
        conf = json.loads(read(os.path.join(DESKTOP, "src-tauri", "tauri.conf.json")))
        nsis = conf["bundle"]["windows"]["nsis"]
        self.assertEqual(nsis["template"], "windows/installer.nsi")
        for image in ("headerImage", "sidebarImage"):
            path = os.path.join(DESKTOP, "src-tauri", nsis[image])
            self.assertTrue(read_bytes(path).startswith(b"BM"), path)

    def test_the_previous_version_is_uninstalled_as_an_update(self):
        leave = self.template[self.template.index("Function PageLeaveReinstall") :]
        leave = leave[: leave.index("FunctionEnd")]
        self.assertIn('StrCpy $R1 "$R1 /UPDATE /P"', leave)
        # Tauri's conditional flags are gone.
        self.assertNotIn("${IfThen} $UpdateMode = 1 ${|} StrCpy $R1", leave)
        # Padboard is closed with the user's consent before the passive uninstall.
        self.assertLess(
            leave.index("CheckIfAppIsRunning"), leave.index('StrCpy $R1 "$R1 /UPDATE /P"')
        )

    def test_no_delete_checkbox_in_an_update(self):
        show = self.template[self.template.index("Function un.ConfirmShow") :]
        show = show[: show.index("CreateWindowEx")]
        self.assertIn("${IfThen} $UpdateMode = 1 ${|} Return ${|}", show)
        # The deletion itself stays guarded.
        self.assertRegex(
            self.template,
            r"\$\{If\} \$DeleteAppDataCheckboxState = 1\s*\n\s*\$\{AndIf\} \$UpdateMode <> 1",
        )

    def test_no_directory_page_when_installed(self):
        t = self.template
        page = t.index("!insertmacro MUI_PAGE_DIRECTORY")
        # The directory page has our PRE function (declared just before it).
        self.assertEqual(
            t.rindex("!define MUI_PAGE_CUSTOMFUNCTION_PRE", 0, page),
            t.index("!define MUI_PAGE_CUSTOMFUNCTION_PRE PadboardSkipDirectory"),
        )
        self.assertLess(t.index("Var PadboardInstalled"), t.index("Function .onInit"))
        skip = t[t.index("Function PadboardSkipDirectory") :]
        skip = skip[: skip.index("FunctionEnd")]
        # Passive mode still skips it, as Tauri's SkipIfPassive did.
        self.assertIn("$PassiveMode = 1", skip)
        self.assertIn("$PadboardInstalled = 1", skip)
        self.assertIn("Abort", skip)
        # Read at start, before the reinstall page may uninstall: same
        # Uninstall key as PageReinstall, and the folder to restore.
        init = t[t.index("Function .onInit") :]
        init = init[: init.index("FunctionEnd")]
        found = init.index("StrCpy $PadboardInstalled 1")
        for read_key in (
            'ReadRegStr $0 SHCTX "${UNINSTKEY}" ""',
            'ReadRegStr $1 SHCTX "${UNINSTKEY}" "UninstallString"',
            'ReadRegStr $1 SHCTX "${MANUPRODUCTKEY}" ""',
        ):
            self.assertLess(init.index(read_key), found, read_key)
        self.assertLess(init.index("!insertmacro SetContext"), found)
        # The registers used later (PageReinstall, EarlyChecks) are kept.
        self.assertIn("Push $0", init)
        self.assertIn("Pop $0", init)


def read_bytes(path):
    with open(path, "rb") as f:
        return f.read(2)


if __name__ == "__main__":
    unittest.main()
