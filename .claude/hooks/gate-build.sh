#!/usr/bin/env bash
# Stop: nao deixa terminar com build quebrado SE houver mudancas .rs nao commitadas.
# Silencioso em turnos sem mudanca de codigo. Guarda contra loop infinito.
# [tailoring] Quando o Servo virar dependencia, escopar a build (-p basedbrowser ...)
# para nao recompilar o motor a cada Stop.
set -uo pipefail
input="$(cat)"
if [ "$(printf '%s' "$input" | jq -r '.stop_hook_active' 2>/dev/null)" = "true" ]; then
  exit 0
fi
cd "${CLAUDE_PROJECT_DIR:-$PWD}" 2>/dev/null || exit 0

# So age se ha mudancas em .rs (modificadas, staged ou novas/untracked)
changed="$(git diff --name-only HEAD -- '*.rs' 2>/dev/null; git ls-files --others --exclude-standard -- '*.rs' 2>/dev/null)"
[ -z "$changed" ] && exit 0

if ! out="$(cargo build --workspace --quiet 2>&1)"; then
  printf 'Stop bloqueado: cargo build falhou (ha mudancas .rs nao commitadas). Corrija antes de terminar.\n%s\n' "$(printf '%s' "$out" | tail -6)" >&2
  exit 2
fi
exit 0
