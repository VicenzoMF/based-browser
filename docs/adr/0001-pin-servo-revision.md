# ADR-0001: Fixar uma revisão do Servo

- **Status:** Proposed (revisão exata a definir no M0)
- **Data:** 2026-06-10

## Contexto
Servo não é um crate do crates.io; é consumido contra a árvore de código e sua API muda
rápido. O Verso (browser construído sobre Servo) foi arquivado em 2026 por não conseguir
acompanhar esse churn com pouca mão de obra.

## Decisão
Fixar uma **revisão git específica** do Servo e atualizá-la apenas em "sprints de update"
deliberados, guiados por runbook (HARNESS-ROADMAP, fase H3). A revisão fixada e o
`rust-toolchain.toml` são **config protegida**: só mudam via ADR novo.

## Consequências
- (+) Builds reproduzíveis; updates do Servo passam a **falhar alto** (testes quebram) em vez
  de degradar em silêncio.
- (−) Ficamos atrás do upstream entre sprints.
- **Pendente (M0):** definir a revisão e promover este ADR para `Accepted` — ou criar um
  ADR-0002 que o *supersede* já com a revisão escolhida.
