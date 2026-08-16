"""Tests for the installable Decky archive layout."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile

_MODULE_PATH = Path(__file__).parents[1] / "verify_package.py"
_SPEC = importlib.util.spec_from_file_location("verify_package", _MODULE_PATH)
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError(f"cannot load {_MODULE_PATH}")
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)
validate_archive = _MODULE.validate_archive


class PackageValidationTests(unittest.TestCase):
    """Exercise requirements imposed by Decky's development ZIP installer."""

    def make_archive(self, files: dict[str, str]) -> Path:
        """Create an archive that remains valid for the current test."""

        temporary = tempfile.NamedTemporaryFile(suffix=".zip", delete=False)
        temporary.close()
        archive = Path(temporary.name)
        self.addCleanup(archive.unlink, missing_ok=True)
        with ZipFile(archive, "w") as package:
            for name, contents in files.items():
                package.writestr(name, contents)
        return archive

    @staticmethod
    def valid_files() -> dict[str, str]:
        """Return the minimum valid package contents."""

        root = "HyperionCapture"
        files = {
            f"{root}/{name}": "fixture" for name in _MODULE.REQUIRED_FILES
        }
        files[f"{root}/plugin.json"] = json.dumps({"name": "Hyperion Capture"})
        files[f"{root}/package.json"] = json.dumps({"version": "0.0.2"})
        return files

    def test_accepts_decky_directory_layout(self) -> None:
        validate_archive(self.make_archive(self.valid_files()))

    def test_rejects_plugin_json_at_archive_root(self) -> None:
        files = self.valid_files()
        files["plugin.json"] = files.pop("HyperionCapture/plugin.json")

        with self.assertRaisesRegex(ValueError, "one top-level directory"):
            validate_archive(self.make_archive(files))

    def test_rejects_missing_package_json(self) -> None:
        files = self.valid_files()
        del files["HyperionCapture/package.json"]

        with self.assertRaisesRegex(ValueError, "package.json"):
            validate_archive(self.make_archive(files))


if __name__ == "__main__":
    unittest.main()
