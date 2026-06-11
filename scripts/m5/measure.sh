#!/usr/bin/env bash
# =============================================================================
# M5 — engine de medição de footprint (RSS/PSS) — BasedBrowser vs. Chrome.
#
# Mede a memória de UM alvo num estado controlado, somando a ÁRVORE DE PROCESSOS
# inteira (justo: Chrome é multiprocess; BasedBrowser é single-process — ver
# ADR-0008). Lança o alvo com perfil LIMPO, espera o settle, amostra /proc K vezes
# e reporta mean/median/min/max/stdev de RSS e PSS (kB). Emite JSON (stdout) +
# tabela legível (stderr).
#
# Uso:   measure.sh <basedbrowser|chrome> <n_tabs> <page_abs_path>
# Config (env): WARMUP SAMPLES REPS  BIN CHROME
# Requer: Linux com /proc/<pid>/smaps_rollup. NÃO usa o children-file (ausente
#         neste kernel) — caminha a árvore por PPID (/proc/<pid>/stat).
# =============================================================================
set -uo pipefail

# Locale C: garante que o awk use '.' como separador decimal (não ',' do pt-BR), senão
# o JSON sai inválido (ex.: "mean":100,0). Também estabiliza o `sort -n`.
export LC_ALL=C

WARMUP="${WARMUP:-6}"      # segundos antes de amostrar (load da página + settle idle)
SAMPLES="${SAMPLES:-5}"    # nº de amostras por execução (1/s); reporta a mediana
REPS="${REPS:-5}"          # execuções independentes (pass^k, Harness H4)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${BIN:-$ROOT_DIR/target/release/basedbrowser}"
CHROME="${CHROME:-google-chrome-stable}"

# ---------------------------------------------------------------------------
# Helpers de /proc (sem forks no caminho quente: usa `read`, não `cat`/`awk`).
# ---------------------------------------------------------------------------

# PPID de um pid: campo após o ÚLTIMO ')' do /proc/<pid>/stat (robusto a `comm`
# com espaços/parênteses). Imprime nada e retorna 1 se o pid sumiu.
ppid_of() {
  local stat after state ppid rest
  # Grupo com redirect: o `2>/dev/null` cobre TAMBÉM o erro de abrir o arquivo (o pid pode
  # sumir entre o glob e o read — race benigno), senão a mensagem escaparia p/ o stderr.
  { read -r stat < "/proc/$1/stat"; } 2>/dev/null || return 1
  after=${stat##*)}                  # " <state> <ppid> <pgrp> ..."
  read -r state ppid rest <<<"$after"
  printf '%s\n' "$ppid"
}

# Imprime todos os pids da subárvore enraizada em <root> (incluindo o root).
tree_pids() {
  local root="$1" d p pp
  declare -A parent=()
  for d in /proc/[0-9]*; do
    p=${d#/proc/}
    pp=$(ppid_of "$p") || continue
    parent[$p]=$pp
  done
  local -a queue=("$root") out=("$root")
  while ((${#queue[@]})); do
    local cur=${queue[0]}
    queue=("${queue[@]:1}")
    for p in "${!parent[@]}"; do
      if [[ ${parent[$p]} == "$cur" ]]; then
        out+=("$p")
        queue+=("$p")
      fi
    done
  done
  printf '%s\n' "${out[@]}"
}

# Soma Rss e Pss (kB) de smaps_rollup sobre a árvore de <root>.
# Imprime: "<rss_kb> <pss_kb> <npids>".
mem_of_tree() {
  local root="$1" rss=0 pss=0 n=0 p line key val
  while read -r p; do
    [[ -r "/proc/$p/smaps_rollup" ]] || continue
    local got_r=0 got_p=0
    while read -r key val _; do
      case "$key" in
        Rss:) rss=$((rss + val)); got_r=1 ;;
        Pss:) pss=$((pss + val)); got_p=1 ;;
      esac
      ((got_r && got_p)) && break
    done < "/proc/$p/smaps_rollup"
    ((got_r)) && n=$((n + 1))
  done < <(tree_pids "$root")
  printf '%s %s %s\n' "$rss" "$pss" "$n"
}

# Mediana inteira de uma lista de inteiros (argumentos).
median() {
  local -a s
  mapfile -t s < <(printf '%s\n' "$@" | sort -n)
  local c=${#s[@]}
  ((c == 0)) && { printf '0\n'; return; }
  printf '%s\n' "${s[$((c / 2))]}"
}

# mean median min max stdev (floats, 1 casa) de uma lista. Pré-ordena com `sort -n`
# (a entrada chega ordenada → mediana = elemento do meio), evitando `asort` (gawk-only;
# o awk padrão aqui é mawk).
stats() {
  printf '%s\n' "$@" | sort -n | awk '
    { v[NR]=$1; sum+=$1 }
    END {
      n=NR; if(n==0){print "0 0 0 0 0"; exit}
      mean=sum/n;
      med = (n%2) ? v[(n+1)/2] : (v[n/2]+v[n/2+1])/2;
      mn=v[1]; mx=v[n];
      ss=0; for(i=1;i<=n;i++){d=v[i]-mean; ss+=d*d}
      sd=(n>1)?sqrt(ss/(n-1)):0;
      printf "%.1f %.1f %.1f %.1f %.1f", mean, med, mn, mx, sd
    }'
}

# ---------------------------------------------------------------------------
# Lança o alvo (perfil limpo) em background. Imprime "<pid>:<tmpdir>".
# ---------------------------------------------------------------------------
launch_target() {
  local target="$1" n="$2" page="$3" url cfg
  url="file://$page"
  cfg="$(mktemp -d)"
  case "$target" in
    basedbrowser)
      XDG_CONFIG_HOME="$cfg" BASEDBROWSER_URL="$url" BASEDBROWSER_OPEN_TABS="$n" \
        "$BIN" >>"$cfg/stdout.log" 2>&1 &
      ;;
    chrome)
      local -a urls=()
      local i
      for ((i = 0; i < n; i++)); do urls+=("$url"); done
      "$CHROME" --user-data-dir="$cfg" --no-first-run --no-default-browser-check \
        --disable-extensions --disable-component-update --no-service-autorun \
        "${urls[@]}" >>"$cfg/stdout.log" 2>&1 &
      ;;
    *)
      printf 'measure.sh: alvo desconhecido: %s\n' "$target" >&2
      return 1
      ;;
  esac
  printf '%s:%s\n' "$!" "$cfg"
}

# Mata a árvore inteira (coleta os pids ANTES de matar) e remove o tmpdir.
kill_tree() {
  local root="$1" p
  local -a pids
  mapfile -t pids < <(tree_pids "$root")
  for p in "${pids[@]}"; do kill -TERM "$p" 2>/dev/null; done
  sleep 1
  for p in "${pids[@]}"; do kill -KILL "$p" 2>/dev/null; done
  wait "$root" 2>/dev/null
}

# Uma execução: lança → settle → amostra → mata. Imprime "<rss_kb> <pss_kb> <npids>".
measure_once() {
  local target="$1" n="$2" page="$3" launched pid cfg
  launched="$(launch_target "$target" "$n" "$page")" || return 1
  pid=${launched%%:*}
  cfg=${launched#*:}
  local result="0 0 0"
  if kill -0 "$pid" 2>/dev/null; then
    sleep "$WARMUP"
    local -a rss_list=() pss_list=()
    local i out r rest s np
    for ((i = 0; i < SAMPLES; i++)); do
      kill -0 "$pid" 2>/dev/null || break
      out="$(mem_of_tree "$pid")"
      r=${out%% *}
      rest=${out#* }
      s=${rest%% *}
      np=${rest##* }
      rss_list+=("$r")
      pss_list+=("$s")
      ((i < SAMPLES - 1)) && sleep 1
    done
    if ((${#pss_list[@]})); then
      result="$(median "${rss_list[@]}") $(median "${pss_list[@]}") ${np:-0}"
    fi
  fi
  kill_tree "$pid"
  rm -rf "$cfg"
  printf '%s\n' "$result"
}

# ---------------------------------------------------------------------------
# main: mede uma célula (target × n × page) com REPS execuções e emite JSON.
# ---------------------------------------------------------------------------
main() {
  local target="${1:?uso: measure.sh <basedbrowser|chrome> <n_tabs> <page_path>}"
  local n="${2:?n_tabs obrigatório}"
  local page="${3:?page_path obrigatório}"
  [[ -f "$page" ]] || { printf 'measure.sh: página não encontrada: %s\n' "$page" >&2; return 1; }
  if [[ "$target" == basedbrowser && ! -x "$BIN" ]]; then
    printf 'measure.sh: binário release ausente: %s (rode: cargo build --release -p basedbrowser)\n' "$BIN" >&2
    return 1
  fi

  local pagename; pagename="$(basename "$page" .html)"
  printf '[m5] medindo %s tabs=%s page=%s reps=%s warmup=%ss samples=%s\n' \
    "$target" "$n" "$pagename" "$REPS" "$WARMUP" "$SAMPLES" >&2

  local -a rss_reps=() pss_reps=()
  local npids=0 rep out r rest s np
  for ((rep = 1; rep <= REPS; rep++)); do
    out="$(measure_once "$target" "$n" "$page")"
    r=${out%% *}; rest=${out#* }; s=${rest%% *}; np=${rest##* }
    rss_reps+=("$r"); pss_reps+=("$s"); npids="$np"
    printf '[m5]   rep %s/%s: rss=%s kB pss=%s kB npids=%s\n' "$rep" "$REPS" "$r" "$s" "$np" >&2
  done

  read -r rss_mean rss_med rss_min rss_max rss_sd < <(stats "${rss_reps[@]}")
  read -r pss_mean pss_med pss_min pss_max pss_sd < <(stats "${pss_reps[@]}")

  printf '[m5]   => PSS median=%s kB (%.1f MiB)  RSS median=%s kB (%.1f MiB)  npids=%s\n' \
    "$pss_med" "$(awk "BEGIN{print $pss_med/1024}")" \
    "$rss_med" "$(awk "BEGIN{print $rss_med/1024}")" "$npids" >&2

  # JSON de uma linha (machine-readable; o run.sh agrega).
  printf '{"target":"%s","tabs":%s,"page":"%s","reps":%s,"npids":%s,' \
    "$target" "$n" "$pagename" "$REPS" "$npids"
  printf '"pss_kb":{"mean":%s,"median":%s,"min":%s,"max":%s,"stdev":%s},' \
    "$pss_mean" "$pss_med" "$pss_min" "$pss_max" "$pss_sd"
  printf '"rss_kb":{"mean":%s,"median":%s,"min":%s,"max":%s,"stdev":%s},' \
    "$rss_mean" "$rss_med" "$rss_min" "$rss_max" "$rss_sd"
  printf '"pss_samples":[%s],"rss_samples":[%s]}\n' \
    "$(IFS=,; echo "${pss_reps[*]}")" "$(IFS=,; echo "${rss_reps[*]}")"
}

# Só roda main() quando executado diretamente (permite `source` p/ testar as funções).
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi
