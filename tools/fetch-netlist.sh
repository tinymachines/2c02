#!/usr/bin/env bash
# Fetches Quietust's Visual 2C02 die data and simulator into
# extern/visual2c02/, each file verified against the sha256 recorded on
# 2026-09-02 (docs/ppu-handoff-v0_1.md section 2). The data derives from
# the visual6502 team's CC BY-NC-SA imagery and is never committed or
# shipped; see NOTICE.md.
set -euo pipefail
cd "$(dirname "$0")/.."

BASE="http://www.qmtpro.com/~nes/chipimages/visual2c02"
DEST="extern/visual2c02"

declare -A SHA=(
  [segdefs.js]=0322784743189ac75ba738630b97985dca507fe687a26f27333cc69ba774d87e
  [transdefs.js]=813ea9b73d833aa24fd3305d4734ddd378645c1a48d2c269cf9f54befa4a2471
  [nodenames.js]=7a6d41c271024d49544208b1507ff00bc837cc75105e03004d4a380e0bc7cd7a
  [wires.js]=449e16c469bb86572cb587b689813594aa2bd513a132ea09213d80c578d86122
  [chipsim.js]=9e8a9de74a97f622e06ebed473a29d95d5249a6a87259266a1184a9911ba1711
)

mkdir -p "$DEST"
for f in "${!SHA[@]}"; do
    if [ -f "$DEST/$f" ] && echo "${SHA[$f]}  $DEST/$f" | sha256sum -c - >/dev/null 2>&1; then
        echo "already fetched: $f"
        continue
    fi
    curl -sL -o "$DEST/$f" "$BASE/$f"
    echo "${SHA[$f]}  $DEST/$f" | sha256sum -c -
done
echo "fetched and verified: $DEST"
