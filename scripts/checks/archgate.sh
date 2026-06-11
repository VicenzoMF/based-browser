#!/usr/bin/env bash
# Archgate runner — roda todos os checks executaveis que acoplam ADRs/config protegida a uma regra
# mecanica (HARNESS-ROADMAP H3). Cada check em scripts/checks/check-*.sh emite erro-como-instrucao
# (ERRO / POR QUE / FIX / EXEMPLO) e sai !=0 ao falhar. Roda no gate local (lefthook) e no CI.
# Determinismo: ordem alfabetica dos checks; agrega o pior codigo de saida.
set -uo pipefail

dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rc=0
ran=0

shopt -s nullglob
for chk in "$dir"/check-*.sh; do
  ran=$((ran + 1))
  if ! bash "$chk"; then
    rc=1
  fi
done

if [ "$ran" -eq 0 ]; then
  echo "archgate: nenhum check encontrado (scripts/checks/check-*.sh)" >&2
  exit 2
fi

if [ "$rc" -eq 0 ]; then
  echo "archgate: $ran check(s) OK"
else
  echo "archgate: FALHOU (ver erros acima); corrija conforme a instrucao de cada check." >&2
fi
exit "$rc"
