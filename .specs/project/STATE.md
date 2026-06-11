# State

**Last Updated:** 2026-06-11
**Current Work:** **Marco M8 CONCLUÍDO** ✅ — **Sustentabilidade (Goal #3)**, o ÚLTIMO Goal do PROJECT ainda
não atacado; mitiga o risco existencial **L-001** (o Verso morreu afogado no churn do Servo) transformando
a lição num MECANISMO. Entregue: **(1) CI** (`.github/workflows/ci.yml`) que valida o gate na revisão
fixada (**archgate → fmt → clippy `--exclude servo-poc -D warnings` → test**) por push(main)+PR+manual,
runner `ubuntu-24.04` free (repo público). A incerteza era de INFRA ("cabe um CI completo do Servo num
runner free?"): RESOLVIDA NA PRÁTICA — o 1º run a frio passou por TODAS as etapas (free-disk-space ~31GB →
apt ~40 pkgs com loop resiliente a renames mesa → `actions-rust-lang/setup-rust-toolchain` lê o
`rust-toolchain.toml`=1.92.0 + cache → fmt → **clippy/cold-build do motor+mozjs VERDE** → test). NÃO
precisou degradar. Prova decisiva prévia: o próprio CI do Servo roda assim. Pegadinha resolvida: a action
seta `RUSTFLAGS=-D warnings` global por padrão → quebraria no warning de uma DEP (Servo) → **neutralizado**
(`rustflags: ""`); o gate de lint vem do nosso clippy explícito. **(2) Runbook** (`docs/runbooks/
atualizar-servo.md`) + `scripts/update-servo/run.sh`: mede um bump-candidato num **git worktree isolado**
(não toca o pin protegido da `main`), reusa o cache, cronometra vs a meta "< 1 dia". Dry-run validado
(rehearsal 0.2.0, cache quente): gate **VERDE em ~81s**; `0.2.0` é a versão mais nova publicada (sem alvo
de upgrade real ainda) → a medição de churn de upgrade ocorre no próximo release (runbook pronto); um bump
real = +~7 min de recompile do motor + triagem de churn, ambos << 1 dia. **(3) Archgate** (`scripts/
checks/`, HARNESS-ROADMAP H3): checks com **erro-como-instrução** que acoplam ADR↔regra — `check-servo-pin`
(pin nos 2 crates + toolchain = ADR-0002; divergência → exit 2 com FIX) + `check-adr-status`; rodam no
lefthook E no CI. **(4) Sandbox sem egress** (`sandbox/`): garantia central (`network_mode: none`)
**verificável** por smoke (`OK: sem egress`); render headful documentado com caveat de GPU/display (CI não
roda — headless, L-008). Decisões em **ADR-0011**; **AD-014** + **L-011** abaixo. **6 commits atômicos
(T0–T6).** Nenhuma dep nova; config protegida (pin/toolchain/lints/`.claude`/ADRs) intocada. **Próximo:
outras plataformas (Windows/DirectX, macOS/Metal, Android).**

**Marco M7 CONCLUÍDO** ✅ — **devtools / inspeção in-app** (console + eval + rede), **sem Firefox externo**. Era o marco de MAIOR incerteza de API; a pesquisa NA FONTE concluiu que (ao contrário dos downloads do M6) a inspeção É viável por dois caminhos: **console/eval são in-process** (`WebViewDelegate::show_console_message` recebe todo `console.log` INCONDICIONALMENTE; `WebView::evaluate_javascript` → `JSValue`, com refs de DOM ⇒ inspeção via eval), e **a rede só sai pelo socket do servidor de devtools do Servo** (o crate `servo-devtools` é hermético; sem consumo in-process — a parede do M6/L-009). **Decisão do usuário (Plan Mode): construir o NOSSO cliente RDP in-app** (`src/devtools_client.rs`) que conecta no servidor do próprio Servo (loopback) e entrega **rede COMPLETA (req+resp, headers, payload, status)** no nosso UI — o caveat "só Firefox nightly" do upstream NÃO se aplica (os 2 lados são nossos, na 0.2.0 pinada; protocolo fixo pelo pin). `init_manager` liga o servidor OPT-IN (`BASEDBROWSER_DEVTOOLS`, loopback, porta fixa 7000 — efêmera `:0` é inútil pois o Servo reporta a porta PEDIDA, não a real do listener); `Embedder` é `ServoDelegate` (autoriza a conexão + spawna o cliente cedo). Painel no chrome (`ui/app.slint`): aba Console (log ao vivo + REPL de eval) + aba Rede (lista + detalhe de headers/payload). Threading respeita o ADR-0007 (cliente em thread dedicada → canal `mpsc` → Timer drena na thread de UI; nada toca o Slint na thread do cliente). Segurança: socket OFF por padrão, loopback, conexão autorizada; risco residual (outro processo local) aceito por ser opt-in/dev (hardening por token deferido). Decisões em **ADR-0010**; **AD-013** + **L-010** abaixo. Evidência (sem captura de janela, L-008): driver `BASEDBROWSER_DEVTOOLS_TEST` + `scripts/m7/verify-devtools.sh` (6 checagens ✅: console hello-42, eval 2+2→4 e document.title→BBDEVTOOLS, rede GET status=200 + response header, models do painel populados). Nenhuma dep nova; config protegida intocada. 6 commits atômicos (T1–T6). **Próximo: sustentabilidade (runbook + CI do pin do Servo, Goal #3) e/ou outras plataformas.**

**Marco M6 CONCLUÍDO** ✅ — **recursos de usuário**: fecha a lacuna que impedia o uso no dia a dia. **Persistência de cookies + `localStorage`/`sessionStorage`** entre execuções — `init_manager` agora aplica `ServoBuilder.opts(Opts{ config_dir: Some(~/.config/basedbrowser/servo/), ..Opts::default() })` (temporary_storage=false ⇒ persiste; o Servo passa o `config_dir` p/ `new_resource_threads` (cookies) E `new_storage_threads` (storage)). Mexida MÍNIMA e aditiva na API do Servo (1 ponto; embedding fino, L-001); não reorganiza o init lazy do GL (L-004); honra `XDG_CONFIG_HOME` (perfis-limpos do ADR-0008). **"Limpar dados de navegação"** (botão no chrome) = `clear_cookies()` + `clear_site_data(sites, Local|Session)` via `SiteDataManager` + `persist::clear_history()`; PRESERVA favoritos e a sessão (convenção de browser); roda em callback de UI (fora do `spin_event_loop`; invariante anti-reentrância do ADR-0007). **Downloads: SPIKE CONCLUÍDO — inviável na API estável do `servo 0.2.0`** (o embedder não vê os headers da RESPOSTA; `load_web_resource` só dá a request, `.intercept()` FORNECE a resposta, `network_manager()` só cache, `fetch_async` interno; sem API de download/link/menu) ⇒ auto-detecção de attachment é arquiteturalmente impossível, o workaround degrada p/ "cole-uma-URL" a custo real → **DEFERIDO** (não forçar). Decisões+veredito em **ADR-0009**; **AD-012** + **L-009** abaixo. Evidência reproduzível (sem captura de janela, L-008): drivers gated `BASEDBROWSER_{PERSIST,CLEAR}_TEST` + `scripts/m6/` (`verify-persist.sh`: RUN2 lê `cookie=42 local=persisted-99`; `verify-clear.sh`: cookies/histórico→0, favoritos preservados). Nenhuma dep nova; config protegida intocada. 5 commits atômicos (T1–T5). **Próximo: M7 = devtools/inspeção.**

**Marco M5 CONCLUÍDO** ✅ — **validar a tese (footprint/RAM vs. Chromium)**, o Goal #1 do PROJECT, que nunca tinha sido medido. Harness de medição **reproduzível** em bash (`scripts/m5/`: `measure.sh` soma a ÁRVORE DE PROCESSOS via `/proc/<pid>/smaps_rollup` — PPID-walk, pois o children-file está ausente no kernel; `run.sh` roda a matriz; `pages/{idle,heavy}.html` determinísticas, sem rede). Metodologia JUSTA (confirmada na fonte: `Opts.multiprocess` default=`false` → **BasedBrowser é single-process**; **Chrome é multiprocess**): perfil limpo nos dois (`XDG_CONFIG_HOME`/`--user-data-dir`), headful, **release** (L-005), K=5 (pass^k, mediana robusta), **PSS** como métrica-título (RSS junto). Única mudança de produto = hook env `BASEDBROWSER_OPEN_TABS=N` (abre N abas p/ o custo por-aba; embedding fino, L-001). **VEREDITO: tese VALIDADA** — BasedBrowser é mais leve que o Chrome em TODOS os estados: ocioso **171,1 MiB PSS (1 proc) vs 314,7 MiB (13 proc) = 1,84×**; por-aba **5,5 vs 11,8 MiB = 2,16×**; pesada 205 vs 333 MiB. PORÉM o "ordens de magnitude" do PROJECT é **refutado/qualificado** — na métrica justa (PSS) é ~1,8×, não 10× (o RSS ×5,2 infla o Chrome por contar páginas compartilhadas ~13×). Decisão+números em **ADR-0008** (datado, imutável; design-for-rot). Relatório interno do Servo (`create_memory_report`) **adiado** (L-001: 4+ superfícies de API de crate interno; veredito não depende). **Re-priorização (2026-06-11, usuário): M6 = recursos de usuário** (cookies/`localStorage` persistentes via `opts.config_dir`; downloads — sem API de 1ª classe no Servo, parte dura) **e M7 = devtools** (era M6). Sobre o M4 (ADR-0007/AD-010/L-007) abaixo.

**M4 (anterior):** **recursos de navegador** — o browser virou usável no dia a dia. **Multi-aba** (`TabManager`/`Tab` em `src/main.rs`): UM `Servo`, N `WebView`s, cada aba com seu `OffscreenRenderingContext` (FBO próprio) derivado do `WindowRenderingContext` pai; **só a aba ATIVA é pintada/blitada → reusa a ponte GPU zero-copy do M3 trocando só a origem do blit** (FBO da ativa); abas de fundo `set_throttled(true)`, não bombeadas. Barra de abas (`ui/app.slint`): abrir(+)/fechar(×)/trocar(clique); `window.open`/`target=_blank` via fila diferida (`pending_new`). O `Embedder` roteia callbacks por `webview.id()` → `TabState`, marca `chrome_dirty`, e o LOOP re-sincroniza a aba ativa → props do Slint (preserva o invariante anti-reentrância: delegate só faz borrow IMUTÁVEL; `manager` via `Weak`, sem ciclo Rc). **Persistência** (`src/persist.rs`, deps `serde`/`serde_json`/`dirs`): JSON em `~/.config/basedbrowser/` com escrita atômica (tmp+rename), tolerante a falha. **Favoritos** (★/barra, `bookmarks.json`), **histórico** (visitas por `notify_url_changed`, `history.json`, dedup+teto FIFO 1000; painel ☰ com busca + autocomplete na barra), **restauração de sessão** (`session.json`: URLs+ativa salvas no exit, restauradas no `init_manager`; precede o `BASEDBROWSER_URL`). **Chrome migrado** da macro inline grande p/ `ui/app.slint` via **re-export inline** (`slint::slint!(export {..} from "../ui/app.slint")`) — NÃO `build.rs`/`include_modules!()` (que injetaria o gerado como fonte do crate → 640 erros nos lints `deny`; a macro inline é isenta do clippy). Decisões em **ADR-0007** (estende 0003/0004/0005). **Evidência (smoke headless, captura de janela bloqueada no Wayland → drivers in-app + dumps):** abrir/trocar/fechar com conteúdo distinto por aba (aba1 VERDE/page2 no FBO próprio, ativa final ROXO/aba0); `window.open` → 2 abas; favoritos+histórico carregam entre execuções; sessão de 2 abas (ativa=1) restaurada. M0–M3 (zero-copy/input/resize) intactos; clippy `-D warnings`+fmt+6 testes verdes. **Próximo: M5** (a definir — ver ROADMAP "Future Considerations"). Pendências humanas (não bloqueiam): conectores globais claude.ai só na web; 2 deny rules do AgentShield no `settings.json`; README.md na raiz (opcional). Otimizações adiadas do M3: sync por fence/semáforo no lugar do `glFinish`; intervalo de polling adaptativo do event-loop.

---

## Recent Decisions (Last 60 days)

### AD-014: M8 — sustentabilidade (CI hospedado free + runbook medido + archgate + sandbox) (2026-06-11)

**Decision:** Atacar o **Goal #3** (sustentabilidade) com um **CI completo no GitHub Actions** (espelha o
gate local: archgate→fmt→clippy `-D warnings`→test; push+PR+manual; runner `ubuntu-24.04` free) + um
**runbook determinístico medido** de bump do pin (`docs/runbooks/atualizar-servo.md` + `scripts/update-
servo/run.sh` em git worktree isolado) + **archgate** (ADR↔check executável, erro-como-instrução) +
**sandbox sem egress** (verificável). Formalizado no **ADR-0011**.
**Reason:** É o único Goal não atacado e a defesa direta contra o L-001 (churn do Verso): updates do
Servo passam a **falhar alto** (CI/archgate) em vez de em silêncio, e o procedimento de bump é **medível**
vs "< 1 dia". A incerteza era INFRA (cabe no runner free?) — resolvida na prática: o cold-build do motor
passou verde; o CI do próprio Servo já provava a viabilidade.
**Trade-off:** ~40 pkgs apt (não 18); cache do GHA tem teto de 10 GB (fallback: cache só de `~/.cargo` /
sccache, deferido); sandbox headful precisa de GPU/display via passthrough (no-egress é a garantia
verificável); `RUSTFLAGS=-D warnings` da action precisou ser neutralizado p/ não quebrar no warning de dep.
**Impact:** Goal #3 fechado; L-001 operacionalizado por mecanismo. Nenhuma dep nova; config protegida
intocada; embedding fino preservado. Base p/ "outras plataformas" (matriz multi-OS no mesmo CI).

### AD-013: M7 — devtools in-app (console/eval in-process + cliente RDP próprio p/ rede) (2026-06-11)

**Decision:** Entregar inspeção in-app SEM Firefox: **console + eval in-process**
(`show_console_message` + `evaluate_javascript`) e **rede completa via um cliente RDP NOSSO**
(`src/devtools_client.rs`) que conecta no servidor de devtools do próprio Servo (loopback, OPT-IN por
`BASEDBROWSER_DEVTOOLS`). Painel no `ui/app.slint` (Console + Rede). Formalizado no **ADR-0010**.
**Reason:** O dado de rede COMPLETO (req+resp/headers/payload) existe mas só sai pelo socket RDP (o
crate `servo-devtools` é hermético — sem consumo in-process). O usuário (Plan Mode) escolheu o cliente
próprio em vez do Firefox externo; o caveat "Firefox nightly" não se aplica (os 2 lados são nossos, na
0.2.0 pinada → protocolo fixo pelo pin). Console/eval são caminhos in-process de 1ª classe.
**Trade-off:** ~300 linhas de protocolo RDP que mantemos (revisitadas nos sprints de update do pin);
porta FIXA (o Servo reporta a porta pedida, não a real → `:0` inútil); rede só popula com o opt-in
ligado; sem WebSocket/breakpoints/árvore-DOM-visual no v1 (DOM via eval).
**Impact:** Inspeção real destrava o dev sobre o Servo. Embedding fino (1 ponto no `ServoBuilder` +
`set_delegate`); nenhuma dep nova. Threading respeita o ADR-0007 (cliente em thread → canal → UI).
Segurança: socket OFF por padrão, loopback, conexão autorizada; risco residual (processo local) aceito
por ser opt-in/dev (hardening por token deferido).

### AD-012: M6 — persistência por padrão + limpar dados; downloads deferido (2026-06-11)

**Decision:** LIGAR a persistência de cookies/Web Storage POR PADRÃO (`opts.config_dir` setado em
`init_manager`, `temporary_storage=false`); adicionar "limpar dados de navegação" (cookies + Web
Storage via `SiteDataManager` + nosso histórico, PRESERVANDO favoritos/sessão); e **DEFERIR downloads**
(spike concluiu inviável na API estável do `servo 0.2.0`). Formalizado no **ADR-0009**.
**Reason:** Persistir é a lacuna que o M6 existe p/ fechar (logins sobrevivem); privacidade fica sob
controle do usuário via "limpar dados" (convenção de browser preserva favoritos). Downloads: o Servo
0.2.0 não expõe os headers da RESPOSTA ao embedder (nem API de download/link/menu) → auto-detecção
impossível e workaround degradado a custo real → não forçar (confirmado na fonte).
**Trade-off:** Sem modo efêmero (candidato a toggle futuro); clear de Web Storage é escopado por
domínio registrado (localhost/IPs não enumerados — caveat); sem downloads no M6.
**Impact:** Browser usável no dia a dia (logins persistem; controle de dados). Mexida na API do Servo
mínima (1 ponto; embedding fino, L-001). Base p/ o M7 (devtools). Nenhuma dep nova.

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

### L-009: Downloads no Servo 0.2.0 — sem inspeção de RESPOSTA ⇒ auto-detecção impossível (2026-06-11)

**Context:** No M6 (spike de downloads), avaliando como detectar e salvar um arquivo quando o servidor
responde `Content-Disposition: attachment`.
**Problem/aprendizado:** o que DEFINE downloads (detectar a resposta) depende de ver os **headers da
RESPOSTA**, e o Servo 0.2.0 **não os expõe ao embedder**: `WebViewDelegate::load_web_resource`/
`WebResourceLoad` só dá a *request*; `.intercept()` faz você FORNECER a resposta (não recebe bytes);
`Servo::network_manager()` só mexe em cache; `net_traits::fetch_async` é crate interno; `EmbedderMsg`/
`WebViewDelegate` não têm variante de download; não há API de menu/contexto de link. Um download
"iniciado pelo usuário" (GET nosso) degrada p/ "cole-uma-URL" (sem contexto de link) e exige hand-roll
de HTTP/TLS (~250 linhas mantidas por nós) ou dep nova fora do cache.
**Solution:** **deferir** o download completo, documentando a razão técnica + onde destrava no ADR-0009
(não forçar uma feature degradada nem inflar o crate de churn — L-001). O spike cumpriu seu papel:
provar inviabilidade é um resultado tão válido quanto implementar.
**Prevents:** queimar esforço num subsistema HTTP paralelo (ou numa UX degradada) por uma capacidade
que a API estável do motor simplesmente não suporta hoje. **Processo:** confirmar NA FONTE (4 crates do
servo) antes de decidir; a decisão de escopo (deferir) foi do usuário, informada pelo spike.

### L-010: DevTools do Servo — console/eval in-process, mas rede só via socket RDP (2026-06-11)

**Context:** No M7, mapeando o que dá p/ inspecionar (console/DOM/rede) com a API estável do `servo 0.2.0`.
**Problem/aprendizado:** três regimes MUITO diferentes na MESMA feature: (1) **console** chega ao
embedder INCONDICIONALMENTE (`show_console_message`; `dom/console.rs` emite sempre, separado do gating
de devtools) e **eval** é 1ª classe (`evaluate_javascript` → `JSValue` com refs de DOM) — ambos
in-process, fáceis; (2) **rede** tem o dado COMPLETO (req+resp/headers/payload) mas o crate
`servo-devtools` é HERMÉTICO (só `pub fn start_server`) — **zero consumo in-process**; só sai por um
**socket TCP** falando o protocolo RDP do Firefox. Detalhes que de-riscaram o cliente: o handshake
mínimo é `root → listTabs → getWatcher → watchResources["network-event"]` (wire `<len>:<json>`),
`network-event` NÃO tem snapshot (só eventos futuros → o cliente tem que subir CEDO e fazer **retry de
`listTabs`** até a aba existir, senão trava numa corrida de timing); status/headers/payload vêm em
`resources-updated-array` + pedidos `getResponse{Headers,Content}` ao `NetworkEventActor`; e o Servo
**reporta a porta PEDIDA, não a real** do listener (`lib.rs:202-203`) → porta efêmera `:0` é inútil.
**Solution:** console/eval direto no delegate; rede por um cliente RDP NOSSO (loopback, mesma 0.2.0
pinada → não é "Firefox-frágil"), em thread dedicada + canal p/ a UI (ADR-0007). Mapear o wire NA FONTE
**e** capturar pacotes reais com um probe (python) antes de codar o parser de-riscou o ponto mais incerto.
**Prevents:** travar o handshake numa corrida de timing, ou assumir consumo in-process que não existe, ou
usar porta efêmera que o Servo não reporta. **Processo:** ler a fonte + probe empírico > chutar o protocolo.

### L-011: CI do Servo cabe num runner free — mas tem 3 pegadinhas de infra (2026-06-11)

**Context:** No M8, montando um CI hospedado que compila o motor Servo + mozjs do fonte (incerteza:
"cabe num runner GitHub free?").
**Problem/aprendizado:** o build CABE (4 vCPU/16 GB/14 GB SSD em repo público; o próprio CI do Servo roda
assim), mas com **3 pegadinhas** que custariam um cold-build vermelho (~30 min) cada p/ descobrir: (1)
**disco** — os 14 GB de fábrica NÃO bastam; `jlumbroso/free-disk-space` (libera ~31 GB) é OBRIGATÓRIO,
não opcional (`tool-cache: false` p/ preservar toolchains). (2) **`RUSTFLAGS=-D warnings` global** —
`actions-rust-lang/setup-rust-toolchain` seta isso por PADRÃO, e como `RUSTFLAGS` se aplica a TODAS as
deps, um único warning do Servo/mozjs (que não controlamos) quebraria o build → **`rustflags: ""`** e
deixar o gate de lint no clippy explícito (que só linta nossos crates). (3) **nomes apt transitórios** —
`libegl1-mesa-dev`/`libgles2-mesa-dev` mudam entre 22.04/24.04 → loop resiliente que instala só o
disponível (incluindo as duas formas) e LOGA o que pulou, em vez de `apt-get install` estourar num nome
ausente. Bônus: `actions-rust-lang/setup-rust-toolchain` **respeita o `rust-toolchain.toml`** (instala
exatamente o channel pinado) — `dtolnay/rust-toolchain` NÃO lê o arquivo (duplicaria o pin).
**Solution:** de-riscar os 3 pontos ANTES do 1º push (pesquisa na fonte + Cargo.lock/testes headless
checados localmente) e validar o resto **empiricamente** (push + `gh run watch`). Actions pinadas por SHA
(L-002). O caminho degradado (CI manual/local) ficou documentado mas não foi necessário.
**Prevents:** queimar ciclos de cold-build (~30 min cada) descobrindo disco/RUSTFLAGS/apt no vermelho, ou
assumir que "CI completo roda de boa" sem reconfirmar na prática (padrão de honestidade M6/M7).
**Processo:** ADR-0011 + archgate (ADR↔check) operacionalizam a decisão; runbook mede o Goal #3.

## Quick Tasks Completed

| #   | Description | Date | Commit | Status |
| --- | ----------- | ---- | ------ | ------ |

---

## Deferred Ideas

- [x] **Medição sistemática de RAM vs. Chromium para validar a tese central** — feito (M5, ADR-0008): harness `scripts/m5/`, **tese VALIDADA** (BB 1,84× mais leve ocioso em PSS, 2,16× por aba), "ordens de magnitude" qualificado p/ ~1,8×
- [ ] **Relatório interno do Servo** (`create_memory_report` → breakdown JS-heap/layout) cruzado com o RSS do SO — adiado no M5 (L-001: 4+ superfícies de API de crate interno; acessível via `servo::profile_traits`, `lib.rs:54`) — Captured during: M5
- [ ] **Otimizar o baseline absoluto** (171 MiB ociosos não são "featherweight"; single-process carrega SpiderMonkey+layout+wgpu/Vulkan+Slint) — candidato a marco futuro; M5 só MEDIU — Captured during: M5
- [ ] **Downloads de arquivos** — deferido no M6 (L-009/ADR-0009): inviável na API estável do `servo 0.2.0` (embedder não vê headers de resposta; sem API de download/link/menu). Destrava quando o Servo expuser um hook de resposta/evento de download, OU num marco dedicado com cliente HTTP próprio — Captured during: M6
- [ ] **Modo privado/efêmero** (toggle `Opts.temporary_storage=true` na UI) — M6 persiste por padrão; um modo sem persistência fica p/ depois — Captured during: M6
- [ ] **`clear_session_cookies()`** (limpar só cookies de sessão) como opção granular no "limpar dados" — Captured during: M6
- [ ] **Hardening do devtools por token** — exigir o `auth_token` (de `OnDevtoolsStarted`) em vez de autorizar toda conexão de loopback, fechando o risco residual de outro processo local conectar (ADR-0010). O quirk de comprimento do token (`servo-devtools/lib.rs:879`, `{:X}` de u32 com <8 dígitos hex) precisa ser contornado — Captured during: M7
- [ ] **DevTools v2** — WebSocket/SSE na aba Rede, árvore de DOM visual (hoje DOM via eval), breakpoints/debugger (atores `thread`/`source`/`breakpoint` existem no `servo-devtools`), e talvez consumir o console TAMBÉM pelo RDP p/ stacktraces — Captured during: M7
- [x] **CI que testa a revisão fixada do Servo a cada atualização** — feito (M8, ADR-0011): `.github/workflows/ci.yml` (archgate+fmt+clippy+test) verde no runner free; cold-build do motor cabe — Captured during: project init
- [ ] Render-diff / "olhos" E2E — **destravado (M1 ✅)**; nota: captura de **janela** automatizada está bloqueada no GNOME 46/Wayland (gdbus negado; `import`/X11 não vê Wayland). Caminho viável p/ E2E: dump in-app do frame (`BASEDBROWSER_DUMP_FRAME=<path>`) e comparar PNGs — Captured during: harness H2
- [x] **Conteúdo do runbook de update do Servo** — feito (M8, ADR-0011): `docs/runbooks/atualizar-servo.md` + `scripts/update-servo/run.sh` (medido vs "< 1 dia"; dry-run rehearsal verde em ~81s) — Captured during: harness H3
- [x] **Custom lints com fix-instructions** — feito (M8): archgate (`scripts/checks/`) com erro-como-instrução (ERRO/POR QUÊ/FIX/EXEMPLO), acoplando ADR↔check (pin/toolchain/ADR-status). Adicionar mais quando o agente errar — Captured during: harness H3
- [x] **Ativar a sandbox `sandbox/docker-compose.yml`** — feito (M8): no-egress verificável (smoke `OK: sem egress`); render headful documentado c/ caveat GPU/display — Captured during: harness H3
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
- [x] **M6: recursos de usuário** — feito (ADR-0009, AD-012, L-009): persistência de cookies + Web Storage via `opts.config_dir` (`init_manager`); "limpar dados de navegação" (cookies/storage via `SiteDataManager` + histórico, preserva favoritos); downloads DEFERIDO (inviável na API estável do Servo 0.2.0). Evidência: drivers `BASEDBROWSER_{PERSIST,CLEAR}_TEST` + `scripts/m6/` (verify-persist: RUN2 lê `cookie=42 local=persisted-99`; verify-clear: cookies/histórico→0, favoritos preservados). Nenhuma dep nova. 5 commits T1–T5
- [x] **M7: devtools / inspeção in-app** — feito (ADR-0010, AD-013, L-010): console (`show_console_message`) + eval (`evaluate_javascript`) in-process + **cliente RDP próprio** (`src/devtools_client.rs`) p/ rede completa (req+resp/headers/payload) conectando no servidor de devtools do Servo (loopback, OPT-IN `BASEDBROWSER_DEVTOOLS`); painel no `ui/app.slint` (Console + Rede). Evidência: driver `BASEDBROWSER_DEVTOOLS_TEST` + `scripts/m7/verify-devtools.sh` (6 checagens ✅). Nenhuma dep nova. 6 commits T1–T6
- [x] **M8: sustentabilidade (Goal #3)** — feito (ADR-0011, AD-014, L-011): **CI** (`.github/workflows/ci.yml`, archgate+fmt+clippy+test) verde no runner free (cold-build do motor cabe; prova: o CI do próprio Servo) + **runbook** medido (`docs/runbooks/atualizar-servo.md` + `scripts/update-servo/run.sh`, worktree isolado; dry-run rehearsal verde ~81s vs meta < 1 dia) + **archgate** (`scripts/checks/`, ADR↔check erro-como-instrução) + **sandbox sem egress** (smoke `OK: sem egress`). Pegadinhas de infra (L-011): free-disk-space obrigatório, `rustflags:""`, apt resiliente a renames. Nenhuma dep nova; config protegida intocada. 6 commits T0–T6

---

## Preferences

**Model Guidance Shown:** never
