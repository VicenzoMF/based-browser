# State

**Last Updated:** 2026-06-11
**Current Work:** **Marco M5 CONCLUÍDO** ✅ — **validar a tese (footprint/RAM vs. Chromium)**, o Goal #1 do PROJECT, que nunca tinha sido medido. Harness de medição **reproduzível** em bash (`scripts/m5/`: `measure.sh` soma a ÁRVORE DE PROCESSOS via `/proc/<pid>/smaps_rollup` — PPID-walk, pois o children-file está ausente no kernel; `run.sh` roda a matriz; `pages/{idle,heavy}.html` determinísticas, sem rede). Metodologia JUSTA (confirmada na fonte: `Opts.multiprocess` default=`false` → **BasedBrowser é single-process**; **Chrome é multiprocess**): perfil limpo nos dois (`XDG_CONFIG_HOME`/`--user-data-dir`), headful, **release** (L-005), K=5 (pass^k, mediana robusta), **PSS** como métrica-título (RSS junto). Única mudança de produto = hook env `BASEDBROWSER_OPEN_TABS=N` (abre N abas p/ o custo por-aba; embedding fino, L-001). **VEREDITO: tese VALIDADA** — BasedBrowser é mais leve que o Chrome em TODOS os estados: ocioso **171,1 MiB PSS (1 proc) vs 314,7 MiB (13 proc) = 1,84×**; por-aba **5,5 vs 11,8 MiB = 2,16×**; pesada 205 vs 333 MiB. PORÉM o "ordens de magnitude" do PROJECT é **refutado/qualificado** — na métrica justa (PSS) é ~1,8×, não 10× (o RSS ×5,2 infla o Chrome por contar páginas compartilhadas ~13×). Decisão+números em **ADR-0008** (datado, imutável; design-for-rot). Relatório interno do Servo (`create_memory_report`) **adiado** (L-001: 4+ superfícies de API de crate interno; veredito não depende). **Re-priorização (2026-06-11, usuário): M6 = recursos de usuário** (cookies/`localStorage` persistentes via `opts.config_dir`; downloads — sem API de 1ª classe no Servo, parte dura) **e M7 = devtools** (era M6). Sobre o M4 (ADR-0007/AD-010/L-007) abaixo.

**M4 (anterior):** **recursos de navegador** — o browser virou usável no dia a dia. **Multi-aba** (`TabManager`/`Tab` em `src/main.rs`): UM `Servo`, N `WebView`s, cada aba com seu `OffscreenRenderingContext` (FBO próprio) derivado do `WindowRenderingContext` pai; **só a aba ATIVA é pintada/blitada → reusa a ponte GPU zero-copy do M3 trocando só a origem do blit** (FBO da ativa); abas de fundo `set_throttled(true)`, não bombeadas. Barra de abas (`ui/app.slint`): abrir(+)/fechar(×)/trocar(clique); `window.open`/`target=_blank` via fila diferida (`pending_new`). O `Embedder` roteia callbacks por `webview.id()` → `TabState`, marca `chrome_dirty`, e o LOOP re-sincroniza a aba ativa → props do Slint (preserva o invariante anti-reentrância: delegate só faz borrow IMUTÁVEL; `manager` via `Weak`, sem ciclo Rc). **Persistência** (`src/persist.rs`, deps `serde`/`serde_json`/`dirs`): JSON em `~/.config/basedbrowser/` com escrita atômica (tmp+rename), tolerante a falha. **Favoritos** (★/barra, `bookmarks.json`), **histórico** (visitas por `notify_url_changed`, `history.json`, dedup+teto FIFO 1000; painel ☰ com busca + autocomplete na barra), **restauração de sessão** (`session.json`: URLs+ativa salvas no exit, restauradas no `init_manager`; precede o `BASEDBROWSER_URL`). **Chrome migrado** da macro inline grande p/ `ui/app.slint` via **re-export inline** (`slint::slint!(export {..} from "../ui/app.slint")`) — NÃO `build.rs`/`include_modules!()` (que injetaria o gerado como fonte do crate → 640 erros nos lints `deny`; a macro inline é isenta do clippy). Decisões em **ADR-0007** (estende 0003/0004/0005). **Evidência (smoke headless, captura de janela bloqueada no Wayland → drivers in-app + dumps):** abrir/trocar/fechar com conteúdo distinto por aba (aba1 VERDE/page2 no FBO próprio, ativa final ROXO/aba0); `window.open` → 2 abas; favoritos+histórico carregam entre execuções; sessão de 2 abas (ativa=1) restaurada. M0–M3 (zero-copy/input/resize) intactos; clippy `-D warnings`+fmt+6 testes verdes. **Próximo: M5** (a definir — ver ROADMAP "Future Considerations"). Pendências humanas (não bloqueiam): conectores globais claude.ai só na web; 2 deny rules do AgentShield no `settings.json`; README.md na raiz (opcional). Otimizações adiadas do M3: sync por fence/semáforo no lugar do `glFinish`; intervalo de polling adaptativo do event-loop.

---

## Recent Decisions (Last 60 days)

### AD-011: M5 — metodologia de medição de footprint + veredito da tese (2026-06-11)

**Decision:** Medir a tese do PROJECT (footprint vs. Chromium) com um harness bash em `scripts/m5/`
(NÃO um bin Rust — embedding fino, L-001), somando a **árvore de processos** inteira dos dois via
`/proc/<pid>/smaps_rollup` (PPID-walk; children-file ausente). Metodologia justa: BasedBrowser é
**single-process** (`multiprocess` default=`false`, confirmado na fonte) vs. Chrome **multiprocess**;
perfil limpo nos dois, headful, **release**, K=5 (mediana), **PSS** = métrica-título. Única mudança de
produto = hook `BASEDBROWSER_OPEN_TABS`. Números canônicos no **ADR-0008** (datado). Relatório interno
do Servo (`create_memory_report`) **adiado** (L-001).
**Reason:** O Goal #1 ("medir RSS ocioso vs. Chromium") nunca fora medido — era a maior dívida de
evidência do projeto. PSS é justo para árvore multiprocess + libs compartilhadas; mediana é robusta a
outliers de settle; release porque debug+métrica engana (L-005).
**Trade-off:** Não cabear o relatório interno deixa o "onde mora a memória" como sinal indireto (custo
marginal por-aba) em vez de breakdown JS-heap/layout. Aceito p/ não inflar o crate de churn.
**Impact:** **Tese VALIDADA** com evidência reproduzível (BB 1,84× mais leve ocioso em PSS; 2,16× por
aba), mas o "ordens de magnitude" do PROJECT foi **qualificado** (é ~1,8×, não 10×). Base honesta p/ o
M6 e p/ decidir se "otimizar o baseline absoluto" vira marco futuro.

### AD-010: M4 — multi-aba (N WebViews/1 Servo) + persistência + chrome em arquivo `.slint` (2026-06-10)

**Decision:** Promover o `Runtime` (single-WebView) a **`TabManager { tabs: Vec<Tab>, active, .. }`**: UM `Servo`, N `WebView`s; cada `Tab` com seu `OffscreenRenderingContext` (FBO próprio) derivado do único `WindowRenderingContext` pai (compartilham o contexto GL do surfman). **Só a aba ATIVA é pintada/blitada → reusa a ponte GPU do M3 trocando a origem do blit**; abas de fundo `set_throttled(true)`. `Embedder` roteia por `webview.id()` → `TabState` (interior-mutável), marca `chrome_dirty`; o LOOP escreve no Slint (anti-reentrância: delegate só borrow imutável; `manager` via `Weak`). `window.open` (`request_create_new`) via fila diferida drenada pós-spin. Persistência (`persist.rs`, `serde`/`serde_json`/`dirs`) em JSON sob `~/.config/basedbrowser/` (atômica, tolerante a falha): favoritos, histórico (dedup+teto FIFO 1000, painel+autocomplete), sessão (restaurada no `init_manager`). Chrome → `ui/app.slint` via **re-export da macro inline** (NÃO `build.rs`). Formalizado no **ADR-0007**.
**Reason:** O Servo suporta N WebViews por instância (confirmado na fonte: `examples/winit_minimal.rs`, `tests/webview.rs`); offscreen-por-aba (FBO próprio, contexto pai compartilhado) deixa a ponte GPU do M3 intacta (só muda o FBO de origem) e dá retenção de frame por aba. O re-export inline mantém a UI num `.slint` (LSP/preview) SEM quebrar o gate de lint (o `include_modules!()` injetaria o gerado como fonte → 640 erros `unwrap_used`/`expect_used`; a macro de crate externo é isenta do clippy).
**Trade-off:** Mais superfície de churn (múltiplas WebViews/contextos; classe do L-004 ao abrir abas em runtime — mitigado criando offscreen com o pai corrente). Histórico grava a cada visita (arquivo pequeno). Supera o "sem build.rs" da AD-008 (mas continua sem build.rs).
**Impact:** M4 fechado — browser usável (multi-aba, histórico, favoritos, sessão) com o zero-copy do M3 preservado. Base p/ M5.

### AD-009: M3 — render GPU zero-copy via memória externa Vulkan↔GL (2026-06-10)

**Decision:** Trocar o transporte de frame da cópia-CPU (`read_to_image`) por **texture sharing zero-copy**: renderer do Slint `femtovg-wgpu` (Vulkan, `unstable-wgpu-28`); imagem Vulkan com memória externa (`OPAQUE_FD`) exportada como FD e importada no GL do Servo (`GL_EXT_memory_object_fd`), embrulhada como `wgpu::Texture` (`as_hal`/`create_texture_from_hal`/`texture_from_raw(External)`) → `Image::try_from`. Blit do FBO offscreen → textura compartilhada (flip Y) + `glFinish`. Device wgpu capturado do Slint via `set_rendering_notifier`. Todo o `unsafe` isolado em `src/gpu_bridge.rs`. Formalizado em **ADR-0005** + **ADR-0006** (validação).
**Reason:** É o caminho do exemplo oficial slint-ui/servo, reconciliado com `servo 0.2.0`/`wgpu-28`. Elimina o readback+upload CPU por frame (causa do L-005). O device automático do Slint já habilita `VK_KHR_external_memory_fd` (wgpu-hal), então não precisou de device Manual.
**Trade-off:** `unsafe` FFI GL/Vulkan/FD (isolado); coexistência surfman/GL + wgpu/Vulkan na mesma janela (classe do L-004, mitigada); `glFinish` como sync v1 (custo dominante restante).
**Impact:** M3 fechado com ganho medido (−40% no pump). Base p/ M4. `BorrowedOpenGLTexture` (GL puro) foi rejeitado: o `WindowRenderingContext` do servo 0.2.0 não expõe share de contexto GL — por isso o caminho Vulkan.

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

### L-006: Interop GL↔Vulkan no M3 — flip de orientação + o que de-riscou o gate (2026-06-10)

**Context:** No M3, ao compartilhar memória externa Vulkan entre o GL do Servo e o wgpu/Vulkan do Slint.
**Problem/aprendizado:** (1) GL e Vulkan têm **ordem de linha oposta na MESMA memória** (GL row 0 = bottom; Vulkan row 0 = top). Sem flip, o Slint exibiria de cabeça pra baixo. **O `glBlitFramebuffer` faz o flip Y** (dst com Y invertido) → a textura compartilhada fica top-left, como o Slint amostra. Cuidado: a leitura de evidência via `glReadPixels` já sai na ordem que o Slint vê — **não** aplicar flip extra (errar isso dá um dump invertido enganoso). (2) O `gl` crate 0.14 **não** traz as entry-points `GL_EXT_memory_object[_fd]` — carregar à mão via `get_proc_address` do surfman. (3) O tiling da textura GL (`GL_OPTIMAL_TILING_EXT`) deve casar com o `vk::ImageTiling::OPTIMAL` da imagem Vulkan.
**Solution/de-risk (gate, tudo confirmado na fonte do cache antes de codar):** wgpu-hal da wgpu-28 **habilita `VK_KHR_external_memory_fd`** no device automático (adapter.rs `required_device_extensions`) → não precisou de device Manual. `slint::wgpu_28::wgpu` **re-exporta o próprio crate wgpu** (e `wgpu::hal`/`wgc`/`wgt`) → sem dep `wgpu` separada (zero mismatch). `as_hal::<Vulkan>()` dá `raw_device`/`raw_physical_device`/`shared_instance().raw_instance()`. `ash` deve casar a versão do wgpu-hal (0.38.x).
**Prevents:** semanas perdidas em "tela invertida/garbled" ou mismatch de versão no interop GPU. **Processo:** ler a fonte do cache do cargo (servo/slint/wgpu/wgpu-hal/surfman) é o que de-riscou o ponto mais difícil do projeto — não chutar a API.

### L-007: `slint::include_modules!()` quebra o gate de lint; a macro inline é isenta (2026-06-10)

**Context:** No M4 (T1), ao migrar o chrome da macro inline grande p/ um arquivo `ui/app.slint`, a 1ª tentativa foi o caminho "oficial": `build.rs` com `slint_build::compile` + `slint::include_modules!()`.
**Problem:** `include_modules!()` faz `include!()` do `app.rs` gerado pelo Slint, que vira **código-fonte do crate** — e o gerado usa `.unwrap()`/`.expect()` à vontade → **640 erros** nos lints `deny` do projeto (`unwrap_used`/`expect_used`). Sair disso exigiria `#[allow]` espalhado em código gerado (que ainda briga com `allow_attributes` e a filosofia "use `#[expect]`"). Foi por isso que o M2/M3 usaram a macro inline.
**Solution:** Manter a entrada pela macro `slint::slint!`, mas **re-exportando o arquivo**: `slint::slint!(export { MainWindow, .. } from "../ui/app.slint");`. A expansão de macro de **crate externo é isenta do clippy** → o gate fica verde sem `#[allow]`, e a UI vive num `.slint` (LSP/preview). Path relativo ao `.rs` (toolchain ≥1.88; confirmado em `slint-macros/lib.rs`).
**Prevents:** quebrar o gate de lint (ou poluir o código com `#[allow]` em saída de codegen) ao buscar "tooling" de UI. **Processo:** ADR-0007 registra a decisão; AD-008 ("sem build.rs") segue válida.

### L-008: Medir footprint multiprocess de forma justa — PSS + árvore + mediana (2026-06-11)

**Context:** No M5, ao comparar a memória do BasedBrowser (single-process) com a do Chrome (multiprocess).
**Problem/aprendizado:** (1) **RSS engana numa árvore multiprocess** — soma páginas compartilhadas
(binário, libs) uma vez por processo, inflando o Chrome (RSS ×5,2 vs PSS ×1,8 no ocioso). O **PSS**
(smaps_rollup) divide a página compartilhada entre os mapeadores → é a métrica honesta. (2) Comparação
só é justa **somando a árvore de processos inteira** dos dois (o Chrome subiu 13→18 processos; o
BasedBrowser ficou em 1 — `multiprocess` default=`false`). (3) O **children-file do /proc está ausente**
neste kernel (sem `CONFIG_PROC_CHILDREN`) → caminhar a árvore por **PPID** (`/proc/<pid>/stat`, campo
após o último `)`, robusto a `comm` com espaços). (4) Settle não-determinístico gera **outliers** (1/5
runs do N=6 amostrou antes das abas carregarem) → reportar **mediana**, não média. (5) `awk` em locale
pt-BR imprime `,` decimal → **`LC_ALL=C`** ou o JSON sai inválido.
**Prevents:** publicar um número de footprint enganoso (inflado por RSS ou por settle ruim) e "provar"
a tese com metodologia frouxa. **Processo:** o veredito (validada, mas ~1,8× e não "ordens de magnitude")
foi documentado honestamente no ADR-0008 — refutar a hipérbole é tão importante quanto validar o núcleo.

## Quick Tasks Completed

| #   | Description | Date | Commit | Status |
| --- | ----------- | ---- | ------ | ------ |

---

## Deferred Ideas

- [x] **Medição sistemática de RAM vs. Chromium para validar a tese central** — feito (M5, ADR-0008): harness `scripts/m5/`, **tese VALIDADA** (BB 1,84× mais leve ocioso em PSS, 2,16× por aba), "ordens de magnitude" qualificado p/ ~1,8×
- [ ] **Relatório interno do Servo** (`create_memory_report` → breakdown JS-heap/layout) cruzado com o RSS do SO — adiado no M5 (L-001: 4+ superfícies de API de crate interno; acessível via `servo::profile_traits`, `lib.rs:54`) — Captured during: M5
- [ ] **Otimizar o baseline absoluto** (171 MiB ociosos não são "featherweight"; single-process carrega SpiderMonkey+layout+wgpu/Vulkan+Slint) — candidato a marco futuro; M5 só MEDIU — Captured during: M5
- [ ] CI que testa a revisão fixada do Servo a cada atualização — Captured during: project init
- [ ] Render-diff / "olhos" E2E — **destravado (M1 ✅)**; nota: captura de **janela** automatizada está bloqueada no GNOME 46/Wayland (gdbus negado; `import`/X11 não vê Wayland). Caminho viável p/ E2E: dump in-app do frame (`BASEDBROWSER_DUMP_FRAME=<path>`) e comparar PNGs — Captured during: harness H2
- [ ] Conteúdo do runbook de update do Servo — destrava no M0 — Captured during: harness H3
- [ ] Custom lints com fix-instructions — adicionar quando o agente errar (princípio doc [A]) — Captured during: harness H3
- [ ] Ativar a sandbox `sandbox/docker-compose.yml` (rodar browser sobre URL não confiável) — M1 — Captured during: harness H3
- [x] **Waker real** (`EventLoopWaker` que acorda o loop sob demanda) — feito (T6/M3, AD-009): `ServoWaker` + spin adaptativo (60 Hz ativo / ~10 Hz ocioso, ramp por `wake()`/input). Sem regressão (62 fps animado). Achado: o CPU ocioso em release já era baixo (~5%); o gating de spin não o muda — o lever real é o intervalo de polling (abaixo)
- [ ] **Intervalo de polling adaptativo** (reschedule do `Timer` p/ frequência menor quando ocioso) — reduziria o custo ocioso do event-loop em si; risco de ramp de animação se o Servo não acordar sempre via `wake()` — Captured during: M3
- [ ] Tratar `Code` físico do teclado (hoje `Code::Unidentified`) p/ atalhos que dependem dele — Captured during: M2
- [ ] **Sync GPU por fence/semáforo** no lugar do `glFinish` do M3 (`GL_EXT_semaphore_fd` ↔ `VK_KHR_external_semaphore_fd`) — elimina o stall de sincronização (custo dominante restante do pump); ganho adicional sobre os ~3,1 ms — Captured during: M3

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
- [x] **M3: render GPU zero-copy** — feito (ADR-0005/0006, AD-009, L-006): texture sharing via memória externa Vulkan↔GL (`src/gpu_bridge.rs`), renderer `femtovg-wgpu`. Evidência: readback da textura compartilhada idêntico à fonte (byte a byte) + example.com via HTTPS. Benchmark: pump −40% (5,4→3,1 ms). Commits T0 (renderer), T1 (benchmark), T2–T4 (zero-copy)
- [x] **M4: recursos de navegador** — feito (ADR-0007, AD-010, L-007): multi-aba (`TabManager`/`Tab`, reusa a ponte GPU do M3), `window.open`, favoritos/histórico/sessão persistidos em JSON (`src/persist.rs`, `serde`/`serde_json`/`dirs`), painel+autocomplete de histórico, restauração de sessão. Chrome → `ui/app.slint` (re-export inline, sem build.rs). Evidência: drivers in-app (`BASEDBROWSER_TAB_TEST`/`BOOKMARK_TEST`/`HISTORY_TEST`) + dumps por aba. 8 commits (T1–T7 + T4b)
- [x] **M5: validar a tese (footprint vs. Chromium)** — feito (ADR-0008, AD-011, L-008): harness bash `scripts/m5/` (`measure.sh`+`run.sh`+`pages/`), PPID-walk de `/proc/smaps_rollup`, PSS-título, soma da árvore, perfil limpo, release, K=5. Hook `BASEDBROWSER_OPEN_TABS`. **Tese VALIDADA** (ocioso BB 171,1 MiB PSS / 1 proc vs Chrome 314,7 / 13 proc = 1,84×; por-aba 5,5 vs 11,8 MiB = 2,16×; pesada 205 vs 333 MiB). "Ordens de magnitude" qualificado p/ ~1,8× (PSS). Commits atômicos T1–T7

---

## Preferences

**Model Guidance Shown:** never
