#!/usr/bin/env bash
# M6 (ADR-0009) — prova que cookies + localStorage SOBREVIVEM ao restart.
#
# Metodologia (sem captura de janela — Wayland, L-008; espelha os perfis-limpos do ADR-0008):
#   - Perfil REAL isolado: XDG_CONFIG_HOME=$PROFILE (temporário, mas PERSISTENTE entre os 2 runs) →
#     não polui o ~/.config do usuário, mas os dados sobrevivem de um run p/ o outro (é o ponto).
#   - Página servida por `python3 -m http.server` em 127.0.0.1 (origem http REAL; file:// não persiste
#     confiável). A página seta e lê cookie+localStorage e reflete no document.title.
#   - Driver in-app BASEDBROWSER_PERSIST_TEST loga o título + cookies em TEXTO.
#   RUN1 (perfil novo) seta os dados → "cookie=MISS local=MISS". RUN2 (mesmo perfil) lê de volta →
#   "cookie=42 local=persisted-99" ⇒ persistência validada.
#
# Uso: cargo build --release -p basedbrowser && scripts/m6/verify-persist.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/target/release/basedbrowser"
PAGES="$ROOT/scripts/m6/pages"
PORT="${PORT:-8731}"
EXIT_MS="${EXIT_MS:-9000}"

[ -x "$BIN" ] || { echo "Build primeiro: cargo build --release -p basedbrowser"; exit 1; }
command -v python3 >/dev/null || { echo "python3 não encontrado"; exit 1; }

PROFILE="$(mktemp -d)"
RESULTS="$(mktemp -d)"
HTTP_PID=""
cleanup() {
  [ -n "$HTTP_PID" ] && kill "$HTTP_PID" 2>/dev/null || true
  rm -rf "$PROFILE" "$RESULTS"
}
trap cleanup EXIT

# Servidor local determinístico (sem rede externa).
( cd "$PAGES" && exec python3 -m http.server "$PORT" ) >/dev/null 2>&1 &
HTTP_PID=$!
sleep 1
URL="http://127.0.0.1:$PORT/persist.html"
echo "Servindo $URL (pid $HTTP_PID) · perfil $PROFILE"

run() {
  local label="$1" log="$2"
  XDG_CONFIG_HOME="$PROFILE" \
  BASEDBROWSER_URL="$URL" \
  BASEDBROWSER_PERSIST_TEST=1 \
  BASEDBROWSER_EXIT_AFTER_MS="$EXIT_MS" \
    "$BIN" >"$log" 2>&1 || true
  printf '[%s] %s\n' "$label" "$(grep -F '[persisttest] title=' "$log" || echo '(sem leitura)')"
}

echo "== RUN1 (perfil novo: seta cookie+localStorage) =="
run RUN1 "$RESULTS/run1.log"
echo "== RUN2 (mesmo perfil: lê de volta) =="
run RUN2 "$RESULTS/run2.log"

echo
echo "== VEREDITO =="
if grep -qF 'BBPERSIST cookie=42 local=persisted-99' "$RESULTS/run2.log"; then
  echo "✅ cookie E localStorage sobreviveram ao restart (RUN2 leu os valores setados no RUN1)"
  grep -F '[persisttest] cookie ' "$RESULTS/run2.log" || true
  exit 0
fi
echo "❌ persistência NÃO confirmada. Leitura do RUN2:"
grep -F '[persisttest]' "$RESULTS/run2.log" || echo "(driver não logou — aumente EXIT_MS?)"
exit 1
