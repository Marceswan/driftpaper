#!/usr/bin/env bash
set -euo pipefail
# Never enable shell tracing: the environment contains signing credentials.
script_dir=$(cd "$(dirname "$0")" && pwd)
bash "$script_dir/check-signing-config.sh"
app_path=${1:?Usage: sign-notarize-macos.sh app-path output-directory}
output_dir=${2:?Usage: sign-notarize-macos.sh app-path output-directory}
test -f "$app_path/Contents/MacOS/DriftPaper"
mkdir -p "$output_dir"
work_dir=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/driftpaper-signing.XXXXXX")
keychain_path="$work_dir/signing.keychain-db"
cleanup() {
  security delete-keychain "$keychain_path" >/dev/null 2>&1 || true
  rm -rf "$work_dir"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
umask 077
keychain_password=$(openssl rand -hex 32)
printf '%s' "$APPLE_CERTIFICATE_P12_BASE64" | base64 --decode > "$work_dir/certificate.p12"
security create-keychain -p "$keychain_password" "$keychain_path"
security set-keychain-settings -lut 3600 "$keychain_path"
security unlock-keychain -p "$keychain_password" "$keychain_path"
security import "$work_dir/certificate.p12" -k "$keychain_path" \
  -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
  -k "$keychain_password" "$keychain_path" >/dev/null
rm "$work_dir/certificate.p12"

printf '%s' "$APPLE_API_KEY_P8_BASE64" | base64 --decode > "$work_dir/notary.p8"
xcrun notarytool store-credentials driftpaper-ci --keychain "$keychain_path" \
  --key "$work_dir/notary.p8" --key-id "$APPLE_API_KEY_ID" \
  --issuer "$APPLE_API_ISSUER_ID" >/dev/null
rm "$work_dir/notary.p8"

notarize() {
  local archive=$1
  local response="$work_dir/notary-result.json"
  if ! xcrun notarytool submit "$archive" --keychain-profile driftpaper-ci \
      --keychain "$keychain_path" --wait --timeout 20m --output-format json > "$response"; then
    cat "$response" >&2
    return 1
  fi
  # A completed submission can be Invalid. Never treat exit code alone as acceptance.
  if ! python3 - "$response" <<'PY'
import json, sys
result = json.load(open(sys.argv[1]))
print('Notarization:', result.get('id'), result.get('status'))
sys.exit(0 if result.get('status') == 'Accepted' else 1)
PY
  then
    local submission_id
    submission_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$response")
    xcrun notarytool log "$submission_id" --keychain-profile driftpaper-ci \
      --keychain "$keychain_path" || true
    return 1
  fi
}

# Sign only after lipo and all bundle modifications have finished.
chmod 755 "$app_path/Contents/MacOS/DriftPaper"
codesign --force --options runtime --timestamp --keychain "$keychain_path" \
  --sign "$APPLE_SIGNING_IDENTITY" "$app_path"
codesign --verify --deep --strict --verbose=2 "$app_path"
signature=$(codesign --display --verbose=4 "$app_path" 2>&1)
if ! printf '%s\n' "$signature" | grep -Fxq "TeamIdentifier=$APPLE_TEAM_ID"; then
  echo 'Signed app team does not match APPLE_TEAM_ID.' >&2
  exit 1
fi

ditto -c -k --sequesterRsrc --keepParent "$app_path" "$work_dir/submission.zip"
notarize "$work_dir/submission.zip"
xcrun stapler staple "$app_path"
xcrun stapler validate "$app_path"
spctl --assess --type execute --verbose=2 "$app_path"

# ZIPs cannot be stapled: package the app after its ticket is attached.
bash "$script_dir/package-macos.sh" "$app_path" "$output_dir/DriftPaper-macOS.zip"
ditto -x -k "$output_dir/DriftPaper-macOS.zip" "$work_dir/verify"
codesign --verify --deep --strict "$work_dir/verify/$(basename "$app_path")"
xcrun stapler validate "$work_dir/verify/$(basename "$app_path")"

dmg_path="$output_dir/DriftPaper-macOS.dmg"
hdiutil create -volname DriftPaper -srcfolder "$app_path" -ov -format UDZO "$dmg_path"
codesign --force --timestamp --keychain "$keychain_path" \
  --sign "$APPLE_SIGNING_IDENTITY" "$dmg_path"
codesign --verify --strict "$dmg_path"
notarize "$dmg_path"
xcrun stapler staple "$dmg_path"
xcrun stapler validate "$dmg_path"
spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg_path"
