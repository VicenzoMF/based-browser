#!/usr/bin/env bash
# update-servo/run.sh — parte MECÂNICA do runbook de bump do pin do Servo (docs/runbooks/atualizar-servo.md).
# Mede o esforço de um bump-candidato contra a meta do PROJECT (Goal #3): "< 1 dia de trabalho por sprint".
#
# SEGURANÇA / DESIGN: roda num GIT WORKTREE ISOLADO sob $TMPDIR — NUNCA mexe no working tree da `main`
# (o pin é config protegida, ADR-0002; um bump real exige ADR novo + atualizar o archgate). Reusa o
# target compartilhado (CARGO_TARGET_DIR) para o cache: re-pin p/ a MESMA versão = quase instantâneo;
# bump real = recompila só o que mudou. Não commita nada.
#
# Uso:
#   scripts/update-servo/run.sh <versao-alvo> [--toolchain <channel>]
#   scripts/update-servo/run.sh 0.2.0                 # rehearsal mecânico (no-op de versão; prova o gate)
#   scripts/update-servo/run.sh 0.3.0 --toolchain 1.93.0
#   scripts/update-servo/run.sh --help
set -uo pipefail

usage() { sed -n '2,20p' "$0"; exit "${1:-0}"; }
[ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ] && usage 0
TARGET="${1:-}"
[ -z "$TARGET" ] && { echo "ERRO: faltou <versao-alvo> (ex.: 0.2.0). Veja --help." >&2; exit 2; }
shift || true
NEW_TOOLCHAIN=""
while [ $# -gt 0 ]; do
  case "$1" in
    --toolchain) NEW_TOOLCHAIN="${2:-}"; shift 2 ;;
    *) echo "ERRO: argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
[ -n "$ROOT" ] || { echo "ERRO: rode dentro do repo." >&2; exit 2; }
cd "$ROOT" || exit 2

CUR="$(grep -oE '^servo[[:space:]]*=[[:space:]]*"=[^"]*"' crates/basedbrowser/Cargo.toml | sed -E 's/.*"=([^"]*)".*/\1/')"
REPORTS="$ROOT/scripts/update-servo/reports"
mkdir -p "$REPORTS"
STAMP="$(git rev-parse --short HEAD)"
REPORT="$REPORTS/bump-${CUR}_to_${TARGET}-${STAMP}.md"

# Worktree isolado (detached) — não toca o working tree atual.
WT="$(mktemp -d "${TMPDIR:-/tmp}/servo-bump.XXXXXX")"
cleanup() { git worktree remove --force "$WT" >/dev/null 2>&1; rm -rf "$WT" >/dev/null 2>&1; }
trap cleanup EXIT
echo ">> criando worktree isolado em $WT (HEAD detached)"
git worktree add --detach "$WT" HEAD >/dev/null 2>&1 || { echo "ERRO: git worktree add falhou." >&2; exit 2; }

# Aplica o pin-alvo SÓ no worktree (sed nos 2 crates que consomem o motor).
for f in crates/basedbrowser/Cargo.toml crates/servo-poc/Cargo.toml; do
  sed -i -E "s/^servo[[:space:]]*=[[:space:]]*\"=[^\"]*\"/servo = \"=$TARGET\"/" "$WT/$f"
done
# Sincroniza o archgate do worktree com o alvo (senão o próprio check barra o dry-run).
sed -i -E "s/^EXPECT_SERVO=\"=[^\"]*\"/EXPECT_SERVO=\"=$TARGET\"/" "$WT/scripts/checks/check-servo-pin.sh"
if [ -n "$NEW_TOOLCHAIN" ]; then
  sed -i -E "s/^channel[[:space:]]*=[[:space:]]*\"[^\"]*\"/channel = \"$NEW_TOOLCHAIN\"/" "$WT/rust-toolchain.toml"
  sed -i -E "s/^EXPECT_TOOLCHAIN=\"[^\"]*\"/EXPECT_TOOLCHAIN=\"$NEW_TOOLCHAIN\"/" "$WT/scripts/checks/check-servo-pin.sh"
fi

export CARGO_TARGET_DIR="$ROOT/target"   # reusa o cache (mozjs/servo já compilados quando a versão bate)
cd "$WT" || exit 2

# cargo update do servo p/ a versão exata (atualiza Cargo.lock no worktree). No-op se já bate.
echo ">> cargo update -p servo --precise $TARGET"
upd_log="$(cargo update -p servo --precise "$TARGET" 2>&1)"; upd_rc=$?

run_step() { # $1=nome  $2..=comando ; ecoa duracao + status
  local name="$1"; shift
  local t0 t1 rc
  t0=$(date +%s)
  echo ">> [$name] $*"
  "$@"; rc=$?
  t1=$(date +%s)
  printf '%s|%s|%s\n' "$name" "$rc" "$((t1 - t0))" >> "$WT/.steps"
  return "$rc"
}

: > "$WT/.steps"
OVERALL=0
T_ALL0=$(date +%s)
run_step "cargo update"  bash -c "exit $upd_rc"          || OVERALL=1
run_step "fmt"     cargo fmt --all --check                || OVERALL=1
run_step "build"   cargo build --workspace --exclude servo-poc || OVERALL=1
run_step "clippy"  cargo clippy --workspace --exclude servo-poc --all-targets -- -D warnings || OVERALL=1
run_step "test"    cargo test --workspace --exclude servo-poc  || OVERALL=1
run_step "archgate" bash scripts/checks/archgate.sh       || OVERALL=1
T_ALL1=$(date +%s); TOTAL=$((T_ALL1 - T_ALL0))

# Relatório (gitignored).
{
  echo "# Dry-run de bump do Servo: $CUR -> $TARGET"
  echo
  echo "- Base (HEAD): \`$STAMP\`  ·  Worktree isolado (não tocou a \`main\`)"
  [ -n "$NEW_TOOLCHAIN" ] && echo "- Toolchain alvo: $NEW_TOOLCHAIN"
  echo "- \`cargo update -p servo --precise $TARGET\` rc=$upd_rc"
  echo
  echo "| Passo | Status | Duração (s) |"
  echo "|-------|--------|-------------|"
  while IFS='|' read -r n rc d; do
    [ "$rc" = "0" ] && st="✅ ok" || st="❌ FALHOU (rc=$rc)"
    echo "| $n | $st | $d |"
  done < "$WT/.steps"
  echo
  echo "- **TOTAL (wall-clock do gate): ${TOTAL}s (~$((TOTAL / 60)) min)**"
  echo "- **Veredito vs meta Goal #3 (< 1 dia = 8 h = 28800 s): $([ "$OVERALL" = 0 ] && echo "VERDE" || echo "VERMELHO — triar churn")**"
  if [ "$upd_rc" -ne 0 ]; then echo; echo "## cargo update (saída)"; echo '```'; echo "$upd_log"; echo '```'; fi
} > "$REPORT"

echo
echo "============================================================"
cat "$REPORT"
echo "============================================================"
echo ">> relatório salvo em: $REPORT (gitignored)"
[ "$OVERALL" = 0 ] && echo ">> RESULTADO: gate VERDE no pin $TARGET." || echo ">> RESULTADO: gate VERMELHO — ver passos acima (churn de API a triar)."
exit "$OVERALL"
