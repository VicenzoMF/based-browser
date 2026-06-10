# State

**Last Updated:** 2026-06-10
**Current Work:** harness **H1 fundação concluída e verde** (lints/hooks/settings deny/ADR-0001). Próximo: **M0** (receita de build do Servo) + finalizar H1 (prune de MCP, instalar lefthook).

---

## Recent Decisions (Last 60 days)

### AD-001: Stack Slint + Servo (2026-06-10)

**Decision:** UI/chrome em Slint (backend winit) + motor web em Servo (`libservo`/`WebView`).
**Reason:** Ambos são Rust-native (mínima ponte de API/build); a própria equipe do Slint já integrou Servo e documentou (post "Using Servo with Slint"), então a viabilidade está comprovada.
**Trade-off:** Servo tem compat web incompleta; Slint não é desenhado para hospedar um motor web (integração é pioneira).
**Impact:** Define toda a arquitetura: Slint dono do loop, Servo em threads, bridge via waker.

### AD-002: Motor próprio (Servo) em vez de webview do sistema (2026-06-10)

**Decision:** Usar Servo como motor, NÃO wry/WebKitGTK.
**Reason:** O objetivo central é um motor leve Rust-native; usar o webview do sistema trairia a tese (no Linux seria WebKitGTK C++).
**Trade-off:** Parte da web real vai quebrar/renderizar errado; muito mais esforço. Aceito explicitamente pelo usuário.
**Impact:** Compat incompleta é uma constraint de produto, não um bug.

### AD-003: Render começa por cópia-CPU, evolui para GPU (2026-06-10)

**Decision:** M1 usa cópia-CPU (buffer offscreen do Servo → `slint::Image`); GPU texture sharing fica para M3.
**Reason:** Cópia-CPU é simples e fácil de debugar; prova o pipeline ponta-a-ponta antes de otimizar.
**Trade-off:** Gargalo de performance conhecido (cópia CPU→GPU por frame) até M3.
**Impact:** M1 entrega valor (pixels na tela) sem o custo do interop Vulkan↔GL.

### AD-004: De-risking primeiro — provar o motor antes do Slint (2026-06-10)

**Decision:** M0 builda e roda Servo isolado (exemplo winit) ANTES de integrar Slint.
**Reason:** O maior risco/custo é o build do Servo; falha aqui invalida todo o resto.
**Trade-off:** Nenhum pixel "do produto" no M0; é puro de-risking.
**Impact:** Ordem das fases segue risco decrescente.

### AD-005: Adotar harness engineering (principle-first) (2026-06-10)

**Decision:** Construir um harness de desenvolvimento em fases (H1–H4) fundamentado em 4 docs no Pageboy. Roadmap em `HARNESS-ROADMAP.md`. Estratégia: **principle-first + cherry-pick do ECC** (não adoção plena por padrão).
**Reason:** Servo tem churn de API (lição do Verso) + build pesado; harness compõe juros e mitiga ambos. Dev solo → "afie o harness com UM agente antes de escalar".
**Trade-off:** Tempo investido em tooling em vez de feature; harness tem "shelf life" curto (pode ser absorvido pelos agentes).
**Impact:** H1 roda junto com M0 (lints Rust, hooks, settings.json deny, ADR de pin do Servo, prune de MCPs). Decisão de profundidade do ECC ainda em aberto (ver HARNESS-ROADMAP.md).

---

## Active Blockers

_Nenhum no momento._

---

## Lessons Learned

### L-001: O Verso morreu por não acompanhar o churn do Servo (2026-06-10)

**Context:** Verso era o projeto mais avançado de browser sobre Servo.
**Problem:** Foi arquivado em 2026 — não conseguiu acompanhar as mudanças da API do Servo com pouca mão de obra/financiamento.
**Solution:** Manter o código de embedding o mais fino possível e fixar uma revisão do Servo; fazer sprints de atualização periódicos e deliberados.
**Prevents:** Que o projeto morra afogado em churn de upstream.

---

## Quick Tasks Completed

| #   | Description | Date | Commit | Status |
| --- | ----------- | ---- | ------ | ------ |

---

## Deferred Ideas

- [ ] Medição sistemática de RAM vs. Chromium para validar a tese central — Captured during: project init
- [ ] CI que testa a revisão fixada do Servo a cada atualização — Captured during: project init

---

## Todos

- [ ] Validar deps de sistema do Servo no Ubuntu 24.04 e tempo da primeira compilação — M0
- [ ] Decidir e fixar a revisão/commit do Servo a usar — M0 (vira ADR-0001)
- [ ] Verificar se Servo exige toolchain Rust fixado (vs. 1.90.0 stable atual) — M0
- [x] H1: AGENTS.md+CLAUDE.md ponteiro, lints Cargo.toml, hook PostToolUse rustfmt, settings.json deny — feito e verde (clippy/fmt/build)
- [x] H1: profundidade do ECC decidida — principle-first + cherry-pick (AD-005)
- [ ] H1: prune de MCPs ativos (manter ~context7+pageboy) para <10 MCPs/<80 tools — harness (precisa do usuário, via /mcp)
- [ ] H1: instalar lefthook (`lefthook install`) p/ ativar o gate de pre-commit — harness

---

## Preferences

**Model Guidance Shown:** never
