#!/usr/bin/env bash

set -x

ls -la /opt/

# determine which binary to keep
if /opt/acars-oxide.amd64 --version > /dev/null 2>&1; then
  mv -v /opt/acars-oxide.amd64 /opt/acars-oxide
elif /opt/acars-oxide.arm64 --version > /dev/null 2>&1; then
  mv -v /opt/acars-oxide.arm64 /opt/acars-oxide
elif /opt/acars-oxide.armv7 --version > /dev/null 2>&1; then
  mv -v /opt/acars-oxide.armv7 /opt/acars-oxide
else
  echo >&2 "ERROR: Unsupported architecture"
  exit 1
fi
