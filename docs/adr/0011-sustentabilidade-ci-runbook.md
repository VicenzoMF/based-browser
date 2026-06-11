# ADR-0011: Sustentabilidade (Goal #3) — CI na revisão fixada + runbook de bump + archgate + sandbox

- **Status:** Accepted
- **Data:** 2026-06-11
- **Relaciona:** ADR-0002 (pin `servo =0.2.0` + toolchain `1.92.0`). Não o supersede — este ADR adiciona
  os MECANISMOS de sustentabilidade em volta do pin (HARNESS-ROADMAP H3). Mitiga **L-001** (Verso/churn).

## Contexto

O **Goal #3 do PROJECT** ("atualizar a revisão fixada do Servo em **< 1 dia de trabalho por sprint**") era
o único Goal não atacado, e é a defesa direta contra o risco existencial **L-001** (o Verso morreu afogado
no churn da API do Servo). Até aqui o gate de qualidade só existia **localmente** (lefthook + hooks
`.claude/`): um bump do pin podia quebrar em silêncio entre máquinas, e não havia procedimento **medido**.

Incerteza de infra (o "M7-equivalente" deste marco): o build do Servo é **pesado** (motor + mozjs do
fonte; deps de sistema via apt; `target` de vários GB; ~7 min a frio). A pergunta-chave: **cabe num CI
hospedado free?** Decisão tomada **na prática**, não no chute.

## Decisão

1. **CI completo no GitHub Actions** (`.github/workflows/ci.yml`), por **push(main) + PR + manual**,
   espelhando o gate local: **archgate → fmt → clippy `--exclude servo-poc -D warnings` → test**. Runner
   `ubuntu-24.04` **free** (repo público). Headless por natureza (**L-008**): builda/linta/testa, **não
   abre janela** — os `scripts/m{5,6,7}` (headful/GPU) ficam locais (runbook).
2. **Runbook determinístico** (`docs/runbooks/atualizar-servo.md`) + script
   (`scripts/update-servo/run.sh`) que mede um bump-candidato num **git worktree isolado** (não toca o pin
   protegido da `main`), reusa o cache e cronometra contra a meta "< 1 dia".
3. **Archgate** (`scripts/checks/`): checks executáveis com **erro-como-instrução** que acoplam o ADR a
   uma regra mecânica — `check-servo-pin` (pin nos 2 crates + toolchain = valores do ADR-0002) e
   `check-adr-status`. Rodam no gate local (lefthook) **e** no CI. Um bump legítimo do pin atualiza
   `EXPECT_*` no check (a prova executável da decisão consciente — ADR↔check).
4. **Sandbox sem egress** (`sandbox/`) ativada: `network_mode: none` + `cap_drop: ALL` + non-root +
   read-only; garantia central (sem egress) **verificável** por smoke; render headful documentado com
   caveat de GPU/display (CI não roda).

### Viabilidade — confirmada na prática

- **Prova decisiva:** o **próprio CI do Servo** roda o build Linux completo em runner GitHub-hosted
  `ubuntu` com `jlumbroso/free-disk-space`. Runner público = **4 vCPU / 16 GB RAM / 14 GB SSD → ~45 GB**
  após liberar disco; teto **6h/job**; **minutos grátis** em repo público.
- **Empírico (este marco):** o 1º run a frio passou por TODAS as etapas no runner — free-disk → apt →
  toolchain `1.92.0` (de `rust-toolchain.toml`, via `actions-rust-lang/setup-rust-toolchain`) → fmt →
  **clippy (cold build do motor + mozjs) verde** → test. **Não precisou degradar.**
- **Dry-run do runbook (rehearsal 0.2.0, cache quente):** gate **verde em ~81s** (build 29s / clippy 18s
  / test 34s). Um bump real adiciona o recompile do motor (~7 min a frio, ADR-0002/M0) + triagem de churn
  — ambos **<< 1 dia**. `0.2.0` é a versão mais nova publicada (sem alvo de upgrade ainda); a medição de
  churn de upgrade real ocorre no próximo release do Servo (runbook pronto).

### Segurança / supply-chain (L-002)

- Actions de terceiros **pinadas por commit SHA**, de fontes reputáveis: `actions/checkout` (GitHub),
  `jlumbroso/free-disk-space` (usada pelo CI do Servo), `actions-rust-lang/setup-rust-toolchain`.
- `permissions: contents: read`; `RUSTFLAGS` neutralizado (a action seta `-D warnings` global por padrão
  → quebraria no warning de uma DEP que não controlamos; o gate de lint vem do nosso clippy explícito).
- Caminho **degradado** (caso o build se mostrasse caro/instável): CI manual/agendado + verificação local
  espelhando o gate. **Documentado mas NÃO necessário** — o CI hospedado free coube.

## Consequências

- (+) Um bump do Servo que quebre build/lints/testes **falha alto** (CI + archgate), não em silêncio →
  fecha o Goal #3 e operacionaliza o L-001.
- (+) Runbook **medido** prova que a meta "< 1 dia" é atingível (baseline sem-churn ~minutos).
- (+) Nenhuma dep nova; config protegida (pin/toolchain/lints/`.claude`/ADRs) intocada; embedding fino.
- (−) `apt` lista canônica do Servo (~40 pkgs, **não 18** como dizia o ADR-0002 de memória) — reconciliado
  aqui; loop resiliente a renames mesa entre 22.04/24.04.
- (−) Cache do GHA tem teto de 10 GB; se o `target` de deps estourar, cair p/ cache só de `~/.cargo` ou
  sccache (otimização deferida; só se necessário).
- (−) Sandbox headful precisa de GPU/display via passthrough (afrouxa o isolamento de propósito — [D]);
  o no-egress é a garantia central verificável; `no-new-privileges` pode dar EPERM no exec sob AppArmor.

## Fontes

- CI do Servo (prova de viabilidade): github.com/servo/servo `.github/workflows/linux.yml`
- Runner free / limites: docs.github.com/actions/reference/runners/github-hosted-runners + .../limits
- Deps Linux do Servo: book.servo.org/building/linux.html
- Actions: actions/checkout · jlumbroso/free-disk-space · actions-rust-lang/setup-rust-toolchain
- Versões do `servo`: crates.io/crates/servo (newest = 0.2.0 em 2026-06-11)
