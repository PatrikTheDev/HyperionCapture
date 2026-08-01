"""Validated configuration shared by the Decky backend and its tests."""

import re
from dataclasses import asdict, dataclass
from typing import Any
from urllib.parse import urlparse

_DRM_DEVICE = re.compile(r"^/dev/dri/card[0-9]+$")
_DRM_CONNECTOR = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]*$")


def _integer(value: Any, name: str) -> int:
    """Parse an integer without accepting booleans or fractional numbers."""

    if isinstance(value, bool):
        raise TypeError(f"{name} must be an integer")
    if isinstance(value, int):
        return value
    if isinstance(value, str) and re.fullmatch(r"[0-9]+", value.strip()):
        return int(value)
    raise TypeError(f"{name} must be an integer")


@dataclass(frozen=True)
class CaptureConfig:
    """User-controlled arguments accepted by the capture process."""

    hyperion_url: str = "http://127.0.0.1:8090/"
    flatbuffer_port: int = 19400
    priority: int = 150
    fps: int = 20
    output_height: int = 480
    drm_device: str = "/dev/dri/card0"
    drm_connector: str = ""
    auto_start: bool = True

    @classmethod
    def from_mapping(cls, value: dict[str, Any]) -> "CaptureConfig":
        """Parse and validate data received from persistent storage or RPC."""

        auto_start = value.get("auto_start", cls.auto_start)
        if not isinstance(auto_start, bool):
            raise TypeError("Start-with-Decky must be true or false")
        config = cls(
            hyperion_url=str(value.get("hyperion_url", cls.hyperion_url)).strip(),
            flatbuffer_port=_integer(
                value.get("flatbuffer_port", cls.flatbuffer_port), "FlatBuffers port"
            ),
            priority=_integer(value.get("priority", cls.priority), "Priority"),
            fps=_integer(value.get("fps", cls.fps), "FPS"),
            output_height=_integer(
                value.get("output_height", cls.output_height), "Output height"
            ),
            drm_device=str(value.get("drm_device", cls.drm_device)).strip(),
            drm_connector=str(value.get("drm_connector", cls.drm_connector)).strip(),
            auto_start=auto_start,
        )
        config.validate()
        return config

    def validate(self) -> None:
        """Reject values that are invalid or unsafe to pass to the process."""

        url = urlparse(self.hyperion_url)
        if url.scheme not in {"http", "https"} or not url.hostname:
            raise ValueError("Hyperion URL must be an absolute HTTP or HTTPS URL")
        if url.username is not None or url.password is not None:
            raise ValueError("Hyperion URL must not contain credentials")
        if any(ord(character) < 32 for character in self.hyperion_url):
            raise ValueError("Hyperion URL must not contain control characters")
        if not 1 <= self.flatbuffer_port <= 65535:
            raise ValueError("FlatBuffers port must be between 1 and 65535")
        if not 100 <= self.priority <= 199:
            raise ValueError("FlatBuffers priority must be between 100 and 199")
        if not 1 <= self.fps <= 120:
            raise ValueError("FPS must be between 1 and 120")
        if not 1 <= self.output_height <= 4320:
            raise ValueError("Output height must be between 1 and 4320")
        if not _DRM_DEVICE.fullmatch(self.drm_device):
            raise ValueError("DRM device must look like /dev/dri/card0")
        if self.drm_connector and not _DRM_CONNECTOR.fullmatch(self.drm_connector):
            raise ValueError("DRM connector contains unsupported characters")

    def to_dict(self) -> dict[str, Any]:
        """Return JSON-compatible configuration."""

        return asdict(self)

    def capture_args(self) -> list[str]:
        """Build arguments passed after the Flatpak application ID."""

        args = [
            "--capture",
            "kms",
            "--hyperion-url",
            self.hyperion_url,
            "--hyperion-flatbuffer-port",
            str(self.flatbuffer_port),
            "--priority",
            str(self.priority),
            "--fps",
            str(self.fps),
            "--output-height",
            str(self.output_height),
            "--drm-device",
            self.drm_device,
        ]
        if self.drm_connector:
            args.extend(["--drm-connector", self.drm_connector])
        return args
