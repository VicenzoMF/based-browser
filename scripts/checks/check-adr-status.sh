#!/usr/bin/env bash
# Archgate: todo ADR (docs/adr/NNNN-*.md) deve declarar um Status (Proposed|Accepted|Superseded|
# Deprecated) — ver docs/adr/README.md. ADRs sao a verdade vigente que o agente trata como fato; um
# ADR sem status e um registro mal-formado. Erro-como-instrucao (HARNESS-ROADMAP H3).
set -uo pipefail

root="$(git rev-parse --show-toplevel 2>/dev/null)"
[ -n "$root" ] || root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root" || { echo "check-adr-status: nao achei a raiz do repo" >&2; exit 2; }

bad=0
shopt -s nullglob
for adr in docs/adr/[0-9]*.md; do
  if ! grep -qE '^[-*]?[[:space:]]*\*?\*?Status:?\*?\*?' "$adr"; then
    if [ "$bad" -eq 0 ]; then
      cat >&2 <<'EOF'
------------------------------------------------------------------------
ERRO (archgate): ADR sem linha de Status.
POR QUE: ADRs sao registros imutaveis e datados que o agente trata como verdade
  vigente; um ADR sem Status (Proposed|Accepted|Superseded|Deprecated) e mal-formado.
FIX: adicione uma linha de Status ao ADR (ver docs/adr/README.md).
EXEMPLO: "- **Status:** Accepted"
ADRs afetados:
EOF
    fi
    echo "  - $adr" >&2
    bad=1
  fi
done

[ "$bad" -eq 0 ] || { echo "------------------------------------------------------------------------" >&2; exit 2; }
echo "check-adr-status: OK (todos os ADRs tem Status)"
exit 0
