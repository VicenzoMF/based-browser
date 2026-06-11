# Handoff

**Date:** 2026-06-10
**Feature:** M4 — Recursos de navegador ✅ CONCLUÍDO · próximo = M5 (a definir)
**Task:** M4 fechado: multi-aba + histórico + favoritos + restauração de sessão. Iniciar M5.

## Completed ✓

- **M4 done** (critério: multi-aba funciona / abas de fundo não gastam / histórico registra+revisita /
  favoritos persistem / sessão restaura — tudo com evidência; M0–M3 intactos):
  - **T1 — chrome → `ui/app.slint`** (re-export inline `slint::slint!(export {..} from "../ui/app.slint")`).
    NÃO `build.rs`/`include_modules!()` — o gerado viraria fonte do crate e quebraria os lints `deny`
    (640 erros `unwrap_used`); a macro inline é isenta do clippy (L-007). ADR-0007 §5.
  - **T2 — `src/persist.rs`** (deps `serde`/`serde_json`/`dirs`): JSON em `~/.config/basedbrowser/`,
    escrita atômica (tmp+rename), tolerante a falha. Models Bookmark/HistoryEntry/Session + `AppData`.
    6 testes unitários. Histórico alimentado por `notify_url_changed`; sessão salva no exit.
  - **T3 — `Runtime`→`TabManager`** (1 aba = paridade M3): N WebViews/1 Servo; cada `Tab` com seu
    `OffscreenRenderingContext` (FBO próprio); `Embedder` roteia por `webview.id()` → `TabState`;
    ponte GPU do M3 reusada (blit da aba ativa). Zero-copy preservado.
  - **T4 — barra de abas** (abrir/fechar/trocar + throttle de fundo) **+ T4b — `window.open`** (fila
    diferida `pending_new` drenada pós-spin).
  - **T5 — favoritos** (★/barra, persistido) · **T6 — histórico** (painel ☰ com busca + autocomplete)
    · **T7 — restauração de sessão** (`init_manager`/`restore_session`; precede `BASEDBROWSER_URL`).
  - **ADR-0007** (estende 0003/0004/0005) + **AD-010** + **L-007** no STATE. 8 commits atômicos.
  - **Evidência** (captura de janela bloqueada no Wayland → drivers in-app + dumps): abrir/trocar/
    fechar com conteúdo distinto por aba (aba1 VERDE/page2, ativa final ROXO/aba0); `window.open` →
    2 abas; favoritos+histórico carregam entre runs; sessão de 2 abas (ativa=1) restaurada.

## In Progress

- Nada — checkpoint limpo na `main` (T1–T7 + T4b commitados e pushados; este commit = ADR-0007 + docs).

## Pending (próximos marcos definidos)

1. **M5 — validar a tese: footprint/RAM vs. Chromium** (Goal #1 do PROJECT, nunca medido). Harness de
   medição de RSS/PSS (ocioso + por-aba, somando a árvore de processos; `Servo::create_memory_report`)
   + baseline vs Chromium + relatório versionado. Ver ROADMAP "M5".
2. **M6 — devtools/inspeção** (decisão do usuário). Depende do M5 fechado.
3. (barato, paralelo) **Sync GPU por fence/semáforo** no lugar do `glFinish` do M3.
4. (futuro) **Intervalo de polling adaptativo** do event-loop; recursos de usuário (downloads/cookies);
   CI de atualização do Servo.

## Blockers

- Nenhum ativo. Pendências humanas (não bloqueiam): conectores globais claude.ai (só na web); 2 deny
  rules do AgentShield no `settings.json` (precisa de OK explícito); README.md na raiz (opcional).

## Context

- Branch: `main` (pushado p/ github.com/VicenzoMF/based-browser). Idioma: **pt-BR**. Plan Mode antes de executar.
- **M4 (ADR-0007 / AD-010 / L-007):** offscreen-por-aba (FBO próprio) compartilha o contexto surfman do
  pai → a ponte GPU do M3 só troca a origem do blit (FBO da ativa). Anti-reentrância: o delegate só faz
  borrow IMUTÁVEL do `manager` (via `Weak`, sem ciclo) + escreve `Cell`s + marca `chrome_dirty`; o LOOP
  escreve no Slint. `window.open` adia o registro da aba (fila `pending_new`). Persistência atômica e
  tolerante a falha. Chrome inline-re-export (NÃO build.rs) p/ não quebrar o gate de lint.
- **Drivers de evidência** (in-app, gated por env): `BASEDBROWSER_TAB_TEST` (abre/troca/fecha via
  `invoke_*`), `BASEDBROWSER_BOOKMARK_TEST`, `BASEDBROWSER_HISTORY_TEST`, `BASEDBROWSER_EXIT_AFTER_MS`
  (exit limpo → roda o save-on-exit), `BASEDBROWSER_DUMP_FRAME` (fonte + `.gpu.png` da aba ativa).
- Rodar: `cargo run -p basedbrowser` (precisa de display; renderer Vulkan/wgpu). Config em
  `~/.config/basedbrowser/{bookmarks,history,session}.json`.
- Decisões: STATE AD-001..010 · Lições: L-001..007 · ADRs: 0001..0007.
