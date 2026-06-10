#!/usr/bin/env bash
# PreToolUse (Edit|Write|MultiEdit): protege a config do harness.
#  - ADRs existentes    -> "deny" (imutaveis; crie um ADR novo que o supersede)
#  - configs sensiveis  -> "ask"  (humano confirma; mudanca estrutural pede ADR)
# Reasons em ASCII simples (sem aspas/quebras) para JSON seguro.
set -uo pipefail
input="$(cat)"
fp="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null)"
[ -z "$fp" ] && exit 0
proj="${CLAUDE_PROJECT_DIR:-$PWD}"

emit() { # $1=decision  $2=reason
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"%s","permissionDecisionReason":"%s"}}\n' "$1" "$2"
  exit 0
}

# ADRs existentes sao imutaveis (criar ADR novo continua liberado)
case "$fp" in
  */docs/adr/*.md)
    if [ -f "$fp" ]; then
      emit deny "ADRs sao imutaveis. Nao edite este ADR; crie um novo que o supersede (ver docs/adr/README.md)."
    fi
    ;;
esac

# Config protegida -> pede confirmacao humana
case "$fp" in
  */rust-toolchain.toml)
    emit ask "rust-toolchain.toml e config protegida (ver ADR-0001). Trocar a toolchain pede ADR. Confirmar?" ;;
  */.claude/settings.json|*/.claude/hooks/*)
    emit ask "Voce esta alterando a config/hooks do harness. Confirme a mudanca conscientemente." ;;
esac
if [ "$fp" = "$proj/Cargo.toml" ]; then
  emit ask "Editando o Cargo.toml raiz (lints do workspace). Mudanca nos lints/estrutura pede ADR. Confirmar?"
fi

exit 0
