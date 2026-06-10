#!/usr/bin/env bash
# PreToolUse (Bash): barra bypass de gates e pede confirmacao em rm recursivo/forcado.
# Complementa o permissions.deny do settings.json (que ja barra sudo rm e rm -rf /*).
set -uo pipefail
input="$(cat)"
cmd="$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null)"
[ -z "$cmd" ] && exit 0

# --no-verify burla os gates de pre-commit -> bloqueio duro
if printf '%s' "$cmd" | grep -Eq -- '--no-verify'; then
  echo "Bloqueado: --no-verify burla os gates de pre-commit (proibido por AGENTS.md)." >&2
  exit 2
fi

# rm recursivo/forcado -> pede confirmacao humana
if printf '%s' "$cmd" | grep -Eq -- 'rm[[:space:]]+-[A-Za-z]*r[A-Za-z]*f|rm[[:space:]]+-[A-Za-z]*f[A-Za-z]*r'; then
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"rm recursivo/forcado detectado. Confirme o alvo antes de prosseguir."}}\n'
  exit 0
fi

exit 0
