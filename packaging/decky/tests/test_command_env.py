"""Tests for launching host commands outside Decky's bundled runtime."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "py_modules"))

from command_env import system_command_environment


class SystemCommandEnvironmentTests(unittest.TestCase):
    def test_removes_injected_library_path(self) -> None:
        environment = system_command_environment(
            {"PATH": "/usr/bin", "LD_LIBRARY_PATH": "/tmp/_MEI123"}
        )

        self.assertEqual(environment, {"PATH": "/usr/bin"})

    def test_restores_original_library_path(self) -> None:
        environment = system_command_environment(
            {
                "PATH": "/usr/bin",
                "LD_LIBRARY_PATH": "/tmp/_MEI123:/usr/local/lib",
                "LD_LIBRARY_PATH_ORIG": "/usr/local/lib",
            }
        )

        self.assertEqual(
            environment,
            {"PATH": "/usr/bin", "LD_LIBRARY_PATH": "/usr/local/lib"},
        )

    def test_preserves_unrelated_variables(self) -> None:
        environment = system_command_environment(
            {"PATH": "/usr/bin", "FLATPAK_BWRAP": "/custom/bwrap"}
        )

        self.assertEqual(
            environment,
            {"PATH": "/usr/bin", "FLATPAK_BWRAP": "/custom/bwrap"},
        )


if __name__ == "__main__":
    unittest.main()
