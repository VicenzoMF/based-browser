# Handoff

**Date:** 2026-06-11
**Feature:** M7 — DevTools / inspeção in-app (console + eval + rede via cliente RDP próprio) ✅ CONCLUÍDO
**Task:** M7 fechado: console/eval in-process + rede completa (req+resp) sem Firefox. Próximo = sustentabilidade.

## Completed ✓

- **M7 done** (critério: forma funcional e verificada de inspecionar console/JS/rede / ADR-0010 / docs / push):
  - **T1 — servidor + ServoDelegate** (`da86e9f`): `init_manager` liga o servidor de devtools OPT-IN
    (`BASEDBROWSER_DEVTOOLS`, `ServoBuilder.preferences`, loopback porta fixa 7000 — `:0` é inútil, o
    Servo reporta a porta PEDIDA). `Embedder: ServoDelegate` (autoriza conexão + captura porta).
  - **T2 — console in-process** (`3623eb0`): `show_console_message` → buffer; console chega ao embedder
    INCONDICIONALMENTE. Driver `BASEDBROWSER_DEVTOOLS_TEST`.
  - **T3 — eval in-process** (`46cf3fd`): `evaluate_javascript` (REPL) + `format_jsvalue` (DOM via eval).
  - **T4 — cliente RDP de rede** (`9d969b0`): `src/devtools_client.rs` — conecta no servidor do Servo
    (loopback), handshake `root→listTabs→getWatcher→watchResources["network-event"]`, parseia
    req/resp/headers/payload, envia `NetRecord` por canal. Thread dedicada (ADR-0007).
  - **T5 — painel UI + fix de corrida** (`1c565b1`): `ui/app.slint` (Console + Rede) + `setup_devtools`
    (models/callbacks/drenagem). Fix: retry de `listTabs` (o cliente sobe cedo, a aba pode não existir).
  - **T6 — harness** (`e97bf1c`): `scripts/m7/verify-devtools.sh` + `pages/{devtools.html,data.json}`.
  - **T7 — fechar M7** (este commit): ADR-0010 + STATE (AD-013/L-010) + ROADMAP/HANDOFF/AGENTS + push.
  - **Verificado** (release, `scripts/m7/verify-devtools.sh`, 6 checagens ✅): console `hello-42`; eval
    `2+2→4` e `document.title→BBDEVTOOLS`; rede `GET /data.json status=200 OK` + response header; models
    do painel populados (`dev-console=12 / dev-net=6`).

## In Progress

- Nada — checkpoint limpo na `main` (T1–T7 commitados; push no fechamento). Gate verde (fmt/clippy
  `--exclude servo-poc`/6 testes). Nenhuma dep nova; config protegida intocada.

## Pending (próximos marcos)

1. **Sustentabilidade (Goal #3):** runbook + CI que testa a revisão fixada do Servo a cada atualização
   (mitiga L-001). Harness H3. — é o candidato natural a próximo marco.
2. **Outras plataformas** (Windows/DirectX, macOS/Metal, Android).
3. (deferido do M7) **Hardening do devtools por token** + **DevTools v2** (WebSocket/SSE, árvore de DOM
   visual, breakpoints) — ver Deferred Ideas no STATE.
4. (deferido do M6) **Downloads** (destrava com hook de resposta do Servo); **Modo privado**
   (`temporary_storage`). (deferido do M5) baseline absoluto; relatório interno do Servo.

## Blockers

- Nenhum ativo. Pendências humanas (não bloqueiam): conectores globais claude.ai (só na web); 2 deny
  rules do AgentShield no `settings.json` (precisa de OK explícito); README.md na raiz (opcional).

## Context

- Branch: `main` (github.com/VicenzoMF/based-browser). Idioma: **pt-BR**. Plan Mode antes de executar.
- **M7 (ADR-0010 / AD-013 / L-010):** console/eval são in-process (delegate + `evaluate_javascript`); a
  REDE só sai pelo socket RDP do servidor de devtools do Servo (crate hermético) → cliente próprio em
  `src/devtools_client.rs` (loopback, opt-in). Caveat "Firefox nightly" não se aplica (2 lados nossos,
  0.2.0 pinada). Verificação sem captura de janela (L-008): driver gated + `python3 http.server` + texto.
- **Reproduzir:** `cargo build --release -p basedbrowser && scripts/m7/verify-devtools.sh`.
- Decisões: STATE AD-001..013 · Lições: L-001..010 · ADRs: 0001..0010.
