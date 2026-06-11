#!/usr/bin/env bash
# Archgate: o pin do Servo e a toolchain Rust sao CONFIG PROTEGIDA (ADR-0002; runbook ADR-0011).
# Este check ACOPLA o ADR a uma regra executavel (ADR <-> check, HARNESS-ROADMAP H3): se o pin/
# toolchain divergir do valor decidido, o gate/CI falha ALTO com instrucao de correcao — em vez de
# um bump silencioso reintroduzir o risco L-001 (o Verso morreu afogado no churn do Servo).
#
# Para BUMPAR o pin de forma legitima: docs/runbooks/atualizar-servo.md cria um ADR novo que supersede
# o ADR-0002 E atualiza os VALORES ESPERADOS abaixo (a edicao deste arquivo + o ADR sao a prova da
# decisao consciente). Nao edite o pin solto nos Cargo.toml.
set -uo pipefail

# --- Valores decididos (sincronizados com o ADR vigente: ADR-0002) ---
EXPECT_SERVO="=0.2.0"
EXPECT_TOOLCHAIN="1.92.0"
PIN_FILES=("crates/basedbrowser/Cargo.toml" "crates/servo-poc/Cargo.toml")

root="$(git rev-parse --show-toplevel 2>/dev/null)"
[ -n "$root" ] || root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root" || { echo "check-servo-pin: nao achei a raiz do repo" >&2; exit 2; }

fail() { # $1=titulo  $2=detalhe  $3=exemplo
  cat >&2 <<EOF
------------------------------------------------------------------------
ERRO (archgate): $1
  $2
POR QUE: o pin do Servo / a toolchain sao CONFIG PROTEGIDA (ADR-0002). Um bump
  silencioso reintroduz o risco existencial L-001 (churn de upstream — licao do Verso).
FIX: para atualizar legitimamente, siga docs/runbooks/atualizar-servo.md — ele cria um
  ADR novo que supersede o ADR-0002 E atualiza os valores ESPERADOS em
  scripts/checks/check-servo-pin.sh. Nao edite o pin solto.
EXEMPLO (estado correto): $3
------------------------------------------------------------------------
EOF
  exit 2
}

extract() { # $1=arquivo  $2=chave(servo|channel) -> valor entre aspas, ou vazio
  grep -oE "^$2[[:space:]]*=[[:space:]]*\"[^\"]*\"" "$1" 2>/dev/null | head -1 | sed -E 's/.*"([^"]*)".*/\1/'
}

# 1) pin do servo nos crates que consomem o motor
for f in "${PIN_FILES[@]}"; do
  [ -f "$f" ] || fail "arquivo de pin ausente" "esperava encontrar $f" "servo = \"$EXPECT_SERVO\""
  got="$(extract "$f" servo)"
  [ "$got" = "$EXPECT_SERVO" ] || \
    fail "pin do Servo divergente em $f" "esperado servo = \"$EXPECT_SERVO\", achei: \"${got:-<ausente>}\"" "servo = \"$EXPECT_SERVO\""
done

# 2) toolchain pinada
tf="rust-toolchain.toml"
[ -f "$tf" ] || fail "rust-toolchain.toml ausente" "esperava encontrar $tf" "channel = \"$EXPECT_TOOLCHAIN\""
got="$(extract "$tf" channel)"
[ "$got" = "$EXPECT_TOOLCHAIN" ] || \
  fail "toolchain divergente em $tf" "esperado channel = \"$EXPECT_TOOLCHAIN\", achei: \"${got:-<ausente>}\"" "channel = \"$EXPECT_TOOLCHAIN\""

echo "check-servo-pin: OK (servo $EXPECT_SERVO nos ${#PIN_FILES[@]} crates; toolchain $EXPECT_TOOLCHAIN)"
exit 0
