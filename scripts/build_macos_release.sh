#!/bin/bash
set -euo pipefail

npm run tauri -- "$@"
python3 build_turbo_bar.py --skip-build
