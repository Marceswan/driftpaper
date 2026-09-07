#!/usr/bin/env bash
set -euo pipefail
app_path=$1
archive_path=$2
binary_path="$app_path/Contents/MacOS/DriftPaper"
test -f "$binary_path"
chmod 755 "$binary_path"
# Create the distributable before artifact upload can discard executable modes.
ditto -c -k --sequesterRsrc --keepParent "$app_path" "$archive_path"
check_dir=$(mktemp -d)
trap 'rm -rf "$check_dir"' EXIT
ditto -x -k "$archive_path" "$check_dir"
test -x "$check_dir/$(basename "$app_path")/Contents/MacOS/DriftPaper"
plutil -lint "$check_dir/$(basename "$app_path")/Contents/Info.plist"
