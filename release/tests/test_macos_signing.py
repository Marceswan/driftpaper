"""Exercise release failure gates without sending fake credentials to Apple."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "release/scripts"
FAKE_TOOL = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ['TOOL_LOG'], 'a') as log:
    log.write(json.dumps([name, *args]) + '\n')
if name == 'security' and args[0] == 'import' and os.environ.get('FAIL_IMPORT'):
    sys.exit(1)
if name == 'codesign' and '--display' in args:
    print('TeamIdentifier=' + os.environ.get('SIGNED_TEAM', 'ABCDEFGHIJ'))
if name == 'xcrun' and args[:2] == ['notarytool', 'submit']:
    is_dmg = args[2].endswith('.dmg')
    status = os.environ.get('DMG_STATUS' if is_dmg else 'APP_STATUS', 'Accepted')
    print(json.dumps({'id': 'test-submission', 'status': status}))
    if os.environ.get('FAIL_SUBMIT'): sys.exit(1)
if name == 'xcrun' and args[:2] == ['stapler', 'validate'] and os.environ.get('FAIL_STAPLER'):
    sys.exit(1)
if name == 'ditto':
    dest = pathlib.Path(args[-1])
    if '-x' in args:
        binary = dest / 'DriftPaper.app/Contents/MacOS/DriftPaper'
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_text('test executable')
        binary.chmod(0o755)
    else:
        dest.write_text('test archive')
if name == 'hdiutil': pathlib.Path(args[-1]).write_text('test disk image')
'''


class SigningTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name in ["security", "codesign", "xcrun", "ditto", "hdiutil", "spctl", "plutil"]:
            tool = self.bin / name
            tool.write_text(FAKE_TOOL)
            tool.chmod(0o755)
        self.app = self.root / "DriftPaper.app"
        binary = self.app / "Contents/MacOS/DriftPaper"
        binary.parent.mkdir(parents=True)
        binary.write_text("test executable")
        self.log = self.root / "tool-log.jsonl"
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ["PATH"],
                        RUNNER_TEMP=str(self.root), TOOL_LOG=str(self.log),
                        APPLE_CERTIFICATE_P12_BASE64="dGVzdA==",
                        APPLE_CERTIFICATE_PASSWORD="test-password",
                        APPLE_SIGNING_IDENTITY="Developer ID Application: Test (ABCDEFGHIJ)",
                        APPLE_TEAM_ID="ABCDEFGHIJ", APPLE_API_KEY_P8_BASE64="dGVzdA==",
                        APPLE_API_KEY_ID="TESTKEY123", APPLE_API_ISSUER_ID="test-issuer")

    def run_signing(self, **overrides):
        result = subprocess.run(["bash", str(SCRIPTS / "sign-notarize-macos.sh"),
                                 str(self.app), str(self.root / "output")],
                                env=dict(self.env, **overrides), capture_output=True, text=True)
        self.assertNotIn("test-password", result.stdout + result.stderr)
        self.assertEqual(list(self.root.glob("driftpaper-signing.*")), [])
        calls = [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []
        return result, calls

    def test_success_staples_app_before_packaging_and_notarizes_dmg(self):
        result, calls = self.run_signing()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        submit = [i for i, c in enumerate(calls) if c[:3] == ["xcrun", "notarytool", "submit"]]
        staple = [i for i, c in enumerate(calls) if c[:3] == ["xcrun", "stapler", "staple"]]
        zip_index = next(i for i, c in enumerate(calls) if c[0] == "ditto" and c[-1].endswith("output/DriftPaper-macOS.zip"))
        self.assertEqual(len(submit), 2)
        self.assertEqual(len(staple), 2)
        self.assertLess(submit[0], staple[0])
        self.assertLess(staple[0], zip_index)
        self.assertLess(zip_index, submit[1])
        self.assertLess(submit[1], staple[1])
        self.assertEqual(calls[-1][0:2], ["security", "delete-keychain"])
        self.assertTrue((self.root / "output/DriftPaper-macOS.dmg").exists())

    def test_missing_secret_stops_before_keychain_import(self):
        result, calls = self.run_signing(APPLE_API_KEY_P8_BASE64="")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("APPLE_API_KEY_P8_BASE64", result.stderr)
        self.assertEqual(calls, [])

    def test_wrong_certificate_type_stops_before_import(self):
        result, calls = self.run_signing(APPLE_SIGNING_IDENTITY="Apple Distribution: Test (ABCDEFGHIJ)")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, [])

    def test_actual_signer_team_is_checked(self):
        result, calls = self.run_signing(SIGNED_TEAM="WRONGTEAM1")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[:3] == ["xcrun", "notarytool", "submit"] for c in calls))
        self.assertEqual(calls[-1][:2], ["security", "delete-keychain"])

    def test_import_failure_removes_credentials(self):
        result, calls = self.run_signing(FAIL_IMPORT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls[-1][:2], ["security", "delete-keychain"])

    def test_rejected_app_never_gets_packaged(self):
        result, calls = self.run_signing(APP_STATUS="Invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[0] == "hdiutil" or c[:3] == ["xcrun", "stapler", "staple"] for c in calls))
        self.assertFalse((self.root / "output/DriftPaper-macOS.zip").exists())

    def test_rejected_dmg_never_gets_stapled(self):
        result, calls = self.run_signing(DMG_STATUS="Invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[:3] == ["xcrun", "stapler", "staple"] and c[-1].endswith(".dmg") for c in calls))

    def test_submission_failure_stops_distribution(self):
        result, calls = self.run_signing(FAIL_SUBMIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[:3] == ["xcrun", "stapler", "staple"] for c in calls))

    def test_invalid_ticket_stops_distribution(self):
        result, calls = self.run_signing(FAIL_STAPLER="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[0] == "hdiutil" for c in calls))


if __name__ == "__main__":
    unittest.main()
