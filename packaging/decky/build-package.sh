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

stage_root=$(mktemp -d)
trap 'rm -rf -- "$stage_root"' EXIT INT TERM
stage="$stage_root/HyperionCapture"
archive="$stage_root/HyperionCapture.zip"
mkdir -p "$stage"

cp "$plugin_dir/plugin.json" "$stage/plugin.json"
cp "$plugin_dir/package.json" "$stage/package.json"
cp "$plugin_dir/main.py" "$stage/main.py"
cp "$plugin_dir/../../LICENSE" "$stage/LICENSE"
cp -R "$plugin_dir/dist" "$stage/dist"
mkdir -p "$stage/bin" "$stage/py_modules" "$output_dir"
cp "$plugin_dir"/py_modules/*.py "$stage/py_modules/"
cp "$bundle" "$stage/bin/$(basename -- "$bundle")"

(cd "$stage_root" && zip -qr "$archive" HyperionCapture)
python3 "$plugin_dir/verify_package.py" "$archive"
mv "$archive" "$output_dir/HyperionCapture.zip"
echo "$output_dir/HyperionCapture.zip"
