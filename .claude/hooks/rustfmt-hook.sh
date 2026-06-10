#!/usr/bin/env bash
# PostToolUse hook: formata o arquivo Rust recém-editado com rustfmt.
# NUNCA bloqueia o agente — sempre sai com 0 (PostToolUse é advisory).
set -uo pipefail

input="$(cat)"

# Extrai tool_input.file_path do JSON do stdin, sem depender só de jq.
if command -v jq >/dev/null 2>&1; then
  file_path="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null)"
elif command -v python3 >/dev/null 2>&1; then
  file_path="$(printf '%s' "$input" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("tool_input",{}).get("file_path",""))' 2>/dev/null)"
else
  file_path=""
fi

case "$file_path" in
  *.rs)
    if command -v rustfmt >/dev/null 2>&1; then
      rustfmt "$file_path" 2>/dev/null || true
    fi
    ;;
esac

exit 0
