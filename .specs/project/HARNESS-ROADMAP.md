# Harness Engineering Roadmap — BasedBrowser

> **Harness engineering** = "o que você faz pra manter um Coding Agent rodando autônomo com o mínimo de intervenção humana e estabilizar o output" — *as rodinhas do agente*. Princípio-mestre: **"o sistema, não o modelo, é o que importa."** [A]

**Fontes** (indexadas no Pageboy, coleções `default` + `ecc-guides`):
- **[A]** *Harness Engineering Best Practices for Claude Code / Codex (2026)* — Sakasegawa. (doc central; staging + lints Rust)
- **[B]** *ECC — agent harness performance optimization system* (README). (toolkit: skills, instincts, memory, AgentShield)
- **[C]** *ECC — the-longform-guide.md*. (economia de contexto, pipeline, tiering de modelo)
- **[D]** *ECC — the-security-guide.md*. (segurança do harness)

`[tailoring]` = adaptação específica deste projeto (não está nos docs; derivado das constraints Rust/Servo/Slint).

---

## Por que isto importa AGORA neste projeto

1. **Investir em harness compõe juros** [A]: cada regra de lint / teste evita aquele erro em toda sessão futura.
2. **Churn de API do Servo** (lição do Verso, ver STATE.md L-001): testes + ADRs que fixam comportamento fazem updates do Servo **falharem alto** em vez de silenciosamente. `[tailoring]`
3. **Build pesado do Servo**: o loop de feedback precisa ser rápido **sem recompilar o Servo** a cada checagem. `[tailoring]`
4. **Dev solo**: "afie o harness com UM agente primeiro; escalar agentes sem harness gera dívida cognitiva composta, não alavancagem." [A] — sua situação exata.
5. **Browser carrega conteúdo web não confiável** → quando o agente rodar testes, o track de segurança [D] se aplica em dobro.

---

## Princípios-âncora (valem em todas as fases)

- **Resolva com mecanismos, não com prompts.** "Não faça o LLM fazer o trabalho do linter." [A]
- **Design para apodrecer (design for rot).** No repo só entram artefatos **mecanicamente decidíveis/executáveis** (código, testes, lint/types, CI) + **ADRs imutáveis e datados**. Prosa/design-docs explicativos ficam **fora** (apodrecem em silêncio). "Informação obsoleta que o agente acha no repo é indistinguível da verdade atual." [A]
- **Empurre o feedback pra baixo da pilha:** PostToolUse (ms) → pre-commit (s) → CI (min) → humano (horas). [A]
- **Separe planejar de executar** (Plan Mode + aprovação humana). "A coisa mais importante que eu faço." [A]
- **Economia de contexto é o orçamento real:** <10 MCPs / <80 tools [B]; modelo mais barato que resolve [C]; compactar em breakpoints lógicos, não a 95%. [C]
- **Nunca deixe a camada de conveniência ultrapassar a de isolamento.** [D]

---

## Tracks (cross-cutting, evoluem juntos pelas fases)

| Track | Foco | Docs |
|---|---|---|
| **1. Determinismo & Feedback** | Lints, hooks, testes, CI | [A] |
| **2. Economia de Contexto** | MCP slim, tiering de modelo, CLI>MCP, compactação | [B][C] |
| **3. Memória & Estado** | Handover entre sessões, instincts | [A][B][C] |
| **4. Segurança do Harness** | Permissões, sandbox, supply-chain | [D] |
| **5. Verificação ("olhos")** | E2E / render-diff / a11y tree | [A] |

---

## Fases (horizontes alinhados ao staging do Doc A)

### H1 — Fundação Mínima  ·  ~Semana 1  ·  (junto com o M0 do produto)
**Goal:** "Minimum Viable Harness" — o agente já trabalha com guard-rails desde o primeiro commit.

- **AGENTS.md + CLAUDE.md como ponteiro** (< 50 linhas): roteamento + proibições + comandos build/test/run mínimos. Nada de tech-stack/estilo/estado. [A] `[tailoring]` documentar o **caminho rápido de build** (embedder vs. Servo já compilado).
- **Lints Rust no `Cargo.toml`** (padrão "rust-magic-linter") [A]:
  ```toml
  [workspace.lints.clippy]
  pedantic = "warn"
  unwrap_used = "deny"
  expect_used = "deny"
  dbg_macro = "deny"
  [workspace.lints.rust]
  unsafe_code = "warn"   # haverá unsafe no interop GPU; revisar caso a caso
  ```
  e bloquear `#[allow]` solto (`allow_attributes = "deny"`) pra o agente não silenciar lint.
- **Hook PostToolUse = `rustfmt`** (ms) + clippy só no **crate embedder**, nunca recompilando o Servo. `[tailoring]`
- **Pre-commit via Lefthook** (rápido) = `cargo clippy` no workspace do app. [A]
- **Baseline de segurança** [D] (maior ROI, fácil): `permissions.deny` no `.claude/settings.json` →
  ```json
  { "permissions": { "deny": [
      "Read(~/.ssh/**)", "Read(~/.aws/**)", "Read(**/.env*)",
      "Bash(curl * | bash)", "Bash(ssh *)", "Bash(scp *)"
  ] } }
  ```
- **Primeiro ADR** imutável: `docs/adr/0001-pin-servo-revision.md` (revisão git do Servo fixada + por quê). [A] `[tailoring]` (ataca o churn na raiz)
- **Vetar os MCPs já conectados** [D][B]: hoje há vários (ClickUp, Figma, pencil, medusa, context7, pageboy…). Manter **só os do projeto** (provável: `context7` + `pageboy`) → respeita <10 MCPs/<80 tools e reduz superfície de ataque.

### H2 — Loops de Feedback & Disciplina de Plano  ·  ~Semanas 2-4
**Goal:** o agente se auto-corrige e só "termina" quando o verde fecha.

- **Plan → aprovar → executar**: Plan Mode do Claude Code casado com o fluxo `tlc-spec-driven` (Specify→Design→Tasks→Execute). [A][C]
- **Hook Stop = bloqueia parar enquanto os testes não passam** (cuidar de `stop_hook_active` p/ não loopar). [A]
- **Regra do erro:** toda vez que o agente errar, **adicione um teste ou regra de lint** — não um parágrafo de doc. "Testes não mentem quando você os roda." [A]
- **Rotina de sessão / handover** [A][C]: início = verificar cwd + ler `git log` + arquivo de progresso; fim = commit descritivo (`git log --oneline` é o handover). Progresso em **JSON**, não Markdown; **um arquivo por sessão** pra não poluir contexto. (Reusar a memória global + STATE.md que já temos.)
- **Tiering de modelo** [C]: Sonnet p/ ~90% do código; **Opus p/ arquitetura e mexidas na API do Servo**; Haiku p/ busca/exploração.
- **Pipeline em fases** (orquestrador, 1 input → 1 output por fase, `/clear` entre agentes) [C]:
  `Research → Plan → Implement → Review → Verify`.
- **Primeiros "olhos" (PoC)** [A] `[tailoring]`: protótipo de **render-diff por screenshot** da janela do BasedBrowser (regressão visual). Ver "Track de Verificação" abaixo — é a peça mais difícil aqui.

### H3 — Anti-Rot, Safety Gates & Runbook do Servo  ·  ~Mês 2-3
**Goal:** o repo resiste a apodrecer e ações destrutivas ficam bloqueadas por mecanismo.

- **Linters customizados com a mensagem-de-erro-como-instrução** [A]: formato `ERRO / POR QUÊ (link ADR) / FIX / EXEMPLO`. "O agente ignora doc, mas não ignora erro de lint (o CI não passa)."
- **Archgate:** acoplar cada ADR a uma **regra executável** (ADR ↔ lint/check). [A]
- **PreToolUse safety gates** [A][D]: bloquear `rm -rf`, drop table, edição de `.env`; **proteger config** (`Cargo.toml` seção de lints, `rust-toolchain.toml`, **revisão fixada do Servo**) pra o agente corrigir o código, não o linter. Banir `git commit --no-verify`.
- **Runbook/skill "atualizar Servo"** `[tailoring]`: procedimento determinístico de bump da revisão fixada + rodar a suíte → mede contra a meta do PROJECT.md (**< 1 dia por sprint de update**). Mitiga diretamente a lição do Verso.
- **Sandbox p/ rodar conteúdo não confiável** [D] `[tailoring]`: quando o agente abrir URLs arbitrárias no browser em teste, rodar em container **sem egress** (`network: none`), `cap_drop: ALL`, non-root. Blast radius pequeno.
- **Trocar prosa por testes + ADRs**: remover gradualmente docs descritivos; dependências viram types/schemas + testes estruturais. [A]

### H4 — Loops Avançados, GC & Medição  ·  ~Mês 3+
**Goal:** o harness é medido e melhora sozinho; só então pensar em escalar.

- **eval-harness** [B][C]: medir o pipeline com **pass@k** (precisa funcionar 1×) vs **pass^k** (consistência) — usar pass^k onde a estabilidade importa (ex.: interop GPU).
- **GC determinístico** [A]: processos de limpeza **baseados em regra**, não em "julgamento" de agente (que também sofre context rot).
- **Métricas de efetividade do harness** [A] `[tailoring]` (versão solo): PRs/dia, taxa de retrabalho, **% de sessões que terminam no verde**, **duração do sprint de update do Servo**, nº de erros pegos por hook vs. por humano.
- **Escalar agentes só se necessário** [A][C]: worktrees/paralelismo "só por necessidade real". Você é UM dev — primeiro maximize um agente.

---

## Track de Verificação ("olhos") — o ponto mais difícil deste projeto

O Doc A é honesto: a seção de E2E é **web/mobile-cêntrica e NÃO cobre Slint nem Servo**; o princípio transferível é *"a árvore de acessibilidade é a interface universal"* + *"dê ao agente saída em texto estruturado que ele possa verificar."* [A]

Opções a investigar (marcado como **pesquisa**, sem fabricar solução):
1. **Render-diff por screenshot** da janela do BasedBrowser (regressão visual determinística). `[tailoring]`
2. **Reftests / Web Platform Tests do próprio Servo** para comportamento do motor. `[tailoring]`
3. **Árvore de acessibilidade**: validar se o Slint expõe a11y (AccessKit/AT-SPI no Linux) utilizável como texto estruturado de verificação — **a confirmar**, não assumir.

---

## Decisão em aberto: profundidade de adoção do ECC

O ECC [B] é uma **implementação pronta** de muitos destes princípios. Opções:
- **(Recomendado) Principle-first + cherry-pick:** seguir o roadmap acima e puxar do ECC só peças pontuais — skills `tdd-workflow`, `verification-loop`, `search-first`, `strategic-compact`, `security-review`; `rules/common` + regras Rust; e **AgentShield** (`npx ecc-agentshield scan`) p/ o track de segurança. Mantém o harness fino (lição do Verso vale aqui também).
- **Adoção plena do ECC:** instalar via plugin (`/plugin install ecc@ecc`, Claude Code ≥ v2.1.0), **um único caminho de install** (não empilhar métodos). Mais poder, mais peso/superfície.
- **Só princípios:** ignorar o ECC e construir tudo à mão.

---

## Mapa Harness × Produto

| Produto | Harness em paralelo |
|---|---|
| **M0** Build Servo | **H1** (fundação + ADR de pin do Servo + lints + segurança baseline) |
| **M1** MVP Slint+Servo | **H2** (loops, plano, tiering, 1º render-diff) |
| **M2** Navegável | **H2→H3** (safety gates, runbook Servo) |
| **M3** GPU | **H3** (pass^k no interop, sandbox) |
| **M4** Recursos | **H4** (medição, GC) |

---

## "Minimum bar" de segurança (subset solo-local do checklist de 11 do Doc D)

Prioritário agora: **(1)** regras `deny` no settings.json · **(2)** nunca `--dangerously-skip-permissions` em loop sem revisão · **(3)** memória sem segredos, projeto vs. global separados · **(4)** vetar skills/MCP/hooks como supply-chain (AgentShield) · **(5)** sandbox sem egress p/ conteúdo web não confiável. Itens de escala multi-agente/networked (identidade dedicada, dead-man switch, OpenTelemetry) ficam **adiados** enquanto for dev solo local.
