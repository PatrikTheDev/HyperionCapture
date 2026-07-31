#!/bin/sh
set -eu

plugin_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
bundle="$plugin_dir/bin/io.github.PatrikTheDev.HyperionCapture.flatpak"
output_dir="$plugin_dir/out"

if [ ! -f "$bundle" ]; then
    echo "missing Flatpak bundle: $bundle" >&2
    echo "build it using the instructions in packaging/flatpak/README.md" >&2
    exit 1
fi

bun run --cwd "$plugin_dir" build

stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT INT TERM

cp "$plugin_dir/plugin.json" "$stage/plugin.json"
cp "$plugin_dir/main.py" "$stage/main.py"
cp "$plugin_dir/../../LICENSE" "$stage/LICENSE"
cp -R "$plugin_dir/dist" "$stage/dist"
cp -R "$plugin_dir/py_modules" "$stage/py_modules"
mkdir -p "$stage/bin" "$output_dir"
cp "$bundle" "$stage/bin/$(basename -- "$bundle")"

(cd "$stage" && zip -qr "$output_dir/HyperionCapture.zip" .)
echo "$output_dir/HyperionCapture.zip"
