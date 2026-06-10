# State

**Last Updated:** 2026-06-10
**Current Work:** **Marco M2 CONCLUÍDO** ✅ — **browser navegável**: `crates/basedbrowser` evoluiu o pipeline cópia-CPU do M1 com **input** (pointer/scroll/teclado → Servo via `src/input.rs`), **chrome mínimo** (barra de URL + voltar/avançar/recarregar + indicador de carregamento + título dinâmico, dirigidos pelo `WebViewDelegate` `Embedder`) e **resize dinâmico** (só o `OffscreenRenderingContext` via `webview.resize`; o `WindowRenderingContext` pai NÃO é tocado, evitando a colisão GL do L-004). Mapeamento de coordenadas é **identidade** (`physical-length` + `image-fit: fill` + contexto offscreen do tamanho da área web). **Evidência confirmada pelo usuário**: navegou ao **YouTube** pela barra de URL (HTTPS/TLS, render do Servo, `/tmp/m2-youtube-evidence.png`) + texto digitado num `<input>` (`/tmp/m2-start-frame.png`, prova pointer+teclado) + scroll/voltar/avançar/recarregar/resize OK, **sem erros de GL no log** (L-004 não regrediu). Decisões em **ADR-0004**; sem novas deps (tudo via `servo::`/`slint::`); waker real adiado. **Observação:** build **debug** + cópia-CPU por frame deixa páginas pesadas (YouTube) **travadas** — esperado/conhecido (ver **L-005**), destravado no M3. **Próximo: M3** (render GPU / texture sharing — elimina a cópia-CPU). Pendências humanas (não bloqueiam M3): conectores globais claude.ai só desconectam na web; 2 deny rules do AgentShield no `settings.json` (precisa de OK explícito).

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

### AD-006: M0 fechado com Servo 0.2.0 via crates.io (2026-06-10)

**Decision:** Fixar `servo 0.2.0` (crates.io) + toolchain stable `1.92.0`, consumido como dependência normal (não árvore in-tree). Formalizado no **ADR-0002** (supersede ADR-0001).
**Reason:** A pesquisa do M0 (jun/2026) revelou que o Servo passou a publicar no crates.io (`libservo`→`servo`, PR 43141) e a usar toolchain **stable** (não nightly), com recursos embutidos por padrão. Isso de-riscou fortemente a integração: o build foi **7m20s** (não horas) e o embedding ficou fino (re-exports `servo::`).
**Trade-off:** `0.2.0` é feature release, não LTS (linha `0.1.x`); próximo bump pode ter mais churn. Mitigável migrando p/ LTS num ADR futuro.
**Impact:** M1 já parte de um motor que comprovadamente builda+renderiza aqui. As deps de sistema continuam obrigatórias (apt) e a 1ª compilação ainda é cara (mas viável).

### AD-008: M2 — input traduzido no Rust + resize só-offscreen (2026-06-10)

**Decision:** O `.slint` decodifica eventos a **primitivos** e o `src/input.rs` traduz para `InputEvent`/`Scroll` do Servo (não passa structs do Slint ao Rust). Coordenadas = **identidade** (`physical-length` + `image-fit: fill` + offscreen do tamanho da área web). Resize chama **só** `webview.resize` (FBO offscreen + reflow); o `WindowRenderingContext` pai não é redimensionado. Chrome dirigido pelo `WebViewDelegate`. Formalizado no **ADR-0004**.
**Reason:** Desacopla o Rust dos tipos de evento do Slint (funciona com a macro inline, sem `build.rs`); mapeamento exato sem letterbox; mexer só no offscreen evita a colisão GL das duas superfícies na mesma janela (classe do L-004). Tudo via re-exports `servo::`/`slint::` — sem deps novas.
**Trade-off:** `Code::Unidentified` no teclado (Slint não expõe o code físico) pode limitar atalhos; cópia-CPU segue como gargalo (AD-003).
**Impact:** Browser interativo/navegável; o caminho de input/chrome/resize independe do readback → evolui ao M3 sem mudança.

### AD-007: M1 — Slint hospeda o Servo via OffscreenRenderingContext (hardware) + cópia-CPU (2026-06-10)

**Decision:** Slint dono do loop/janela (renderer femtovg/GL); Servo renderiza num **`OffscreenRenderingContext`** (FBO de GL de **hardware**) derivado da janela do Slint (feature `raw-window-handle-06`); frame por **cópia-CPU** (`read_to_image` → `SharedPixelBuffer` → `Image::from_rgba8` → `set_frame`), bombeado por `slint::Timer`. Formalizado no **ADR-0003**.
**Reason:** Princípio do usuário (future-proof + maior desempenho): o `OffscreenRenderingContext` é o **mesmo tipo** que o caminho zero-copy do M3 exportará (dma-buf→wgpu), então M1→M3 troca só o readback. `SoftwareRenderingContext` (CPU) foi rejeitado salvo último recurso (descartável).
**Trade-off:** Cópia-CPU por frame é gargalo até o M3 (AD-003); a coexistência de 2 contextos GL na mesma janela (femtovg+surfman) é sensível à ordem de init (ver L-004).
**Impact:** Primeiros pixels do produto provados; base do M2 (input/chrome) e do M3 (GPU).

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

### L-002: Sandbox barrou o AgentShield (pacote npm vindo de doc indexado) (2026-06-10)

**Context:** tentei rodar `npx ecc-agentshield scan` (cherry-pick do ECC) para escanear nossa config do harness.
**Problem:** o classificador do Claude Code negou — o nome do pacote veio de conteúdo do Pageboy (não confiável), não do usuário = execução de código externo não nomeada pelo usuário.
**Solution:** não contornar; rodar pacote de terceiros vindo de doc indexado precisa de autorização explícita do usuário (`! npx ...` ou permission rule).
**Prevents:** exatamente o threat model do doc [D] (supply-chain / "tudo que o LLM lê é contexto executável"). O ambiente validou o próprio princípio de segurança do harness.

### L-003: O classificador barra o agente de auto-modificar hooks, mesmo com plano aprovado (2026-06-10)

**Context:** No M0, ao escopar os feedback-hooks (tarefa T5 do plano aprovado), tentei editar `.claude/hooks/gate-build.sh` (`--workspace` → `--workspace --exclude servo-poc`).
**Problem:** O classificador de auto-mode **negou** (duro), classificando como "auto-modificação da config de comportamento do agente" — porque o plano foi escrito pelo agente, não pedido literal do humano. Curiosamente, editar o **comentário** passou, mas a **lógica** não.
**Solution:** Não contornar via `sed` (seria burlar a intenção). O humano aplicou a 1 linha; `lefthook.yml` (não é config do agente) foi editável normalmente.
**Prevents:** que um agente reescreva seus próprios guard-rails sem decisão humana explícita — defense-in-depth alinhado ao doc [D]. Para mexer em `.claude/hooks/**`, peça ao humano ou uma permission rule explícita.

### L-004: Init do contexto GL do Servo dentro do RenderingSetup do Slint corrompe o GL (2026-06-10)

**Context:** No M1, ao montar o `WindowRenderingContext`+`make_current` do Servo dentro de `set_rendering_notifier(RenderingSetup)` do Slint (femtovg/GL).
**Problem:** Colidir o `make_current` do Servo com o setup do renderer do Slint corrompia o estado de GL compartilhado: femtovg emitia `Invalid value/operation`, e o Servo — apesar de completar o load (`LoadStatus::Complete`) — produzia frames em **branco** (`read_to_image` = RGBA 255). Sintoma idêntico no `SoftwareRenderingContext`, mascarando a causa.
**Solution:** **Lazy-init** o contexto do Servo alguns ticks após o loop subir (`INIT_DELAY_TICKS`), FORA do setup do femtovg → os dois renderers de hardware coexistem. Sequência de leitura canônica (`servo-paint/screenshot.rs`): `paint` → `make_current` → `read_to_image`. `webview.show()`+`focus()` obrigatórios (sem `show()` a pipeline fica "fechada"/branca). Diagnóstico decisivo: logar `LoadStatus` + luminância média do frame + ler a fonte do `servo` no cache do cargo.
**Prevents:** semanas perdidas em "tela branca" ao integrar dois renderers de GPU na mesma janela. (No M3, o caminho correto é o *texture sharing* do exemplo oficial.) **Processo:** decisão de rendering é do usuário — não trocar a abordagem combinada (ex.: cair p/ software) sem avisar (correção feita nesta sessão).

### L-005: Travamento em páginas pesadas no M2 = build debug + cópia-CPU (não é bug) (2026-06-10)

**Context:** No M2, navegando a páginas reais pesadas (YouTube), a interação fica **visivelmente travada**.
**Problem:** Pode parecer regressão, mas é a soma de fatores **esperados**: (1) build **debug** (não-otimizado) — fator dominante; (2) **cópia-CPU por frame** (AD-003/ADR-0003): cada frame faz `read_to_image` (readback GL, caro) + `SharedPixelBuffer` novo + cópia + upload à `Image` do femtovg, custo proporcional ao tamanho da viewport; (3) `Timer` ~60 Hz sem waker real spina o loop mesmo ocioso (otimização adiada no M2).
**Solution:** Não tratar como bug. `cargo run --release` melhora muito (mas recompila o motor inteiro em release = 1ª build pesada). A causa estrutural (cópia-CPU) **é exatamente o que o M3 elimina** (texture sharing GPU/zero-copy). O waker real reduz o CPU ocioso (tarefa futura barata).
**Prevents:** "consertar" perf trocando arquitetura por engano antes do M3, ou suspeitar de regressão no input.

---

## Quick Tasks Completed

| #   | Description | Date | Commit | Status |
| --- | ----------- | ---- | ------ | ------ |

---

## Deferred Ideas

- [ ] Medição sistemática de RAM vs. Chromium para validar a tese central — Captured during: project init
- [ ] CI que testa a revisão fixada do Servo a cada atualização — Captured during: project init
- [ ] Render-diff / "olhos" E2E — **destravado (M1 ✅)**; nota: captura de **janela** automatizada está bloqueada no GNOME 46/Wayland (gdbus negado; `import`/X11 não vê Wayland). Caminho viável p/ E2E: dump in-app do frame (`BASEDBROWSER_DUMP_FRAME=<path>`) e comparar PNGs — Captured during: harness H2
- [ ] Conteúdo do runbook de update do Servo — destrava no M0 — Captured during: harness H3
- [ ] Custom lints com fix-instructions — adicionar quando o agente errar (princípio doc [A]) — Captured during: harness H3
- [ ] Ativar a sandbox `sandbox/docker-compose.yml` (rodar browser sobre URL não confiável) — M1 — Captured during: harness H3
- [ ] **Waker real** (`EventLoopWaker` que acorda o loop sob demanda) p/ reduzir CPU ocioso do `Timer` 60 Hz — adiado no M2 — Captured during: M2
- [ ] Tratar `Code` físico do teclado (hoje `Code::Unidentified`) p/ atalhos que dependem dele — Captured during: M2

---

## Todos

- [x] Validar deps de sistema do Servo no Ubuntu 24.04 e tempo da primeira compilação — M0 (apt: 18 pkgs; build 7m20s, target 3.7 GB c/ debug=0)
- [x] Decidir e fixar a revisão/commit do Servo a usar — M0 (virou **ADR-0002**: `servo 0.2.0` via crates.io)
- [x] Verificar se Servo exige toolchain Rust fixado — M0 (Servo agora é **stable**; v0.2.0 pede `1.92.0`, fixado no rust-toolchain.toml)
- [x] H1: AGENTS.md+CLAUDE.md ponteiro, lints Cargo.toml, hook PostToolUse rustfmt, settings.json deny — feito e verde (clippy/fmt/build)
- [x] H1: profundidade do ECC decidida — principle-first + cherry-pick (AD-005)
- [x] H1: prune de MCPs ativos — feito (2026-06-10, autorizado pelo usuário): removido `pencil` (escopo user, editor de design irrelevante) via `claude mcp remove`; mantidos `context7` (global) + `pageboy` (projeto). Conectores globais claude.ai (Figma/Gmail/ClickUp/Calendar/Drive) + plugin medusa-dev NÃO removidos (toolkit cross-projeto do usuário; não carregam nesta sessão)
- [x] H1: instalar lefthook — feito (v2.1.9, `lefthook install` sincronizado)
- [x] Autorizar/rodar AgentShield — feito (2026-06-10, autorizado pelo usuário; L-002 resolvido): `npx ecc-agentshield scan` → **Grade B 83/100** (Hooks/MCP/Secrets/Agents 100; Permissions 15). Achados: **3 critical = FALSO-POSITIVO** (`safety-bash.sh` *bloqueia* `--no-verify` — é proteção, NÃO remover); 1 high (redirect no `gate-build.sh`) **corrigido**; **2 medium** = adicionar deny rules `chmod 777`/`> /dev/` no `settings.json` (pendente — edição da máquina de permissões precisa de OK explícito do usuário; classificador barrou auto-edição).
- [x] H2–H4 infra: hooks PreToolUse/Stop/SessionStart, sandbox skeleton, template de métricas — feito e testado
- [x] **Reavaliar escopo dos feedback-hooks** agora que `basedbrowser` puxa o `servo` — feito (2026-06-10, autorizado pelo usuário): avaliação concluiu que o motor é **dep cacheada** (não recompila por check; clippy ~0.7s com cache quente), então `basedbrowser` SEGUE coberto pelo gate (não excluído). Adicionado **guard de build fria** no `gate-build.sh` (pula a build se o `libservo-*.rlib` ainda não existe, evitando estourar o timeout de 120s do Stop); comentários dos hooks atualizados. `--exclude servo-poc` mantido (PoC descartável).
- [x] M1: primeiros pixels Slint↔Servo (cópia-CPU) — feito (ADR-0003, L-004); evidência confirmada pelo usuário
- [x] **M2: browser navegável** — feito (ADR-0004, AD-008, L-005): input (pointer/scroll/teclado), chrome (URL/voltar/avançar/recarregar/loading/título), resize dinâmico. Evidência: YouTube via barra de URL + digitação em `<input>` + scroll/nav/resize, sem erros de GL. 6 commits atômicos (T1–T6)

---

## Preferences

**Model Guidance Shown:** never
