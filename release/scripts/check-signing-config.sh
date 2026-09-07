#!/usr/bin/env bash
set -euo pipefail

missing=0
for name in APPLE_CERTIFICATE_P12_BASE64 APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_TEAM_ID APPLE_API_KEY_P8_BASE64 APPLE_API_KEY_ID APPLE_API_ISSUER_ID; do
  if [[ -z "${!name:-}" ]]; then
    echo "Missing required signing secret: $name" >&2
    missing=1
  fi
done
if [[ "$missing" != 0 ]]; then
  echo 'See docs/macos-signing.md. Unsigned releases are disabled.' >&2
  exit 1
fi
if [[ "$APPLE_SIGNING_IDENTITY" != 'Developer ID Application: '* ]]; then
  echo 'APPLE_SIGNING_IDENTITY must be a Developer ID Application identity.' >&2
  exit 1
fi
if [[ ! "$APPLE_TEAM_ID" =~ ^[A-Z0-9]{10}$ ]] || [[ "$APPLE_SIGNING_IDENTITY" != *"($APPLE_TEAM_ID)" ]]; then
  echo 'Signing identity must belong to APPLE_TEAM_ID (10 uppercase letters/digits).' >&2
  exit 1
fi
