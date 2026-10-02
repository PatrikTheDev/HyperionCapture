"""Tests for the privileged Decky plugin lifecycle."""

import importlib.util
import subprocess
import sys
import tempfile
import time
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
        self.plugin._log_path = root / "capture.log"
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
            patch.object(self.plugin, "_is_running", return_value=False),
            patch.object(self.plugin, "_run", return_value=completed) as run,
        ):
            self.assertTrue(self.plugin._ensure_installed())

        self.assertIn("--reinstall", run.call_args.args[0])

    def test_refuses_to_guess_when_installed_version_probe_fails(self) -> None:
        with (
            patch.object(self.plugin, "_is_installed", return_value=True),
            patch.object(self.plugin, "_installed_version", return_value=None),
            patch.object(self.plugin, "_run") as run,
        ):
            self.assertFalse(self.plugin._ensure_installed())

        run.assert_not_called()
        self.assertEqual(
            self.plugin._last_error,
            "Could not determine the installed Flatpak version",
        )

    def test_installed_version_uses_steamos_supported_flatpak_list(self) -> None:
        output = f"org.example.Other  9.9\n{_MODULE.APP_ID}  0.0.4\n"
        completed = subprocess.CompletedProcess([], 0, output, "")
        with patch.object(self.plugin, "_run", return_value=completed) as run:
            self.assertEqual(self.plugin._installed_version(), "0.0.4")

        self.assertEqual(
            run.call_args.args[0],
            [
                "flatpak",
                "list",
                "--system",
                "--app",
                "--columns=application,version",
            ],
        )

    def test_installed_probe_uses_flatpak_info(self) -> None:
        completed = subprocess.CompletedProcess([], 0, "", "")
        with patch.object(self.plugin, "_run", return_value=completed) as run:
            self.assertTrue(self.plugin._is_installed())

        self.assertEqual(
            run.call_args.args[0],
            ["flatpak", "info", "--system", _MODULE.APP_ID],
        )

    def test_reports_repeated_recent_capture_failure(self) -> None:
        error = "failed to capture frame; will re-enumerate and retry error=unsupported buffer"
        self.plugin._log_path.write_text("\n".join([error] * 3), encoding="utf-8")

        self.assertEqual(
            self.plugin._recent_capture_error(),
            "Capture process is running but repeatedly failing: unsupported buffer",
        )

    def test_ignores_a_single_transient_failure(self) -> None:
        self.plugin._log_path.write_text(
            "failed to publish frame error=connection reset\n", encoding="utf-8"
        )

        self.assertIsNone(self.plugin._recent_capture_error())

    def test_capture_failure_expires_after_recovery(self) -> None:
        error = "failed to publish frame error=connection reset"
        self.plugin._log_path.write_text("\n".join([error] * 3), encoding="utf-8")
        recovered_at = time.time() + _MODULE.FAILURE_FRESHNESS_SECONDS + 1
        with patch.object(_MODULE.time, "time", return_value=recovered_at):
            self.assertIsNone(self.plugin._recent_capture_error())


class PluginStatusTests(unittest.IsolatedAsyncioTestCase):
    async def test_running_failed_process_is_not_reported_as_capturing(self) -> None:
        plugin = _MODULE.Plugin()
        with (
            patch.object(plugin, "_is_installed", return_value=True),
            patch.object(plugin, "_is_running", return_value=True),
            patch.object(plugin, "_recent_capture_error", return_value="capture failed"),
        ):
            status = await plugin.get_status()

        self.assertTrue(status["running"])
        self.assertFalse(status["capturing"])
        self.assertEqual(status["error"], "capture failed")


class PluginStartupTests(unittest.IsolatedAsyncioTestCase):
    async def test_auto_start_waits_for_gamescope(self) -> None:
        plugin = _MODULE.Plugin()
        with (
            patch.object(plugin, "_load_config", return_value=_MODULE.CaptureConfig()),
            patch.object(plugin, "_gamescope_running", side_effect=[False, False, True]),
            patch.object(plugin, "_start", return_value=True) as start,
            patch.object(_MODULE.asyncio, "sleep", new_callable=unittest.mock.AsyncMock) as sleep,
        ):
            await plugin._main()

        self.assertEqual(sleep.await_count, 2)
        start.assert_called_once()

    async def test_stop_cancels_pending_auto_start(self) -> None:
        plugin = _MODULE.Plugin()

        async def stop_while_waiting(_seconds: float) -> None:
            await plugin.stop_capture()

        with (
            patch.object(plugin, "_load_config", return_value=_MODULE.CaptureConfig()),
            patch.object(plugin, "_gamescope_running", return_value=False),
            patch.object(plugin, "_stop", return_value=True),
            patch.object(plugin, "_start") as start,
            patch.object(_MODULE.asyncio, "sleep", side_effect=stop_while_waiting),
        ):
            await plugin._main()

        start.assert_not_called()

    async def test_unload_cancels_pending_auto_start(self) -> None:
        plugin = _MODULE.Plugin()

        async def unload_while_waiting(_seconds: float) -> None:
            await plugin._unload()

        with (
            patch.object(plugin, "_load_config", return_value=_MODULE.CaptureConfig()),
            patch.object(plugin, "_gamescope_running", return_value=False),
            patch.object(plugin, "_start") as start,
            patch.object(_MODULE.asyncio, "sleep", side_effect=unload_while_waiting),
        ):
            await plugin._main()

        start.assert_not_called()

    async def test_disabled_auto_start_does_not_probe_gamescope(self) -> None:
        plugin = _MODULE.Plugin()
        with (
            patch.object(plugin, "_load_config", return_value=_MODULE.CaptureConfig(auto_start=False)),
            patch.object(plugin, "_gamescope_running") as gamescope,
            patch.object(plugin, "_start") as start,
        ):
            await plugin._main()

        gamescope.assert_not_called()
        start.assert_not_called()

    def test_manual_launch_refuses_to_run_before_gamescope(self) -> None:
        plugin = _MODULE.Plugin()
        with (
            patch.object(plugin, "_ensure_installed", return_value=True),
            patch.object(plugin, "_is_running", return_value=False),
            patch.object(plugin, "_prepare_bwrap", return_value=True),
            patch.object(plugin, "_load_config", return_value=_MODULE.CaptureConfig()),
            patch.object(plugin, "_gamescope_running", return_value=False),
            patch.object(_MODULE.subprocess, "Popen") as popen,
        ):
            self.assertFalse(plugin._start())

        popen.assert_not_called()
        self.assertIn("Gamescope is not running", plugin._last_error)


if __name__ == "__main__":
    unittest.main()
