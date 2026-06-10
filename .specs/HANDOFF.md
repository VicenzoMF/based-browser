# Handoff

**Date:** 2026-06-10
**Feature:** M2 — Browser navegável ✅ CONCLUÍDO · próximo = M3 (render GPU / texture sharing)
**Task:** M2 fechado: input + chrome + resize sobre o pipeline cópia-CPU do M1. Iniciar M3.

## Completed ✓

- **M2 done** (critério: digitar URL e navegar, clicar/scrollar/digitar na página, voltar/avançar/
  recarregar, indicador de carregamento, resize — com evidência):
  - **Input** (`src/input.rs`): pointer (`MouseButton`/`MouseMove`), scroll (`notify_scroll_event`,
    delta invertido), teclado (`slint::platform::Key` → `keyboard_types::NamedKey`/`Character`). O
    `.slint` decodifica a primitivos; mapeamento de coordenadas **identidade** (`physical-length` +
    `image-fit: fill` + offscreen do tamanho da área web).
  - **Chrome** (`MainWindow` inline + `wire_chrome` + `Embedder`): barra de URL (`load`/`parse_user_url`),
    voltar/avançar/recarregar (guardados por `can_go_*`), indicador de carregamento e título via
    `WebViewDelegate` (`notify_load_status_changed`/`url_changed`/`history_changed`/`page_title_changed`).
  - **Resize:** `webview.resize` redimensiona só o `OffscreenRenderingContext`; o `WindowRenderingContext`
    pai NÃO é tocado (evita a colisão GL do L-004) — verificado sem corrupção.
  - **Evidência confirmada pelo usuário:** YouTube renderizado via barra de URL (HTTPS/TLS;
    `/tmp/m2-youtube-evidence.png`); texto digitado num `<input>` (`/tmp/m2-start-frame.png`);
    scroll/voltar/avançar/recarregar/resize OK; **log sem erros de GL**.
  - **ADR-0004 `Accepted`** (input + resize). **AD-008** no STATE. **L-005** (travamento = debug +
    cópia-CPU; não é bug). Sem deps novas; waker real adiado.
  - Commits: `0317932` (T1 chrome), `9045702` (T2 pointer/scroll), `7389396` (T3 teclado),
    `72d58f9` (T4 navegação), `5d5727f` (T5 resize), + T6 (página de teste + docs).

## In Progress

- Nada — checkpoint limpo na `main` (T1–T5 commitados; T6 = página de teste + docs neste commit).

## Pending (M3 — Performance: render GPU)

1. **Texture sharing** Vulkan/dma-buf → import em GL/`wgpu` (eliminar a cópia-CPU por frame).
2. Slint no renderer `wgpu`; wrap da textura compartilhada; flip vertical + blit.
3. Benchmark cópia-CPU vs. GPU sharing (mede o ganho que destrava a L-005).
4. (barato, paralelo) **Waker real** p/ reduzir CPU ocioso do `Timer` 60 Hz.

## Blockers

- Nenhum ativo. Pendências humanas (não bloqueiam M3): conectores globais claude.ai (só na web);
  2 deny rules do AgentShield no `settings.json` (precisa de OK explícito).

## Context

- Branch: `main`. Idioma: **pt-BR**. Plan Mode antes de executar.
- **Perf (L-005):** travamento em páginas pesadas = build **debug** + **cópia-CPU** por frame (+
  Timer sem waker). `--release` ajuda; o M3 (texture sharing) elimina a causa estrutural. NÃO trocar
  arquitetura por isso antes do M3.
- **Resize (L-004 / ADR-0004):** redimensionar só o offscreen via `webview.resize`; nunca o
  `WindowRenderingContext` pai (resize concorrente das 2 superfícies GL corrompe o estado).
- **L-004 (M1):** init do contexto do Servo é LAZY (fora do `RenderingSetup`); `show()`+`focus()`
  obrigatórios; sequência `paint`→`make_current`→`read_to_image`.
- **Fonte do `servo` 0.2.0 no cache do cargo** é a referência mais confiável da API (re-exporta
  `embedder_traits::*` + `keyboard_types` + `webrender_api::units`).
- Rodar: `cargo run -p basedbrowser` (precisa de display). Dump de evidência:
  `BASEDBROWSER_DUMP_FRAME=/tmp/x.png cargo run -p basedbrowser`. Captura de **janela** automatizada
  segue bloqueada no GNOME 46/Wayland — usar dump in-app + screenshot manual.
- O M3 é a razão de o M1/M2 usarem `OffscreenRenderingContext` (hardware): o tipo já é o do caminho
  de GPU; troca-se só o readback.
- Decisões: STATE AD-001..008 · Lições: L-001..005 · ADRs: 0001..0004.
