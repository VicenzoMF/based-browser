# Roadmap

**Current Milestone:** M10 (performance) / M11 (robustez) — PLANNED (ver Future Considerations). **Todos os
3 Goals do PROJECT ✅ atacados** (footprint M5; motor M1–M3; sustentabilidade M8); **M9 = produto
apresentável** (UI repaginada + UX de navegação).
**Status:** M0–M9 ✅ concluídos (M0–M4 em 2026-06-10; M5–M8 em 2026-06-11; **M9 em 2026-06-12**). M5 = tese
VALIDADA (ADR-0008). M6 = recursos de usuário (cookies/Web Storage PERSISTEM; "limpar dados"; downloads
DEFERIDO, ADR-0009). M7 = devtools / inspeção in-app (console + eval + rede req/resp via cliente RDP
próprio, SEM Firefox; OPT-IN, ADR-0010). **M8 = sustentabilidade (Goal #3)** — CI na revisão fixada +
runbook medido de bump + archgate + sandbox sem egress (ADR-0011). **M9 = redesign da UI (chrome "dark
refinado") + UX de navegação** (atalhos/zoom/find/menu `⋯`/context/favicon; ADR-0012).

---

## M0 — Fundação & PoC do Motor ✅ CONCLUÍDO (2026-06-10)

**Goal:** Provar que o Servo compila e renderiza na máquina-alvo, isolado, antes de envolver o Slint. De-risking do maior ponto de incerteza do projeto.
**Target:** Exemplo mínimo `servo + winit` rodando localmente e abrindo uma página. **Atingido** — `crates/servo-poc`.

### Features

**Setup do projeto & toolchain** - DONE

- Repositório git + estrutura Cargo
- Deps de sistema do Servo no Ubuntu 24.04 validadas/instaladas (18 pkgs apt)
- Revisão fixada: `servo 0.2.0` (crates.io) + toolchain `1.92.0` (ADR-0002)

**PoC do motor isolado** - DONE

- `servo 0.2.0` compilado (build 7m20s)
- Exemplo mínimo `winit + WebView` portado (`crates/servo-poc`, embedding fino)
- URL aberta e **render confirmado** numa janela winit pura (sem Slint) — screenshot

---

## M1 — MVP: Slint hospeda o Servo ✅ CONCLUÍDO (2026-06-10)

**Goal:** Primeiros pixels ponta-a-ponta: uma janela Slint exibindo conteúdo renderizado pelo Servo (URL fixa, cópia-CPU). **Atingido** — `crates/basedbrowser` (Slint 1.16.1 + `servo` 0.2.0). Evidência: janela Slint exibindo HTML/CSS do Servo (screenshot confirmado pelo usuário). Detalhes em **ADR-0003**.

### Features

**Bridge de event loop** - DONE

- Slint dono da janela/loop (backend winit, renderer femtovg/GL)
- `EventLoopWaker` do Servo + `slint::Timer` (~60 Hz) dirigindo `spin_event_loop`; `WebViewDelegate::notify_new_frame_ready` → pump-on-dirty

**Render via cópia-CPU** - DONE

- Servo renderiza num **`OffscreenRenderingContext`** (FBO de GL de hardware) derivado da janela do Slint (feature `raw-window-handle-06`)
- `read_to_image` (RGBA8) → `SharedPixelBuffer` → `Image::from_rgba8` → `set_frame` a cada frame
- URL fixa via `file://` (HTML/CSS auto-contido) exibida dentro da UI Slint
- **Lição (ADR-0003):** init do contexto do Servo é LAZY (fora do `RenderingSetup` do femtovg) p/ não corromper o GL compartilhado

---

## M2 — Browser navegável ✅ CONCLUÍDO (2026-06-10)

**Goal:** Deixa de ser uma imagem estática e vira algo interativo e dirigível pelo usuário.
**Atingido** — `crates/basedbrowser` evoluiu o pipeline do M1 com input, chrome e resize. Evidência:
navegou ao **YouTube** via barra de URL (HTTPS/TLS) renderizado pelo Servo + texto digitado num
`<input>` (pointer+teclado), com scroll/voltar/avançar/recarregar/resize confirmados pelo usuário.
Decisões em **ADR-0004**. Detalhe: build **debug** + cópia-CPU por frame deixa páginas pesadas
travadas — esperado até o M3 (ver Lições/Deferred).

### Features

**Input** - DONE

- Pointer (clique/move) → `InputEvent::{MouseButton,MouseMove}`; scroll → `notify_scroll_event`
- Teclado → `InputEvent::Keyboard` (`slint::platform::Key` → `keyboard_types::NamedKey`/`Character`)
- Tradução no `src/input.rs` (decodificação a primitivos no `.slint`); mapeamento de coordenadas
  **identidade** via `physical-length` + `image-fit: fill` + contexto offscreen do tamanho da área web

**Chrome mínimo (.slint)** - DONE

- Barra de URL (`LineEdit` → `webview.load`; `parse_user_url` prefixa `https://`)
- Voltar / avançar / recarregar (`go_back`/`go_forward`/`reload`, guardados por `can_go_*`)
- Indicador de carregamento + título dinâmico, dirigidos pelo `WebViewDelegate` (`Embedder`)

**Resize dinâmico** - DONE

- `webview.resize` redimensiona só o `OffscreenRenderingContext` (FBO + reflow); o
  `WindowRenderingContext` pai NÃO é tocado (evita a colisão GL do L-004) — verificado sem corrupção

---

## M3 — Performance: render GPU ✅ CONCLUÍDO (2026-06-10)

**Goal:** Eliminar o gargalo da cópia-CPU por frame com compartilhamento de textura GPU.
**Atingido** — `crates/basedbrowser/src/gpu_bridge.rs`: o frame Servo→Slint NÃO passa mais por
cópia-CPU. Renderer do Slint trocado p/ `femtovg-wgpu` (Vulkan). Decisões em **ADR-0005** (arquitetura)
+ **ADR-0006** (validação). Input/chrome/resize do M2 intactos.

### Features

**Texture sharing Vulkan↔GL** - DONE

- Imagem Vulkan com memória externa (`OPAQUE_FD`) → FD (`vkGetMemoryFdKHR`) → import em OpenGL
  (`GL_EXT_memory_object_fd`: `glImportMemoryFdEXT`/`glTexStorageMem2DEXT`)
- Wrap como `wgpu::Texture` no lado Slint (`create_texture_from_hal::<Vulkan>` +
  `texture_from_raw(External)`) → `slint::Image::try_from`; device wgpu capturado via
  `set_rendering_notifier`
- Flip vertical (mismatch GL↔Vulkan) no `glBlitFramebuffer` + `glFinish` (sync v1)
- **Fallback** de cópia-CPU em runtime (não foi necessário)

**Benchmark cópia-CPU vs. GPU sharing** - DONE

- Harness `FrameBench` (env `BASEDBROWSER_BENCH`). Release, 1024×724, página animada @60fps:
  `pump_frame` mean **~5,4 ms (CPU) → ~3,1 ms (GPU)**, p95 ~6–9 → ~3,7 ms (**−40% média, −50% p95**)
- Evidência: readback da textura compartilhada **byte a byte idêntico** à fonte do Servo + página
  HTTPS real (example.com). Captura de janela bloqueada no Wayland → dump in-app (ADR-0003)

---

## M4 — Recursos de navegador ✅ CONCLUÍDO (2026-06-10)

**Goal:** Funcionalidades que tornam o browser usável no dia a dia (dentro dos limites de compat do Servo).
**Atingido** — `crates/basedbrowser` evoluiu o pipeline do M3 com multi-aba, histórico e favoritos.
Decisões em **ADR-0007**. Chrome migrado da macro inline grande p/ `ui/app.slint` (re-export inline,
SEM build.rs — mantém o gate de lint verde). Deps novas: `serde`/`serde_json`/`dirs`. 8 commits
atômicos (T1–T7 + T4b).

### Features

**Multi-aba** - DONE

- UM `Servo`, N `WebView`s (`TabManager`/`Tab`); cada aba com seu `OffscreenRenderingContext` (FBO
  próprio) derivado do `WindowRenderingContext` pai. Só a aba ATIVA é pintada/blitada — **reusa a ponte
  GPU zero-copy do M3** trocando só a origem do blit (FBO da ativa). Abas de fundo `set_throttled(true)`,
  não bombeadas (economia). Abrir (+)/fechar (×)/trocar (clique) na barra de abas; `window.open`/
  `target=_blank` abre nova aba (fila diferida). Input/navegação vão p/ a aba ativa.
- Evidência: abrir(1→2)→page2→trocar→fechar(2→1) com conteúdo distinto por aba (aba1 VERDE/page2 no
  FBO próprio, textura ativa final ROXO/aba0); `window.open` → 2 abas; sem panic/borrow reentrante.

**Histórico de sessão** - DONE

- Visitas gravadas (alimentadas por `notify_url_changed`), persistidas em `~/.config/basedbrowser/
  history.json` (dedup consecutivo + teto FIFO 1000). Painel (botão ☰) com lista + busca (revisitar) +
  autocomplete na barra de URL. Evidência: 8 visitas persistidas → painel popula (dedup), busca filtra,
  autocomplete sugere, revisita carrega.

**Favoritos** - DONE

- ★ adiciona a página atual; barra de favoritos (clique abre / × remove); persistidos em
  `bookmarks.json`. Evidência: ★ → arquivo com 1 entrada → 2ª execução CARREGA o favorito.

**Restauração de sessão** - DONE

- Abas abertas (URLs + índice ativo) salvas no exit, restauradas no start (`init_manager`); precede o
  `BASEDBROWSER_URL`. Evidência: RUN 1 salva 2 abas (ativa=1) → RUN 2 restaura 2 abas, ativa=1.

---

## M5 — Validar a tese: footprint/RAM vs. Chromium ✅ CONCLUÍDO (2026-06-11)

**Goal:** Provar (ou refutar) o **Goal #1 do PROJECT** ("footprint enxuto; medir RSS ocioso vs.
Chromium e documentar a diferença"), que sustenta a razão de existir do projeto e nunca foi medido.
**Atingido** — harness de medição reproduzível (`scripts/m5/`) + metodologia justa + **ADR-0008**
(datado, com números + veredito). **TESE VALIDADA**: o BasedBrowser é mais leve que o Chrome em todos
os estados; o "ordens de magnitude" do PROJECT foi **qualificado** (é ~1,8× em PSS, não 10×).

### Features

**Harness de medição de memória** - DONE

- `scripts/m5/measure.sh`: mede **RSS/PSS** somando a **árvore de processos** (PPID-walk de
  `/proc/<pid>/smaps_rollup` — children-file ausente no kernel). BasedBrowser é **single-process**
  (`Opts.multiprocess` default=`false`, confirmado na fonte); Chrome é multiprocess (13→18 procs).
  Perfil limpo, headful, **release** (L-005), K=5 (pass^k, mediana). **PSS** = métrica-título.
- `run.sh` roda a matriz; `pages/{idle,heavy}.html` (determinísticas, sem rede). Hook de produto
  `BASEDBROWSER_OPEN_TABS=N` (custo por-aba; embedding fino, L-001).
- Relatório interno do Servo (`create_memory_report`) **adiado** (L-001; veredito não depende).

**Baseline vs. Chromium + relatório** - DONE

- Baseline = `google-chrome-stable` (.deb), MESMA metodologia. **Números (release, K=5):**
  ocioso **BB 171,1 MiB PSS (1 proc) vs Chrome 314,7 MiB (13 proc) = 1,84×** (RSS ×5,2, mas RSS
  infla o Chrome); por-aba **5,5 vs 11,8 MiB = 2,16×**; pesada 205 vs 333 MiB. Veredito + metodologia
  em **ADR-0008** (números canônicos lá; saída do harness é gitignorada).

---

## M6 — Recursos de usuário (cookies/storage, limpar dados) ✅ CONCLUÍDO (2026-06-11)

**Goal:** Fechar a lacuna que impedia o uso no dia a dia: **persistência de cookies +
`localStorage`/`sessionStorage`** entre execuções + ação de **"limpar dados de navegação"**.
**Atingido** — decisões em **ADR-0009**. Downloads: spike concluído inviável → **deferido**. Nenhuma
dep nova; config protegida intocada. 5 commits atômicos (T1–T5).

### Features

**Persistência de cookies + Web Storage** - DONE

- `init_manager` aplica `ServoBuilder.opts(Opts{ config_dir: Some(~/.config/basedbrowser/servo/),
  ..Opts::default() })` (temporary_storage=false ⇒ persiste). Mexida MÍNIMA e aditiva na API do Servo
  (1 ponto; embedding fino, L-001); não reorganiza o init lazy do GL (L-004); honra `XDG_CONFIG_HOME`
  (preserva perfis-limpos do ADR-0008). **Verificado** (`scripts/m6/verify-persist.sh`): RUN2 lê o
  cookie+localStorage setados no RUN1.

**Limpar dados de navegação** - DONE

- Botão "Limpar dados" → `clear_cookies()` + `clear_site_data(sites, Local|Session)` (via
  `SiteDataManager`) + `persist::clear_history()`. PRESERVA favoritos e a sessão. Roda em callback de
  UI (fora do `spin_event_loop`; invariante anti-reentrância do ADR-0007). **Verificado**
  (`scripts/m6/verify-clear.sh`): cookies/histórico → 0, favoritos preservados.

**Downloads** - DEFERRED (spike concluído — ADR-0009)

- **Inviável na API estável do `servo 0.2.0`:** o embedder NÃO vê os headers da RESPOSTA
  (`load_web_resource` só dá a request; `.intercept()` FORNECE a resposta; `network_manager()` só
  cache; `fetch_async` interno; sem API de download/link/menu). Auto-detecção de
  `Content-Disposition: attachment` é arquiteturalmente impossível; o workaround degrada para
  "cole-uma-URL" a custo real. **Deferido** (não forçar); destrava quando o Servo expuser um hook de
  resposta/evento de download, ou num marco dedicado com stack HTTP próprio.

---

## M7 — Devtools / inspeção in-app ✅ CONCLUÍDO (2026-06-11)

**Goal:** Dar ao desenvolvedor uma forma de inspecionar **console, JS e rede** de uma página renderizada
pelo Servo. Era o marco de MAIOR incerteza de API. **Atingido** — inspeção in-app **sem Firefox externo**.
Decisões em **ADR-0010**. Nenhuma dep nova; config protegida intocada. 6 commits atômicos (T1–T6).

### Features

**Console + eval (in-process)** - DONE

- `WebViewDelegate::show_console_message` captura todo `console.log/warn/error/...` (incondicional, não
  depende do servidor de devtools); `WebView::evaluate_javascript` → `JSValue` dá um REPL e inspeção de
  DOM via eval. Buffer interior-mutável; UI escrita pelo LOOP/timer (ADR-0007).

**Rede (req+resp/headers/payload) via cliente RDP próprio** - DONE

- O dado de rede só sai pelo socket do servidor de devtools do Servo (crate hermético, sem consumo
  in-process). `src/devtools_client.rs` conecta nele (loopback), faz o handshake RDP
  (`root → listTabs → getWatcher → watchResources`) e extrai requisição + RESPOSTA (status/headers/
  payload), enviando à UI por canal (thread dedicada; ADR-0007). Servidor OPT-IN (`BASEDBROWSER_DEVTOOLS`,
  porta fixa loopback). O caveat "Firefox nightly" não se aplica (os 2 lados são nossos, 0.2.0 pinada).

**Painel no chrome** - DONE

- `ui/app.slint` (botão "DevTools"): aba Console (log ao vivo + REPL de eval) + aba Rede (lista método/
  status/URL + detalhe de headers/payload). Re-export inline (sem build.rs, L-007); só primitivos (AD-008).

**Segurança** - DONE

- Socket OFF por padrão (opt-in), bind só em `127.0.0.1`, conexão autorizada. Risco residual (processo
  local) aceito por ser opt-in/dev; hardening por token deferido (ADR-0010).

**Evidência** (sem captura de janela, L-008): driver `BASEDBROWSER_DEVTOOLS_TEST` + `scripts/m7/
verify-devtools.sh` — 6 checagens ✅ (console, eval 2+2 & document.title, rede status 200 + response
header, models do painel populados).

---

## M8 — Sustentabilidade (Goal #3) ✅ CONCLUÍDO (2026-06-11)

**Goal:** Fechar o **último Goal do PROJECT** ("atualizar a revisão fixada do Servo em **< 1 dia por
sprint**") e operacionalizar a defesa contra o risco existencial **L-001** (churn do Verso). A incerteza
era de INFRA ("cabe um CI completo do Servo num runner free?"). **Atingido** — decisões em **ADR-0011**.
Nenhuma dep nova; config protegida intocada. 6 commits atômicos (T0–T6).

### Features

**CI na revisão fixada** - DONE

- `.github/workflows/ci.yml`: push(main)+PR+manual; runner `ubuntu-24.04` free (repo público). Espelha o
  gate local: **archgate → fmt → clippy `--exclude servo-poc -D warnings` → test**. `free-disk-space`
  (~31 GB) → apt (~40 pkgs, loop resiliente a renames mesa) → `actions-rust-lang/setup-rust-toolchain`
  (lê o `rust-toolchain.toml`=1.92.0 + cache) → gate. Actions pinadas por SHA (L-002); `RUSTFLAGS`
  neutralizado. **Validado na prática:** 1º run a frio VERDE em ~15,5 min (cold-build do motor+mozjs cabe).

**Runbook de bump (medido)** - DONE

- `docs/runbooks/atualizar-servo.md` + `scripts/update-servo/run.sh`: bump determinístico num **git
  worktree isolado** (não toca o pin protegido da `main`), reusa o cache, **cronometra vs "< 1 dia"**.
  Dry-run validado (rehearsal 0.2.0, cache quente): gate **VERDE em ~81s**. `0.2.0` é a versão mais nova
  publicada (sem alvo de upgrade real ainda) → medição de churn de upgrade ocorre no próximo release.

**Archgate (ADR↔check) + sandbox sem egress** - DONE

- `scripts/checks/` (rodam no lefthook E no CI): `check-servo-pin` (pin nos 2 crates + toolchain = ADR-
  0002; divergência → exit 2 com instrução **ERRO/POR QUÊ/FIX/EXEMPLO**) + `check-adr-status`. Sandbox
  (`sandbox/`): `network_mode: none` + caps + non-root, no-egress **verificável** (smoke `OK: sem egress`);
  headful documentado c/ caveat de GPU/display (CI não roda — headless, L-008).

---

## M9 — Redesign da UI (chrome "dark refinado") + UX de navegação ✅ CONCLUÍDO (2026-06-12)

**Goal:** Tornar o browser apresentável (feedback do usuário: "a UI está horrível") e acoplar a UX de
navegação que faltava, SEM regredir função. **Atingido** — `ui/app.slint` repaginado + pontos cirúrgicos no
`src/main.rs`/`input.rs`. Direção visual aprovada no **Pencil** (`designs/based-browser.pen`). **ADR-0012**.

### Features

**Chrome dark refinado** - DONE

- `global Theme` (tokens slate `#15151b` + acento indigo `#6c5ce7`) + componentes (`IconBtn`, `LockIcon`
  desenhado, `MenuItem`, `TextBtn`/`SearchField` — substituem `Button`/`LineEdit` do std-widgets)
- Abas-pílula (favicon + elide + × no hover; ativa elevada + contorno accent); **omnibox** arredondada
  (`TextInput` cru + cadeado http/https + ★); toolbar em ícones (‹ › ⟳ ⋯); loading fino; **menu `⋯`**
  (Histórico/Limpar/DevTools/Zoom/Find). Menu/find/context = overlays por `bool` (padrão M4/M7)

**UX de navegação** - DONE

- **Zoom** nativo (`WebView::set_page_zoom`, Ctrl +/−/0, por-aba); **find-in-page** por injeção de JS
  (`setup_find` + TreeWalker — Servo 0.2.0 sem busca nativa); **favicons** (`notify_favicon_changed` →
  `slint::Image`); **atalhos** (Ctrl+T/W/L/R/Tab/F, Ctrl +/−/0, Esc) no `on_forward_key`; **menu de
  contexto** (right-click)
- **Páginas de erro:** o Servo já mostra a própria (não é tela branca) e não sinaliza falha ao embedder
  (#5463) → tema próprio **deferido** (decisão do usuário)
- **Lição (L-012):** box-layouts do Slint top-alinham filhos de tamanho fixo → componentes auto-centram

Evidência (L-008): design por screenshot do Pencil + gate verde (**9 testes**) + CI + smoke do usuário.
Nenhuma dep nova; config protegida intocada. 9 commits.

---

## Future Considerations (pós-M9)

- **M10 — Performance & responsividade:** sync GPU por fence/semáforo (no lugar do `glFinish` do M3);
  intervalo de polling adaptativo do event-loop.
- **M11 — Robustez & feedback:** crash de aba isolado (`WebViewDelegate::notify_crashed`); scroll restore.
- **Outras plataformas:** Windows/DirectX, macOS/Metal, Android (matriz multi-OS no mesmo CI).
- **Otimizar o baseline absoluto** (171 MiB ociosos; M5 só MEDIU) — candidato a marco futuro.
- **Downloads** / **modo privado** (deferidos do M6); **DevTools v2** / hardening por token (deferidos do M7).
- **Deferidos do M9:** página de erro temática (override de recurso / upstream #5463); find-in-page v2
  (regex/contexto); favicon un-premultiply.
- **sccache** no CI se o cache de 10 GB do GHA estourar (deferido do M8).
