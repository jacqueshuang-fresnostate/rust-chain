#!/usr/bin/env bash
set -euo pipefail

PEN_FILE="${1:?pen file is required}"
SCRIPT_FILE="${2:?execute script is required}"

python3 - "$SCRIPT_FILE" <<'PY' | pencil interactive --in "$PEN_FILE" --out "$PEN_FILE"
import json
import sys

source = open(sys.argv[1], encoding="utf-8").read()
print("execute(" + json.dumps({"input": source}, ensure_ascii=False) + ")")
print("save()")
print("exit()")
PY
