# ADR-0008: M5 — Footprint de memória vs. Chromium (metodologia + veredito da tese)

- **Status:** Accepted
- **Data:** 2026-06-11
- **Relaciona-se com:** valida o **Goal #1 do PROJECT** ("footprint enxuto; medir RSS ocioso
  vs. Chromium e documentar a diferença"). Mantém o pin do ADR-0002 (`servo =0.2.0`) e a
  arquitetura do M3 (ADR-0005/0006, render GPU) e M4 (ADR-0007, multi-aba). Não altera config
  protegida; única mudança de produto = o hook env `BASEDBROWSER_OPEN_TABS` (medição).

## Contexto

A razão de existir do BasedBrowser é a tese **"motor Rust-native ⇒ footprint menor que um
browser Chromium"** (PROJECT.md). M0–M4 entregaram um browser multi-aba dirigível, mas a tese
**nunca tinha sido medida**. M5 é um harness de medição reproduzível + uma metodologia justa +
este relatório datado, que **valida ou refuta** a tese com números.

### Modelo de processos (decide o que é "justo") — confirmado na FONTE

- `Opts.multiprocess` default = `false` (`servo-config-0.2.0/opts.rs:220`); nosso
  `ServoBuilder::default()` (`src/main.rs`) não o altera ⇒ **BasedBrowser roda SINGLE-PROCESS**:
  constellation, script (SpiderMonkey), layout, paint, net são *threads* no MESMO PID, junto
  com o wgpu/Vulkan e o Slint. Medição confirmou: **`npids = 1`**.
- **Chrome é multiprocess** (browser + zygote + GPU + N renderers + utility + crashpad).
  Medição confirmou: **13 processos** ocioso, crescendo com as abas (15 @3, 18 @6).
- ⇒ Comparação maçã-com-maçã = **somar a ÁRVORE DE PROCESSOS inteira nos dois** e usar **PSS**
  (Proportional Set Size) como métrica-título: o PSS divide cada página compartilhada entre os
  processos que a mapeiam, sendo justo para uma árvore multiprocess e para libs de sistema
  compartilhadas. O **RSS é reportado junto**, mas infla o Chrome (conta páginas compartilhadas
  ~13×) — por isso não é a manchete.

## Decisão (metodologia — candidata a re-execução determinística)

1. **Harness** = bash em `scripts/m5/` (mantém o crate do produto fino — L-001):
   - `measure.sh <target> <n_tabs> <page>`: lança o alvo com **perfil limpo**, espera o
     **settle**, amostra `/proc/<pid>/smaps_rollup` (`Rss:`/`Pss:`) somando a árvore de
     processos, mata a árvore, repete **K vezes** e reporta mean/median/min/max/stdev (JSON).
     A árvore é caminhada por **PPID** (`/proc/<pid>/stat`, campo após o último `)`), porque o
     `children`-file (`CONFIG_PROC_CHILDREN`) está ausente neste kernel.
   - `run.sh`: roda a matriz `{basedbrowser, chrome} × {ocioso N∈{1,3,6}; pesada N=1}` e emite a
     tabela comparativa + JSONL de proveniência.
2. **Perfil limpo nos dois:** Chrome via `--user-data-dir=$tmp`; BasedBrowser via
   `XDG_CONFIG_HOME=$tmp` (o `dirs` honra ⇒ sem restauração de sessão/histórico interferindo).
3. **Headful nos dois** — BasedBrowser é uma janela Vulkan real com processo de GPU; Chrome
   headless pularia compositor/GPU e mediria outra coisa.
4. **Release** (L-005): BasedBrowser medido em `--release` (debug + métrica engana).
5. **Estabilidade (Harness H4):** warmup 8 s, 5 amostras/execução (mediana), **K = 5 execuções**
   independentes (pass^k). Reporta a **mediana** entre execuções (robusta a outliers — ver abaixo).
6. **Páginas determinísticas** (sem rede/variância): `scripts/m5/pages/idle.html` (estática
   mínima) e `heavy.html` (400 cards estilizados, estática).
7. **Custo por-aba** = N abas da MESMA página; marginal = `(PSS(N=6) − PSS(N=1)) / 5`.

## Resultados (release, headful, K=5, 2026-06-11; soma da árvore de processos)

### Ocioso (1 aba, `idle.html`)

| Target | nº processos | PSS (MiB) | RSS (MiB) |
|---|--:|--:|--:|
| **BasedBrowser** | **1** | **171.1** | 221.0 |
| Chrome (stable) | 13 | 314.7 | 1156.9 |

**Chrome / BasedBrowser:** **PSS ×1.84** · RSS ×5.24. Estabilidade alta: σ(PSS) ≈ 1.8 MiB
(BasedBrowser) e ≈ 0.5 MiB (Chrome) sobre 5 execuções.

### Custo marginal por-aba (`idle.html`, mesma página)

| Target | PSS N=1 | PSS N=3 | PSS N=6 | **marginal/aba** |
|---|--:|--:|--:|--:|
| **BasedBrowser** | 171.1 | 182.2 | 198.4 | **~5.5 MiB** |
| Chrome | 314.7 | 338.6 | 373.6 | ~11.8 MiB |

BasedBrowser adiciona **~2.1× menos memória por aba** — coerente com o single-process (engine
compartilhado; a aba custa pipeline/script-context/layout). O Chrome forka um renderer por aba.

### Página pesada (`heavy.html`, 400 cards, 1 aba)

| Target | nº processos | PSS (MiB) | RSS (MiB) | Δ vs. ocioso (PSS) |
|---|--:|--:|--:|--:|
| **BasedBrowser** | 1 | 205.4 | 255.8 | +34.3 |
| Chrome | 13 | 333.0 | 1180.6 | +18.3 |

BasedBrowser permanece mais leve no total (205 vs 333 MiB), **mas** a página pesada custa-lhe
**mais memória incremental** (+34 vs +18 MiB) — sinal honesto de que o layout/estilo do Servo é
menos econômico por-página que o do Chrome (ou o baseline do Chrome já amortiza mais). Não inverte
o veredito, mas qualifica-o.

### Nota de estabilidade (honestidade)

O caso BasedBrowser N=6 teve **1 outlier** em 5 execuções (96.7 MiB vs ~203 MiB nas outras 4) —
o settle de 8 s ocasionalmente amostrou antes das 6 abas terminarem de carregar. A **mediana
(198.4 MiB) descarta o outlier**; foi por isso que a metodologia reporta mediana, não média (a
média cairia para 177 MiB, enganosa). Os demais casos têm σ < 1% (muito estáveis).

## Veredito

- **Tese VALIDADA no núcleo:** o BasedBrowser é **mais leve que o Chrome em todos os estados
  medidos** — ocioso (**1.84× menos PSS**, 5.24× menos RSS), por-aba (**2.1× mais barato**) e
  com página pesada (1.6× menos PSS). O motor Rust-native single-process entrega um footprint
  menor que a árvore multiprocess do Chromium. A razão de existir do projeto **se sustenta com
  evidência.**
- **"Ordens de magnitude" do PROJECT — REFUTADO/qualificado:** o Goal #1 dizia "baseline ordens
  de magnitude menor". Na métrica justa (**PSS**), a diferença ociosa é **~1.8×, não 10×**. O
  número de RSS (×5.2) é grande mas **inflado** pela contagem múltipla de páginas compartilhadas
  na árvore do Chrome — não é a manchete honesta. A manchete honesta é: **~1.8× mais leve no
  ocioso (PSS); ~2.1× mais barato por-aba.**
- **Absoluto:** 171 MiB ociosos **não** são "featherweight" — o single-process carrega o stack
  inteiro (SpiderMonkey + layout + wgpu/Vulkan + Slint) num PID. O ganho é **relativo ao Chrome**,
  real e reproduzível, porém moderado em PSS. Otimizar o baseline absoluto fica para o futuro
  (fora do escopo do M5, que MEDE e documenta).

## Alternativas / pontos rejeitados ou adiados

- **Relatório interno do Servo (`Servo::create_memory_report` → `MemoryReportResult`) — ADIADO.**
  Está acessível sem deps novas (`servo` re-exporta `profile_traits`, `lib.rs:54`), e daria o
  breakdown (JS heap/layout/etc.) cruzável com o RSS do SO. Foi **deliberadamente não cabeado**
  para honrar o L-001 (embedding fino; o Verso morreu de churn): adicionaria 4+ superfícies de
  API de um crate interno (`create_memory_report`/`GenericCallback`/`MemoryReportResult`/`Report`),
  mais propenso a churn que a API estável de `WebView`. O veredito **não depende** dele — o
  `/proc` responde exatamente o Goal #1, e o **custo marginal por-aba** já dá um sinal de
  decomposição empírico e diretamente comparável. Caminho registrado para um M5.1/M6 futuro.
- **Chromium (snap) como baseline:** rejeitado a favor do `google-chrome-stable` (.deb) — o
  confinamento do snap adiciona processos/overhead que tornariam a comparação menos justa.
- **Headless:** rejeitado — pula o caminho de GPU/compositor; não representa o uso real (e o
  BasedBrowser não tem modo headless).

## Reprodução

```
cargo build --release -p basedbrowser
scripts/m5/run.sh              # matriz completa (K=5); imprime a tabela + JSONL de proveniência
# ou uma célula:
REPS=5 WARMUP=8 SAMPLES=5 scripts/m5/measure.sh basedbrowser 1 scripts/m5/pages/idle.html
```

A saída de `scripts/m5/results/` é transitória (gitignorada); **os números canônicos vivem aqui**
(ADR datado e imutável — design-for-rot do HARNESS-ROADMAP).

## Fontes (jun/2026)

- Servo 0.2.0 (cache do cargo): `servo-config-0.2.0/opts.rs:214-220` (`multiprocess: false`
  default), `servo-0.2.0/servo.rs:858/958/1027` (ramos single/multiprocess + `create_memory_report`),
  `servo-profile-traits-0.2.0/mem.rs:261-276` (`MemoryReportResult`/`MemoryReport`/`Report`),
  `servo-0.2.0/lib.rs:54` (`pub use profile_traits`).
- Linux: `/proc/<pid>/smaps_rollup` (`Rss`/`Pss`), `/proc/<pid>/stat` (PPID); `man 5 proc`.
- Harness: `scripts/m5/{measure.sh,run.sh,pages/}`. Run de validação: `20260611-101058`.
