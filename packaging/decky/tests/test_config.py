"""Tests for values crossing the untrusted Decky frontend boundary."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "py_modules"))

from config import CaptureConfig


class CaptureConfigTests(unittest.TestCase):
    def test_builds_expected_capture_arguments(self) -> None:
        config = CaptureConfig(drm_connector="DP-1")

        self.assertEqual(
            config.capture_args(),
            [
                "--capture",
                "kms",
                "--hyperion-url",
                "http://127.0.0.1:8090/",
                "--hyperion-flatbuffer-port",
                "19400",
                "--priority",
                "150",
                "--fps",
                "20",
                "--output-height",
                "480",
                "--drm-device",
                "/dev/dri/card0",
                "--drm-connector",
                "DP-1",
            ],
        )

    def test_rejects_credentials_in_url(self) -> None:
        with self.assertRaisesRegex(ValueError, "must not contain credentials"):
            CaptureConfig.from_mapping({"hyperion_url": "http://token@example.test/"})

    def test_rejects_non_drm_device(self) -> None:
        with self.assertRaisesRegex(ValueError, "DRM device"):
            CaptureConfig.from_mapping({"drm_device": "/dev/null"})

    def test_rejects_flatbuffer_priority_outside_protocol_range(self) -> None:
        with self.assertRaisesRegex(ValueError, "priority"):
            CaptureConfig.from_mapping({"priority": 99})

    def test_rejects_fractional_numeric_fields(self) -> None:
        with self.assertRaisesRegex(TypeError, "FPS must be an integer"):
            CaptureConfig.from_mapping({"fps": 20.5})

    def test_rejects_invalid_output_height(self) -> None:
        with self.assertRaisesRegex(ValueError, "Output height"):
            CaptureConfig.from_mapping({"output_height": 0})

    def test_rejects_string_boolean(self) -> None:
        with self.assertRaisesRegex(TypeError, "true or false"):
            CaptureConfig.from_mapping({"auto_start": "false"})


if __name__ == "__main__":
    unittest.main()
