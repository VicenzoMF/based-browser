# ADR-0002: Fixar Servo em 0.2.0 (crates.io) + toolchain Rust 1.92.0

- **Status:** Accepted
- **Data:** 2026-06-10
- **Supersede:** ADR-0001 (que permanece `Proposed` por imutabilidade do registro — o hook
  `protect-config.sh` nega editar ADRs existentes; esta decisão o substitui e o concretiza).

## Contexto

O ADR-0001 decidiu *fixar uma revisão do Servo*, mas deixou a revisão exata e a toolchain
"a definir no M0". A pesquisa do M0 (jun/2026, fontes oficiais) atualizou o cenário:

- O Servo passou a ser **publicado no crates.io** — o crate `libservo` foi renomeado para `servo`
  (PR servo/servo#43141). Linha **LTS `0.1.x`** (abr/2026, ~9 meses de patches) e **feature release
  `0.2.0`** (publicado em 05/jun/2026).
- A toolchain deixou de ser nightly: o `rust-toolchain.toml` do tag `v0.2.0` fixa **stable `1.92.0`**.
- Recursos passam a ser **embutidos por padrão** (`servo-default-resources`, feature
  `baked-in-resources`, PR #43182) — sem pasta `resources/` externa no caso simples.
- A doc oficial de embedding confirma que dá para buildar com `cargo` puro, mas **as dependências
  de sistema continuam obrigatórias** (gstreamer/X11/vulkan/clang/llvm/cmake) e o SpiderMonkey/mozjs
  compila do fonte; o `mach` apenas seta env vars/ativa features.

## Decisão

1. **Pin:** consumir o Servo como dependência de **crates.io**, fixada em **`servo = "=0.2.0"`**
   (versão exata; sem auto-bump). Atualizações só em "sprints de update" deliberados (runbook H3).
2. **Toolchain:** `rust-toolchain.toml` fixado em **`channel = "1.92.0"`** (a toolchain que o tag
   `v0.2.0` do Servo declara), com componentes `rustfmt` e `clippy`.
3. **Consumo fino (lição do Verso, STATE L-001):** depender do crate publicado e versionado em vez de
   um `rev` git de árvore inteira; manter o código de embedding mínimo.
4. **Config protegida:** o pin do Servo (versão no `Cargo.toml` do crate de embedding) e o
   `rust-toolchain.toml` só mudam via **ADR novo** que supersede este.

Escolheu-se `0.2.0` (e não a linha LTS `0.1.x`) por ser o M0 puro de-risking: a API de embedding mais
nova é mais ergonômica e casa exatamente com o exemplo `winit_minimal.rs` atual, reduzindo o atrito
até "pixels na tela". O pin pode migrar para a linha LTS num ADR futuro quando a sustentabilidade
pesar mais que a ergonomia.

## Consequências

- (+) Builds reproduzíveis a partir de um crate publicado; embedding fino e versionado.
- (+) Toolchain stable conhecida e fixada (1.92.0), sem surpresa de nightly.
- (−) `0.2.0` é feature release, não LTS: o próximo bump pode trazer mais churn de API.
- (−) Primeira compilação cara (motor inteiro + mozjs do fonte); deps de sistema via apt.
- **Mecanismo (archgate):** updates do Servo passam a falhar alto (build/clippy/testes quebram) em
  vez de degradar em silêncio. Procedimento de bump entra no runbook do Servo (H3).

## Fontes (jun/2026)

- crates.io `servo` (0.0.1 … 0.1.0/0.1.1 … 0.2.0): https://crates.io/crates/servo
- Toolchain do tag: https://raw.githubusercontent.com/servo/servo/v0.2.0/rust-toolchain.toml → `1.92.0`
- Build no Linux (deps apt): https://book.servo.org/building/linux.html
- Embedding overview: https://book.servo.org/embedding/overview.html
- Rename libservo→servo: https://github.com/servo/servo/pull/43141
- Recursos embutidos: https://github.com/servo/servo/pull/43182
- Exemplo mínimo: components/servo/examples/winit_minimal.rs (tag v0.2.0)
