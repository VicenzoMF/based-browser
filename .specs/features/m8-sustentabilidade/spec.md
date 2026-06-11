# M8 — Sustentabilidade (runbook de bump do Servo + CI) — Specification

## Problem Statement

O BasedBrowser (M0–M7 ✅) fixa o motor em `servo = "=0.2.0"` (ADR-0002). O **Goal #3 do PROJECT**
(sustentabilidade) é o **único Goal ainda não atacado**, e endereça o risco existencial **L-001** (o
Verso morreu afogado no churn da API do Servo). Hoje o gate de qualidade só existe **localmente**
(lefthook + hooks `.claude/`): um bump do pin pode quebrar em silêncio, e não há **procedimento medido**
de atualização. O M8 transforma a lição L-001 num **mecanismo**: um **runbook determinístico** de bump
(medido contra a meta "**< 1 dia por sprint**") + **CI** que valida build/lints/testes na revisão fixada,
+ **Archgate** (ADR↔check) e **sandbox sem egress** (H3).

## Goals

- [ ] **CI** no GitHub Actions que valida a revisão fixada (fmt + clippy `-D warnings` + test, build
      incluído) por push + PR + manual, **verde** no GitHub. (Repo público → runner free.)
- [ ] **Runbook** determinístico de bump do pin do Servo, **medível** contra "< 1 dia por sprint".
- [ ] **Archgate**: check executável que falha (erro-como-instrução) se o pin/toolchain divergir do ADR.
- [ ] **Sandbox sem egress** ativada/documentada (rodar URL não confiável; caveat GPU/display honesto).
- [ ] **Validar o runbook** num dry-run de bump-candidato (medir o esforço), sem commitar o bump.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Matriz multi-OS no CI (Windows/macOS/Android) | Vira o próximo marco "outras plataformas". |
| Commitar o bump real do pin | Exige ADR dedicado num sprint futuro; o dry-run é só medição (branch descartável). |
| Rodar `scripts/m{5,6,7}` (headful/GPU) no CI | Impossível headless (L-008); ficam locais (runbook). |
| sccache | Otimização; documentada como fallback se o cache de 10 GB do GHA estourar. |

---

## User Stories

### P1: CI valida a revisão fixada ⭐

**User Story**: Como dev solo, quero que um push/PR rode o gate completo na revisão fixada do Servo, para
que uma quebra apareça **alto** no GitHub em vez de silenciosamente entre máquinas.

**Acceptance Criteria**:
1. WHEN há push na `main`, um PR, ou disparo manual THEN o CI SHALL rodar `fmt --check` + `clippy
   --workspace --exclude servo-poc -D warnings` + `test --workspace --exclude servo-poc` na toolchain
   `1.92.0` (do `rust-toolchain.toml`).
2. WHEN o build do Servo (motor + mozjs) roda no runner free THEN SHALL caber em disco/tempo (free-disk
   + cache) e terminar **verde**.
3. WHEN o pin ou a toolchain divergem do ADR THEN o **archgate** SHALL falhar com instrução de correção.

**Independent Test**: `gh run watch` no run do CI → conclusão **success**; `scripts/checks/archgate.sh`
sai 0 no estado bom.

### P1: Runbook de bump medível ⭐

**User Story**: Como mantenedor, quero um procedimento determinístico para subir o pin do Servo e **medir**
o esforço, para provar que "< 1 dia por sprint" é atingível (Goal #3).

**Acceptance Criteria**:
1. WHEN sigo o runbook THEN os passos SHALL ser mecânicos (branch → checar crates.io → re-pin → gate +
   verify locais → medir → ADR + PR ou abortar+documentar).
2. WHEN rodo `scripts/update-servo/run.sh <versão>` THEN SHALL aplicar o pin numa branch, rodar o gate e
   **imprimir o wall-clock medido** (sem commitar).

**Independent Test**: dry-run de bump-candidato com tempo medido registrado (runbook/STATE) vs "< 1 dia".

### P2: Sandbox sem egress

**User Story**: Como usuário/dev, quero rodar uma URL não confiável isolada (sem egress), para reduzir o
blast radius de conteúdo web malicioso.

**Acceptance Criteria**:
1. WHEN rodo a sandbox THEN SHALL aplicar `network: none` + `cap_drop: ALL` + non-root + read-only.
2. WHEN documento o uso THEN o caveat de GPU/display (headful) SHALL ser explícito (CI não roda isto).

**Independent Test**: `docker compose -f sandbox/docker-compose.yml config` valida; README com caveat.

---

## Verification (sem captura de janela — Wayland, L-008)

CI **headless** (build+lint+test) verde no GitHub; `scripts/checks/archgate.sh` (texto); runbook
exercitado num dry-run com tempo medido. Decisões: **ADR-0011** · **AD-014** · **L-011** (STATE).
