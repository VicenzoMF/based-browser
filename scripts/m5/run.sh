#!/usr/bin/env bash
# =============================================================================
# M5 — runner da matriz de medição (ver ADR-0008).
#
# Roda {basedbrowser, chrome} × {ocioso N∈TABS_LIST; opc. heavy N=1} × REPS, na
# MESMA metodologia (perfil limpo, headful, soma da árvore de processos), e emite:
#   - results-<stamp>.jsonl  (uma linha JSON por célula — proveniência reproduzível)
#   - summary-<stamp>.md      (tabela comparativa: ocioso, custo por-aba, ratios)
# A manchete (números) vai p/ o ADR-0008 datado; este script é a fonte reexecutável.
#
# Uso:    scripts/m5/run.sh [outdir]
# Config (env): REPS WARMUP SAMPLES TABS_LIST TARGETS HEAVY BIN CHROME
# =============================================================================
set -uo pipefail
export LC_ALL=C

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MEASURE="$HERE/measure.sh"
PAGES="$HERE/pages"
BIN="${BIN:-$ROOT/target/release/basedbrowser}"

REPS="${REPS:-5}"
WARMUP="${WARMUP:-8}"
SAMPLES="${SAMPLES:-5}"
TABS_LIST="${TABS_LIST:-1 3 6}"     # 1º valor = baseline ocioso; demais = custo por-aba
TARGETS="${TARGETS:-basedbrowser chrome}"
HEAVY="${HEAVY:-1}"                  # 1 = inclui o estado "página pesada"

OUTDIR="${1:-$HERE/results}"
mkdir -p "$OUTDIR"
STAMP="$(date +%Y%m%d-%H%M%S)"
JSONL="$OUTDIR/results-$STAMP.jsonl"
SUMMARY="$OUTDIR/summary-$STAMP.md"

mib()   { awk "BEGIN{printf \"%.1f\", ${1:-0}/1024}"; }
ratio() { awk "BEGIN{ if(${2:-0}==0){print \"-\"} else printf \"%.2f\", ${1:-0}/${2:-1} }"; }

# Garante o binário release (1ª build recompila o motor — L-005; depois fica cacheado).
if [[ ! -x "$BIN" ]]; then
  echo "[m5] binário release ausente; rodando cargo build --release -p basedbrowser" >&2
  (cd "$ROOT" && cargo build --release -p basedbrowser) || { echo "[m5] build falhou" >&2; exit 1; }
fi

declare -A PSS RSS NP
extract() { sed -n "s/.*\"$1\":\\([0-9.]*\\).*/\\1/p" <<<"$2"; }      # campo top-level numérico
extract_med() { sed -n "s/.*\"$1\":{\"mean\":[^,]*,\"median\":\\([0-9.]*\\).*/\\1/p" <<<"$2"; }

run_cell() {  # target n page key
  local target="$1" n="$2" page="$3" key="$4" json
  json="$(REPS="$REPS" WARMUP="$WARMUP" SAMPLES="$SAMPLES" BIN="$BIN" \
          bash "$MEASURE" "$target" "$n" "$page")" || { echo "[m5] célula falhou: $key" >&2; return 1; }
  printf '%s\n' "$json" >> "$JSONL"
  PSS[$key]="$(extract_med pss_kb "$json")"
  RSS[$key]="$(extract_med rss_kb "$json")"
  NP[$key]="$(extract npids "$json")"
}

: > "$JSONL"
echo "[m5] matriz: targets=[$TARGETS] tabs=[$TABS_LIST] heavy=$HEAVY reps=$REPS  → $OUTDIR" >&2

BASE_N="${TABS_LIST%% *}"          # primeiro
MAX_N="${TABS_LIST##* }"           # último

for target in $TARGETS; do
  for n in $TABS_LIST; do
    run_cell "$target" "$n" "$PAGES/idle.html" "${target}_idle_${n}"
  done
  [[ "$HEAVY" == 1 ]] && run_cell "$target" 1 "$PAGES/heavy.html" "${target}_heavy_1"
done

# --------------------------- relatório markdown ----------------------------
{
  echo "# M5 — Resultados: footprint BasedBrowser vs. Chrome"
  echo
  echo "- **Data:** $STAMP · **Reps:** $REPS · **Warmup:** ${WARMUP}s · **Samples/rep:** $SAMPLES"
  echo "- **Metodologia:** headful, perfil limpo, soma da ÁRVORE DE PROCESSOS, mediana por rep. PSS = métrica-título."
  echo "- **Página ociosa:** \`pages/idle.html\` (estática, sem rede/JS)."
  echo
  echo "## Ocioso (N=$BASE_N aba)"
  echo
  echo "| Target | nº processos | PSS (MiB) | RSS (MiB) |"
  echo "|---|--:|--:|--:|"
  for target in $TARGETS; do
    k="${target}_idle_${BASE_N}"
    echo "| $target | ${NP[$k]:-?} | $(mib "${PSS[$k]:-0}") | $(mib "${RSS[$k]:-0}") |"
  done
  echo
  if [[ "$TARGETS" == *basedbrowser* && "$TARGETS" == *chrome* ]]; then
    bb="basedbrowser_idle_${BASE_N}"; cr="chrome_idle_${BASE_N}"
    echo "**Chrome / BasedBrowser (ocioso):** PSS ×$(ratio "${PSS[$cr]:-0}" "${PSS[$bb]:-1}") · RSS ×$(ratio "${RSS[$cr]:-0}" "${RSS[$bb]:-1}")"
    echo
  fi
  echo "## Custo por-aba (página \`idle.html\`, mesma página)"
  echo
  echo "| Target | $(for n in $TABS_LIST; do printf 'PSS N=%s | ' "$n"; done)marginal/aba (MiB) |"
  echo "|---|$(for n in $TABS_LIST; do printf -- '--:|'; done)--:|"
  for target in $TARGETS; do
    row="| $target |"
    for n in $TABS_LIST; do row+=" $(mib "${PSS[${target}_idle_${n}]:-0}") |"; done
    base="${PSS[${target}_idle_${BASE_N}]:-0}"; top="${PSS[${target}_idle_${MAX_N}]:-0}"
    span=$(( MAX_N - BASE_N )); (( span < 1 )) && span=1
    marg=$(awk "BEGIN{printf \"%.1f\", (($top)-($base))/1024/$span}")
    row+=" $marg |"
    echo "$row"
  done
  echo
  echo "_marginal/aba = (PSS(N=$MAX_N) − PSS(N=$BASE_N)) / ($MAX_N−$BASE_N), em MiB de PSS._"
  echo
  if [[ "$HEAVY" == 1 ]]; then
    echo "## Página pesada (\`heavy.html\`, N=1)"
    echo
    echo "| Target | nº processos | PSS (MiB) | RSS (MiB) |"
    echo "|---|--:|--:|--:|"
    for target in $TARGETS; do
      k="${target}_heavy_1"
      echo "| $target | ${NP[$k]:-?} | $(mib "${PSS[$k]:-0}") | $(mib "${RSS[$k]:-0}") |"
    done
    echo
  fi
  echo "_JSON bruto (proveniência): \`$(basename "$JSONL")\`._"
} | tee "$SUMMARY"

echo "[m5] pronto: $SUMMARY" >&2
