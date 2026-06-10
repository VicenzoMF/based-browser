#!/usr/bin/env bash
# Stop: nao deixa terminar com build quebrado SE houver mudancas .rs nao commitadas.
# Silencioso em turnos sem mudanca de codigo. Guarda contra loop infinito.
#
# [escopo, M1] basedbrowser (produto) e servo-poc (PoC do M0) puxam o motor Servo. O motor e
# uma DEP CACHEADA: builds incrementais NAO o recompilam (so quando o pin do servo muda), entao
# o gate fica rapido com o cache quente. Mantemos --exclude servo-poc (PoC descartavel, buildada
# manual/CI); basedbrowser CONTINUA coberto pelo gate. O guard de build fria abaixo evita estourar
# o timeout de 120s do Stop quando o motor ainda nao foi compilado. Ver docs/adr/0002+0003 e
# HARNESS-ROADMAP H1.
set -uo pipefail
input="$(cat)"
if [ "$(printf '%s' "$input" | jq -r '.stop_hook_active' 2>/dev/null)" = "true" ]; then
  exit 0
fi
cd "${CLAUDE_PROJECT_DIR:-$PWD}" 2>/dev/null || exit 0

# So age se ha mudancas em .rs (modificadas, staged ou novas/untracked)
changed="$(git diff --name-only HEAD -- '*.rs' 2>/dev/null; git ls-files --others --exclude-standard -- '*.rs' 2>/dev/null)"
[ -z "$changed" ] && exit 0

# Guard de build fria: se o motor (servo) ainda nao foi compilado neste target, uma build aqui
# recompilaria o motor (varios minutos) e estouraria o timeout de 120s do Stop. Nesse caso pulamos
# (a 1a build do motor e deliberada/manual: `cargo build -p basedbrowser`). Com o cache quente, o
# gate roda normalmente e cobre o basedbrowser.
if ! ls target/debug/deps/libservo-*.rlib >/dev/null 2>&1; then
  echo "gate-build: motor (servo) ainda nao compilado neste target; pulando build fria (rode 'cargo build -p basedbrowser' manualmente)." >&2
  exit 0
fi

if ! out="$(cargo build --workspace --exclude servo-poc --quiet 2>&1)"; then
  printf 'Stop bloqueado: cargo build falhou (ha mudancas .rs nao commitadas). Corrija antes de terminar.\n%s\n' "$(printf '%s' "$out" | tail -6)" >&2
  exit 2
fi
exit 0
