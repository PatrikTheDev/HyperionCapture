"""Privileged Decky lifecycle host for the Hyperion Capture Flatpak."""

import asyncio
import json
import os
import shutil
import stat
import subprocess
import time
from collections.abc import Sequence
from pathlib import Path
from typing import Any

import decky
from command_env import system_command_environment
from config import CaptureConfig

APP_ID = "io.github.PatrikTheDev.HyperionCapture"
STATE_DIR = Path("/var/lib/decky-hyperion-capture")
BWRAP_SOURCE = Path("/usr/bin/bwrap")
BWRAP_PATH = STATE_DIR / "bwrap"
FAILURE_MARKERS = (
    "failed to capture frame",
    "failed to publish frame",
)
FAILURE_REPEAT_COUNT = 3
FAILURE_FRESHNESS_SECONDS = 5.0
LOG_TAIL_BYTES = 64 * 1024


class Plugin:
    """Install and supervise the system Flatpak from Decky's root backend."""

    def __init__(self) -> None:
        self._config_path = Path(decky.DECKY_PLUGIN_SETTINGS_DIR) / "config.json"
        self._package_path = Path(decky.DECKY_PLUGIN_DIR) / "package.json"
        self._bundle_path = Path(decky.DECKY_PLUGIN_DIR) / "bin" / f"{APP_ID}.flatpak"
        self._log_path = Path(decky.DECKY_PLUGIN_LOG_DIR) / "capture.log"
        self._last_error = ""
        self._operation_lock = asyncio.Lock()

    async def get_status(self) -> dict[str, Any]:
        """Return the installation and process state for the frontend."""

        installed, running = await asyncio.gather(
            asyncio.to_thread(self._is_installed),
            asyncio.to_thread(self._is_running),
        )
        capture_error = (
            await asyncio.to_thread(self._recent_capture_error) if running else None
        )
        return {
            "installed": installed,
            "running": running,
            "capturing": running and capture_error is None,
            "error": capture_error or self._last_error,
        }

    async def get_config(self) -> dict[str, Any]:
        """Return validated persisted configuration."""

        config = await asyncio.to_thread(self._load_config)
        return config.to_dict()

    async def save_config(self, value: dict[str, Any]) -> dict[str, Any]:
        """Validate and atomically persist configuration received from the UI."""

        try:
            config = CaptureConfig.from_mapping(value)
            await asyncio.to_thread(self._write_config, config)
            self._last_error = ""
            return {"ok": True, "config": config.to_dict(), "error": ""}
        except (OSError, TypeError, ValueError) as error:
            self._last_error = str(error)
            return {"ok": False, "config": None, "error": self._last_error}

    async def start_capture(self) -> bool:
        """Install the payload if necessary and start capture."""

        async with self._operation_lock:
            return await asyncio.to_thread(self._start)

    async def stop_capture(self) -> bool:
        """Stop all running instances of the capture Flatpak."""

        async with self._operation_lock:
            return await asyncio.to_thread(self._stop)

    async def _main(self) -> None:
        decky.logger.info("Loading Hyperion Capture")
        if self._load_config().auto_start:
            async with self._operation_lock:
                await asyncio.to_thread(self._start)

    async def _unload(self) -> None:
        # The Flatpak deliberately survives a Decky reload. On the next load,
        # _main observes it and avoids starting a duplicate.
        decky.logger.info("Hyperion Capture plugin unloaded")

    async def _uninstall(self) -> None:
        # Decky may terminate an uninstall coroutine quickly, so keep cleanup
        # synchronous and bounded, like Decky Sunshine does.
        self._stop()
        try:
            if BWRAP_PATH.exists():
                BWRAP_PATH.unlink()
            if STATE_DIR.exists():
                STATE_DIR.rmdir()
        except OSError as error:
            decky.logger.error("Could not remove privileged launcher: %s", error)

    def _load_config(self) -> CaptureConfig:
        try:
            with self._config_path.open(encoding="utf-8") as file:
                return CaptureConfig.from_mapping(json.load(file))
        except FileNotFoundError:
            return CaptureConfig()
        except (json.JSONDecodeError, OSError, TypeError, ValueError) as error:
            self._last_error = f"Invalid saved configuration: {error}"
            decky.logger.error(self._last_error)
            return CaptureConfig(auto_start=False)

    def _write_config(self, config: CaptureConfig) -> None:
        self._config_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        temporary = self._config_path.with_suffix(".tmp")
        with temporary.open("w", encoding="utf-8") as file:
            json.dump(config.to_dict(), file, indent=2)
            file.write("\n")
        temporary.chmod(0o600)
        temporary.replace(self._config_path)

    def _start(self) -> bool:
        if not self._ensure_installed():
            return False
        if self._is_running():
            self._last_error = ""
            return True
        if not self._prepare_bwrap():
            return False

        config = self._load_config()
        environment = system_command_environment()
        environment["FLATPAK_BWRAP"] = str(BWRAP_PATH)
        command = ["flatpak", "run", "--system", APP_ID, *config.capture_args()]

        try:
            self._log_path.parent.mkdir(parents=True, exist_ok=True)
            with self._log_path.open("wb", buffering=0) as log_file:
                subprocess.Popen(
                    command,
                    env=environment,
                    stdin=subprocess.DEVNULL,
                    stdout=log_file,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
        except OSError as error:
            self._last_error = f"Could not start Hyperion Capture: {error}"
            decky.logger.error(self._last_error)
            return False

        for _ in range(20):
            if self._is_running():
                self._last_error = ""
                return True
            time.sleep(0.25)
        self._last_error = "Flatpak exited before becoming ready; inspect capture.log"
        decky.logger.error(self._last_error)
        return False

    def _stop(self) -> bool:
        if not self._is_running():
            self._last_error = ""
            return True
        result = self._run(["flatpak", "kill", APP_ID], "stopping Hyperion Capture")
        if result is None:
            return False
        self._last_error = ""
        return True

    def _is_installed(self) -> bool:
        result = self._run(
            ["flatpak", "info", "--system", APP_ID],
            "checking the Flatpak installation",
            record_error=False,
        )
        return result is not None

    def _installed_version(self) -> str | None:
        result = self._run(
            ["flatpak", "info", "--system", "--show-version", APP_ID],
            "checking the installed Flatpak version",
            record_error=False,
        )
        if result is None:
            return None
        version = result.stdout.strip()
        return version or None

    def _bundled_version(self) -> str | None:
        try:
            with self._package_path.open(encoding="utf-8") as file:
                value = json.load(file)
            version = value.get("version")
            return version.strip() if isinstance(version, str) and version.strip() else None
        except (json.JSONDecodeError, OSError, AttributeError):
            return None

    def _is_running(self) -> bool:
        result = self._run(
            ["flatpak", "ps", "--columns=application"],
            "checking the capture process",
            record_error=False,
        )
        return result is not None and APP_ID in result.stdout.splitlines()

    def _recent_capture_error(self) -> str | None:
        """Return the latest error when the process is repeatedly failing.

        The capture binary retries recoverable capture and publication errors
        indefinitely. Those retries keep the process alive, so Flatpak process
        state alone cannot indicate that frames are reaching Hyperion. Once the
        log stops receiving failures, the error expires to allow recovery after
        a DRM modeset or temporary network outage.
        """

        try:
            log_stat = self._log_path.stat()
            if time.time() - log_stat.st_mtime > FAILURE_FRESHNESS_SECONDS:
                return None
            with self._log_path.open("rb") as log_file:
                log_file.seek(max(0, log_stat.st_size - LOG_TAIL_BYTES))
                lines = log_file.read().decode("utf-8", errors="replace").splitlines()
        except (OSError, ValueError):
            return None

        failures = [line for line in lines[-50:] if any(marker in line for marker in FAILURE_MARKERS)]
        if len(failures) < FAILURE_REPEAT_COUNT:
            return None

        latest = failures[-1]
        detail = latest.partition(" error=")[2].strip()
        if not detail:
            detail = next(marker for marker in FAILURE_MARKERS if marker in latest)
        return f"Capture process is running but repeatedly failing: {detail}"

    def _ensure_installed(self) -> bool:
        installed = self._is_installed()
        if installed:
            installed_version = self._installed_version()
            bundled_version = self._bundled_version()
            if (
                installed_version is None
                or bundled_version is None
                or installed_version == bundled_version
            ):
                return True

        if not self._bundle_path.is_file():
            self._last_error = f"Bundled Flatpak is missing at {self._bundle_path}"
            decky.logger.error(self._last_error)
            return False

        install_options = ["--reinstall"] if installed else []
        result = self._run(
            [
                "flatpak",
                "install",
                "--system",
                "--noninteractive",
                *install_options,
                str(self._bundle_path),
            ],
            "updating the bundled Flatpak" if installed else "installing the bundled Flatpak",
        )
        return result is not None

    def _prepare_bwrap(self) -> bool:
        try:
            source = BWRAP_SOURCE.resolve(strict=True)
            source_stat = source.stat()
            if not stat.S_ISREG(source_stat.st_mode):
                raise OSError(f"{source} is not a regular file")
            if source_stat.st_uid != 0 or source_stat.st_mode & 0o022:
                raise OSError(f"{source} is not a trusted root-owned binary")

            STATE_DIR.mkdir(mode=0o755, parents=True, exist_ok=True)
            os.chown(STATE_DIR, 0, 0)
            STATE_DIR.chmod(0o755)
            temporary = STATE_DIR / "bwrap.new"
            shutil.copyfile(source, temporary)
            os.chown(temporary, 0, 0)
            temporary.chmod(0o4755)
            temporary.replace(BWRAP_PATH)

            installed = BWRAP_PATH.stat()
            if installed.st_uid != 0 or not installed.st_mode & stat.S_ISUID:
                raise OSError("setuid bit was not retained on the bwrap copy")
            mount_options = self._mount_options(BWRAP_PATH)
            if mount_options is None:
                raise OSError("could not determine bwrap target mount options")
            if "nosuid" in mount_options:
                raise OSError(f"{BWRAP_PATH} is on a nosuid filesystem")
            self._last_error = ""
            return True
        except OSError as error:
            self._last_error = f"Could not prepare privileged bwrap: {error}"
            decky.logger.error(self._last_error)
            return False

    def _run(
        self,
        command: Sequence[str],
        context: str,
        *,
        record_error: bool = True,
    ) -> subprocess.CompletedProcess[str] | None:
        try:
            result = subprocess.run(
                command,
                capture_output=True,
                text=True,
                check=False,
                env=system_command_environment(),
            )
        except OSError as error:
            if record_error:
                self._last_error = f"Error {context}: {error}"
                decky.logger.error(self._last_error)
            return None
        if result.returncode != 0:
            if record_error:
                detail = result.stderr.strip() or f"exit status {result.returncode}"
                self._last_error = f"Error {context}: {detail}"
                decky.logger.error(self._last_error)
            return None
        return result

    @staticmethod
    def _mount_options(path: Path) -> set[str] | None:
        """Return options for the longest mount point containing path."""

        target = path.resolve(strict=False)
        best: tuple[int, set[str]] | None = None
        try:
            with Path("/proc/self/mounts").open(encoding="utf-8") as mounts:
                for line in mounts:
                    fields = line.split()
                    if len(fields) < 4:
                        continue
                    mount_text = fields[1]
                    for encoded, decoded in (
                        ("\\040", " "),
                        ("\\011", "\t"),
                        ("\\134", "\\"),
                    ):
                        mount_text = mount_text.replace(encoded, decoded)
                    mount = Path(mount_text)
                    if target != mount and mount not in target.parents:
                        continue
                    candidate = (len(str(mount)), set(fields[3].split(",")))
                    if best is None or candidate[0] > best[0]:
                        best = candidate
        except OSError:
            return None
        return best[1] if best is not None else None
