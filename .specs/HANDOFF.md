# Handoff

**Date:** 2026-06-11
**Feature:** M8 — Sustentabilidade (Goal #3): CI na revisão fixada + runbook de bump + archgate + sandbox ✅ CONCLUÍDO
**Task:** M8 fechado. **Todos os 3 Goals do PROJECT atacados.** Próximo = outras plataformas.

## Completed ✓

- **M8 done** (critério: CI verde na revisão fixada + runbook determinístico medível vs "< 1 dia" + ADR + push):
  - **T0 — spec tlc** (`889d074`): `.specs/features/m8-sustentabilidade/{spec,tasks}.md`.
  - **T1 — archgate** (`f4452bc`): `scripts/checks/` (`archgate.sh` + `check-servo-pin` + `check-adr-status`),
    erro-como-instrução (ERRO/POR QUÊ/FIX/EXEMPLO), acopla ADR↔check; ligado no `lefthook.yml`.
  - **T2 — CI** (`d90626f`): `.github/workflows/ci.yml` (free-disk → apt → setup-rust-toolchain 1.92.0 +
    cache → archgate → fmt → clippy `--exclude servo-poc -D warnings` → test). Actions pinadas por SHA.
  - **T3 — runbook** (`c2470c2`): `docs/runbooks/atualizar-servo.md` + `scripts/update-servo/run.sh`
    (worktree isolado, cronometra vs "< 1 dia").
  - **T4 — sandbox** (`8c92f56`): `sandbox/` no-egress verificável (smoke) + headful documentado (caveat GPU).
  - **T5 — dry-run**: rehearsal 0.2.0 (cache quente) → gate **VERDE em ~81s**; `main` intocada, worktree limpo.
  - **T6 — ADR-0011 + docs + push** (este): ADR-0011 + STATE(AD-014/L-011)/ROADMAP/HANDOFF/AGENTS.
  - **Verificado:** **CI run a frio VERDE em ~15,5 min** (cold-build do motor+mozjs cabe no runner free —
    sem degradar); archgate sai 0 (bom) / 2 (pin divergente, testado em repo scratch); smoke da sandbox
    `OK: sem egress`. Gate local verde por commit (archgate+clippy). Nenhuma dep nova; config protegida intocada.

## In Progress

- Nada — checkpoint limpo na `main` (T0–T6 commitados; push feito). CI re-roda verde na revisão final.

## Pending (próximos marcos)

1. **Outras plataformas** (Windows/DirectX, macOS/Metal, Android) — matriz multi-OS no mesmo CI. Candidato natural.
2. **Otimizar baseline absoluto** (171 MiB ociosos; M5 só mediu).
3. Deferidos: downloads/modo privado (M6); DevTools v2/hardening por token (M7); sccache no CI (M8).

## Blockers

- Nenhum. Pendências humanas (não bloqueiam): 2 deny rules do AgentShield no `settings.json`; conectores claude.ai (web).

## Context

- Branch: `main` (github.com/VicenzoMF/based-browser). Idioma: **pt-BR**. Plan Mode antes de executar.
- **M8 (ADR-0011 / AD-014 / L-011):** o CI completo do Servo CABE num runner free (prova: o CI do próprio
  Servo). 3 pegadinhas de infra (L-011): `free-disk-space` obrigatório; `rustflags:""` (a action seta `-D
  warnings` global → quebraria no warning de dep); apt resiliente a renames mesa 22.04/24.04. Archgate
  acopla o pin (config protegida, ADR-0002) a um check; bump real exige ADR novo + atualizar `EXPECT_*`.
- **Reproduzir CI:** push → `gh run watch`. **Reproduzir runbook:** `scripts/update-servo/run.sh 0.2.0`.
- Decisões: STATE AD-001..014 · Lições: L-001..011 · ADRs: 0001..0011.
