#!/usr/bin/env bash
# M7 (ADR-0010) — prova a inspeção in-app: CONSOLE + EVAL + REDE (req+resp), sem Firefox externo.
#
# Metodologia (sem captura de janela — Wayland, L-008; perfil-limpo do ADR-0008):
#   - Perfil REAL isolado: XDG_CONFIG_HOME=$PROFILE (temporário) → não polui o ~/.config nem restaura
#     uma sessão antiga (a sessão restaurada teria precedência sobre BASEDBROWSER_URL — ADR-0007).
#   - Página servida por `python3 -m http.server` em 127.0.0.1 (origem http REAL): faz console.log e
#     busca /data.json em intervalo (gera rede com RESPOSTA — status/headers/payload).
#   - BASEDBROWSER_DEVTOOLS=1 liga o servidor de devtools do Servo (loopback) e o NOSSO cliente RDP
#     in-app (devtools_client.rs) conecta nele e extrai os eventos de rede.
#   - Driver in-app BASEDBROWSER_DEVTOOLS_TEST loga em TEXTO: console capturado, eval (2+2 / title),
#     rede (método/URL/status + 1º response header) e os models do painel do Slint (row_count).
#
# Uso: cargo build --release -p basedbrowser && scripts/m7/verify-devtools.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/target/release/basedbrowser"
PAGES="$ROOT/scripts/m7/pages"
PORT="${PORT:-8771}"
EXIT_MS="${EXIT_MS:-18000}"

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

# Servidor local determinístico (sem rede externa).
( cd "$PAGES" && exec python3 -m http.server "$PORT" ) >/dev/null 2>&1 &
HTTP_PID=$!
sleep 1
URL="http://127.0.0.1:$PORT/devtools.html"
echo "Servindo $URL (pid $HTTP_PID) · perfil $PROFILE · porta devtools 7000"

XDG_CONFIG_HOME="$PROFILE" \
BASEDBROWSER_URL="$URL" \
BASEDBROWSER_DEVTOOLS=1 \
BASEDBROWSER_DEVTOOLS_TEST=1 \
BASEDBROWSER_EXIT_AFTER_MS="$EXIT_MS" \
  "$BIN" >"$LOG" 2>&1 || true

echo
echo "== Evidência (in-app, TEXTO) =="
grep -E '\[m7\] devtools: server started|\[m7\] devtools-net: assinado|\[devtoolstest\]' "$LOG" || true

echo
echo "== VEREDITO =="
ok=0
grep -qF '[devtoolstest] console[log] hello-42' "$LOG" && { echo "✅ console.log capturado (hello-42)"; } || { echo "❌ console.log não capturado"; ok=1; }
grep -qE '\[devtoolstest\] console\[result\] 4' "$LOG" && { echo "✅ eval 2+2 → 4"; } || { echo "❌ eval 2+2 falhou"; ok=1; }
grep -qE '\[devtoolstest\] console\[result\] BBDEVTOOLS' "$LOG" && { echo "✅ eval document.title → BBDEVTOOLS (DOM via eval)"; } || { echo "❌ eval document.title falhou"; ok=1; }
grep -qE '\[devtoolstest\] net GET .*/data.json.* status=200 OK' "$LOG" && { echo "✅ rede: GET /data.json com status=200 OK (lado da RESPOSTA via cliente RDP próprio)"; } || { echo "❌ rede status 200 não capturado"; ok=1; }
grep -qE '\[devtoolstest\] net   resp_header\[0\]' "$LOG" && { echo "✅ rede: response header capturado"; } || { echo "❌ response header não capturado"; ok=1; }
grep -qE '\[devtoolstest\] models do painel: dev-console=[1-9].* dev-net=[1-9]' "$LOG" && { echo "✅ painel: models do Slint populados (console + rede)"; } || { echo "❌ models do painel vazios"; ok=1; }

echo
if [ "$ok" -eq 0 ]; then
  echo "✅ M7: inspeção in-app (console + eval + rede req/resp) validada — sem Firefox externo."
else
  echo "❌ Alguma checagem falhou — veja a evidência acima (aumente EXIT_MS?)."
fi
exit "$ok"
