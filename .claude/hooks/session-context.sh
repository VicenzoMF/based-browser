#!/usr/bin/env bash
# SessionStart: injeta contexto enxuto (commits recentes + trabalho atual + ponteiros).
# Mantido pequeno de proposito (o doc [A] alerta contra inchar o contexto).
set -uo pipefail
cd "${CLAUDE_PROJECT_DIR:-$PWD}" 2>/dev/null || exit 0
echo "## BasedBrowser - contexto de sessao (auto)"
echo "Commits recentes:"
git log --oneline -5 2>/dev/null || true
if [ -f .specs/project/STATE.md ]; then
  echo ""
  grep -m1 '^\*\*Current Work:' .specs/project/STATE.md 2>/dev/null || true
fi
echo ""
echo "Plano: .specs/project/{ROADMAP,HARNESS-ROADMAP,STATE}.md - ADRs imutaveis: docs/adr/ - guia: AGENTS.md"
exit 0
