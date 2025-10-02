#!/usr/bin/env bash
# Quick bootstrap com auto-discovery + progressive + wallet + compliance
curl -sSL https://logline.sh/v3.1 | sh -s -- \
  --auto-discover \
  --progressive \
  --wallet-integration \
  --compliance-auto \
  --rollback-on-fail
