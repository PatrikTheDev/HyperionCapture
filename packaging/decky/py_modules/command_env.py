"""Environment handling for commands launched outside Decky's runtime bundle."""

import os
from collections.abc import Mapping


def system_command_environment(
    source: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Return an environment which uses the host system's shared libraries.

    Decky is distributed as a PyInstaller bundle. Its bootloader prepends a
    temporary directory to ``LD_LIBRARY_PATH`` so the plugin backend can find
    its bundled libraries. Host executables such as Flatpak must not inherit
    that path because those libraries can be ABI-incompatible with SteamOS.
    PyInstaller preserves the pre-bundle value in ``LD_LIBRARY_PATH_ORIG``.
    """

    environment = dict(os.environ if source is None else source)
    original = environment.pop("LD_LIBRARY_PATH_ORIG", None)
    if original is None:
        environment.pop("LD_LIBRARY_PATH", None)
    else:
        environment["LD_LIBRARY_PATH"] = original
    return environment
