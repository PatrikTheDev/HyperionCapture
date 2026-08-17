"""Tests for the privileged Decky plugin lifecycle."""

import importlib.util
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

_DECKY_ROOT = Path(__file__).parents[1]
sys.path.insert(0, str(_DECKY_ROOT / "py_modules"))

_decky = types.ModuleType("decky")
_decky.DECKY_PLUGIN_SETTINGS_DIR = "/tmp/hyperion-capture-test/settings"
_decky.DECKY_PLUGIN_DIR = "/tmp/hyperion-capture-test/plugin"
_decky.DECKY_PLUGIN_LOG_DIR = "/tmp/hyperion-capture-test/logs"
_decky.logger = unittest.mock.Mock()
sys.modules["decky"] = _decky

_SPEC = importlib.util.spec_from_file_location("decky_plugin", _DECKY_ROOT / "main.py")
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError("cannot load Decky plugin backend")
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)


class PluginInstallationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.plugin = _MODULE.Plugin()
        self.plugin._bundle_path = root / "capture.flatpak"
        self.plugin._package_path = root / "package.json"
        self.plugin._bundle_path.touch()
        self.plugin._package_path.write_text('{"version":"0.0.5"}', encoding="utf-8")

    def test_does_not_reinstall_matching_payload(self) -> None:
        with (
            patch.object(self.plugin, "_is_installed", return_value=True),
            patch.object(self.plugin, "_installed_version", return_value="0.0.5"),
            patch.object(self.plugin, "_run") as run,
        ):
            self.assertTrue(self.plugin._ensure_installed())

        run.assert_not_called()

    def test_installs_missing_payload(self) -> None:
        completed = subprocess.CompletedProcess([], 0, "", "")
        with (
            patch.object(self.plugin, "_is_installed", return_value=False),
            patch.object(self.plugin, "_run", return_value=completed) as run,
        ):
            self.assertTrue(self.plugin._ensure_installed())

        command = run.call_args.args[0]
        self.assertEqual(command[:4], ["flatpak", "install", "--system", "--noninteractive"])
        self.assertNotIn("--reinstall", command)

    def test_reinstalls_changed_payload(self) -> None:
        completed = subprocess.CompletedProcess([], 0, "", "")
        with (
            patch.object(self.plugin, "_is_installed", return_value=True),
            patch.object(self.plugin, "_installed_version", return_value="0.0.4"),
            patch.object(self.plugin, "_run", return_value=completed) as run,
        ):
            self.assertTrue(self.plugin._ensure_installed())

        self.assertIn("--reinstall", run.call_args.args[0])

    def test_installed_probe_uses_flatpak_info(self) -> None:
        completed = subprocess.CompletedProcess([], 0, "", "")
        with patch.object(self.plugin, "_run", return_value=completed) as run:
            self.assertTrue(self.plugin._is_installed())

        self.assertEqual(
            run.call_args.args[0],
            ["flatpak", "info", "--system", _MODULE.APP_ID],
        )


if __name__ == "__main__":
    unittest.main()
