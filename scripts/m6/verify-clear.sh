#!/usr/bin/env bash
# M6 (ADR-0009) — prova que "limpar dados de navegação" zera cookies + histórico e PRESERVA favoritos.
#
# Metodologia (sem captura de janela — Wayland, L-008): perfil REAL isolado (XDG_CONFIG_HOME temp),
# página em localhost (python3) que seta cookie+localStorage e registra uma visita. O driver in-app
# BASEDBROWSER_CLEAR_TEST favorita a página, loga o estado ANTES, invoca clear-browsing-data, e loga
# DEPOIS. Esperado: cookies(aba) 1→0, history N→0, bookmarks preservado (1→1).
#
# Caveat (documentado no ADR-0009): storage_sites usa domínio registrado (eTLD+1); localhost/IPs não
# são listados, então storage_sites pode ficar 0 mesmo com localStorage setado — a limpeza de Web
# Storage por-site vale p/ domínios reais. A evidência determinística aqui é cookies + histórico.
#
# Uso: cargo build --release -p basedbrowser && scripts/m6/verify-clear.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/target/release/basedbrowser"
PAGES="$ROOT/scripts/m6/pages"
PORT="${PORT:-8732}"
EXIT_MS="${EXIT_MS:-9000}"

[ -x "$BIN" ] || { echo "Build primeiro: cargo build --release -p basedbrowser"; exit 1; }
command -v python3 >/dev/null || { echo "python3 não encontrado"; exit 1; }

PROFILE="$(mktemp -d)"
LOG="$(mktemp)"
HTTP_PID=""
cleanup() {
  [ -n "$HTTP_PID" ] && kill "$HTTP_PID" 2>/dev/null || true
  rm -rf "$PROFILE" "$LOG"
}
trap cleanup EXIT

( cd "$PAGES" && exec python3 -m http.server "$PORT" ) >/dev/null 2>&1 &
HTTP_PID=$!
sleep 1
URL="http://127.0.0.1:$PORT/persist.html"
echo "Servindo $URL (pid $HTTP_PID) · perfil $PROFILE"

XDG_CONFIG_HOME="$PROFILE" \
BASEDBROWSER_URL="$URL" \
BASEDBROWSER_CLEAR_TEST=1 \
BASEDBROWSER_EXIT_AFTER_MS="$EXIT_MS" \
  "$BIN" >"$LOG" 2>&1 || true

echo "== leituras =="
grep -F '[cleartest]' "$LOG" || { echo "(driver não logou — aumente EXIT_MS?)"; exit 1; }

antes="$(grep -F '[cleartest] antes:' "$LOG" || true)"
depois="$(grep -F '[cleartest] depois:' "$LOG" || true)"

echo
echo "== VEREDITO =="
ok=1
echo "$antes" | grep -qE 'cookies\(aba\)=[1-9]' || { echo "❌ esperava cookie(s) ANTES"; ok=0; }
echo "$antes" | grep -qE 'history=[1-9]'        || { echo "❌ esperava histórico ANTES"; ok=0; }
echo "$depois" | grep -qE 'cookies\(aba\)=0'    || { echo "❌ cookies NÃO zeraram"; ok=0; }
echo "$depois" | grep -qE 'history=0'           || { echo "❌ histórico NÃO zerou"; ok=0; }
echo "$depois" | grep -qE 'bookmarks=[1-9]'     || { echo "❌ favoritos NÃO preservados"; ok=0; }
if [ "$ok" = 1 ]; then
  echo "✅ cookies e histórico zerados; favoritos preservados"
  exit 0
fi
exit 1
