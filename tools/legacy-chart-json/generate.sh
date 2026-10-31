#!/usr/bin/env bash
set -euo pipefail
python -I -B "$(dirname "$0")/generate.py" "$@"
