# ADR-0009: M6 — Persistência (cookies/Web Storage) + privacidade, limpar dados, e o spike de downloads

- **Status:** Accepted
- **Data:** 2026-06-11
- **Relaciona-se com:** **estende** (não supersede) ADR-0007 (recursos M4 — persistência em JSON,
  invariante anti-reentrância). Mantém o pin do ADR-0002 (`servo =0.2.0`, toolchain 1.92.0) e os
  perfis-limpos do ADR-0008 (`XDG_CONFIG_HOME`). Nenhuma config protegida alterada; **nenhuma dep nova.**

## Contexto

M0–M5 entregaram um browser multi-aba dirigível com a tese de footprint validada (ADR-0008). Mas uma
lacuna o impedia de ser usável no dia a dia: rodávamos `ServoBuilder::default()` **sem `Opts`**
(`src/main.rs`), então **cookies e `localStorage`/`sessionStorage` NÃO persistiam** — todo login/estado
de site se perdia a cada execução. (Favoritos, histórico e sessão de abas já persistiam desde o M4, via
`src/persist.rs`.) O M6 fecha essa lacuna, adiciona uma ação de **"limpar dados de navegação"**, e
resolve (via spike) a viabilidade de **downloads**.

### Confirmado NA FONTE (cache do cargo do `servo 0.2.0`)

- **Persistência:** `Opts.config_dir: Option<PathBuf>` (default `None`) flui para `new_resource_threads`
  (cookies) **e** `new_storage_threads` (local/session); `temporary_storage: bool` (default `false`)
  afeta ambos. ⇒ Setar `config_dir` com `temporary_storage=false` LIGA a persistência.
- **Limpar:** `Servo::site_data_manager() -> &SiteDataManager` expõe `clear_cookies()`,
  `clear_session_cookies()`, `clear_site_data(&[&str], StorageType)`, `site_data(StorageType)`,
  `cookies_for_url(Url, CookieSource)` — **síncronos**, borrow imutável. `StorageType` = bitflags
  `{Cookies, Local, Session}` (tem `::all()`).
- **Downloads:** **não há API de 1ª classe.** O Servo 0.2.0 **não expõe os headers da RESPOSTA** ao
  embedder: `WebViewDelegate::load_web_resource`/`WebResourceLoad` só dá a *request*; `.intercept()`
  **fornece** a resposta (não recebe bytes); `Servo::network_manager()` só mexe em cache HTTP;
  `net_traits::fetch_async` é crate interno; `EmbedderMsg`/`WebViewDelegate` não têm variante de
  download; não há API de menu de contexto / link.

## Decisão

### 1. Persistência LIGADA por padrão (decisão de privacidade)

No `init_manager`, aplicamos `ServoBuilder::default().opts(Opts { config_dir: Some(dir),
..Opts::default() })`, com `dir = ~/.config/basedbrowser/servo/` (subdir próprio p/ não colidir com
nossos `*.json`; honra `XDG_CONFIG_HOME` via `dirs`, preservando os perfis-limpos do ADR-0008). Mexida
**mínima e aditiva** na API do Servo (embedding fino, L-001): 1 ponto, não reorganiza a ordem de init
(o contexto GL segue lazy, L-004). Sem `config_dir` disponível, cai no default (sem persistência) em
vez de falhar.

**Privacidade:** persistir por padrão é o **comportamento normal de browser** — logins/estado de site
sobrevivem entre execuções, que é exatamente a lacuna que o M6 fecha. O usuário tem controle explícito
via "limpar dados de navegação" (Decisão 2). Não há modo privado/efêmero no M6 (candidato a marco
futuro: um toggle `temporary_storage`).

### 2. "Limpar dados de navegação" = dados de navegação, **preserva favoritos**

Botão "Limpar dados" no chrome → `clear_browsing_data()`: `clear_cookies()` (todos os cookies,
domain-independent) + `clear_site_data(sites, Local|Session)` (enumerando `site_data()` p/ a lista de
sites) + `persist::clear_history()`. **PRESERVA** favoritos (`bookmarks.json`) e a sessão de abas —
curadoria do usuário, por convenção de browser (igual ao "limpar dados de navegação" de Chrome/Firefox).
Roda num callback de UI (FORA do `spin_event_loop`) → só borrow IMUTÁVEL do `manager` p/ pegar
`&servo` (respeita o invariante anti-reentrância do ADR-0007). Métodos síncronos/bloqueantes são
aceitáveis p/ uma ação pontual do usuário.

**Caveat (honesto):** `clear_site_data` é escopado por SITE (eTLD+1, domínio registrado). Origens sem
domínio registrado (`localhost`/IPs — uso de dev) podem não ser enumeradas por `site_data()` p/ a
limpeza de Web Storage; para domínios reais limpa normalmente. Cookies são sempre limpos (via
`clear_cookies()`, domain-independent) e o histórico sempre zera.

### 3. Downloads — SPIKE CONCLUÍDO: **inviável na API estável ⇒ DEFERIDO**

O que *define* downloads num browser — o servidor responde `Content-Disposition: attachment` e o
browser **detecta** e salva o arquivo — depende de **inspecionar os headers da RESPOSTA**, que o Servo
0.2.0 **não expõe ao embedder** (ver Contexto). O único workaround seria um download *iniciado pelo
usuário* (GET nosso), mas: (a) sem API de menu/contexto de link, ele **degrada para um utilitário
"cole-uma-URL"** (um botão "baixar" baixaria o HTML da página atual, não um arquivo linkado); (b)
exigiria um cliente HTTP — hand-roll de TLS+HTTP/1.1+redirect+chunked sobre o `rustls` existente
(~250 linhas que passaríamos a manter, contra o embedding fino) **ou** uma dep nova (`ureq`/`reqwest`,
fora do cache do cargo → rede + autorização). O valor entregue seria baixo e o custo real.

**Decisão (alinhada à pré-decisão do usuário "não forçar"):** **documentar a limitação e DEFERIR** o
download para um marco futuro. Sem código de download no M6. **Destrava quando:** o Servo expuser um
hook de inspeção de resposta / evento de download de 1ª classe (rastrear upstream), **ou** num marco
dedicado que aceite conscientemente um subsistema HTTP paralelo ao stack de rede do Servo.

## Evidência (reproduzível, sem captura de janela — Wayland, L-008)

Drivers in-app gated por env + saída em TEXTO + harness bash em `scripts/m6/` (perfil REAL isolado via
`XDG_CONFIG_HOME` temporário; página servida por `python3 -m http.server` em `127.0.0.1` — origem http
real, `file://` não persiste confiável).

- **Persistência** (`scripts/m6/verify-persist.sh`, driver `BASEDBROWSER_PERSIST_TEST`): RUN1 (perfil
  novo) lê `cookie=MISS local=MISS` e seta; **RUN2 (mesmo perfil) lê `cookie=42 local=persisted-99`** —
  cookie E `localStorage` sobreviveram ao restart (+ cookie `bb_test=42` lido do jar via
  `cookies_for_url`).
- **Limpar** (`scripts/m6/verify-clear.sh`, driver `BASEDBROWSER_CLEAR_TEST`): **antes**
  `cookies(aba)=1 history=1 bookmarks=1` → **depois** `cookies=0 history=0 bookmarks=1` (cookies +
  histórico zerados; favoritos preservados).

## Consequências

- (+) Browser usável no dia a dia: logins/estado de site sobrevivem; controle de privacidade via
  "limpar dados". Reproduzível e verificado.
- (+) Embedding ainda fino (1 ponto no `ServoBuilder`; API estável `SiteDataManager`); nenhuma dep nova;
  config protegida intocada.
- (−) Persistência por padrão é uma escolha de privacidade (sem modo efêmero ainda) — mitigado pelo
  "limpar dados" e por um futuro toggle `temporary_storage`.
- (−) Limpeza de Web Storage por-site só p/ domínios registrados (caveat acima).
- (−) **Sem downloads no M6** — quem quiser salvar um arquivo ainda não consegue (deferido, com a razão
  técnica registrada aqui).

## Alternativas rejeitadas

- **Download manual agora (hand-roll rustls ou dep `ureq`):** rejeitado — UX degradada
  (cole-uma-URL, sem auto-detecção/contexto de link) a custo real (subsistema de rede próprio ou dep
  nova). Não vale forçar; deferido (Decisão 3).
- **Modo efêmero/privado por padrão (`temporary_storage=true`):** rejeitado p/ o M6 — manteria a
  lacuna que o marco existe p/ fechar. Vira candidato a toggle futuro.
- **Limpar tudo (incl. favoritos):** rejeitado — favoritos são curadoria do usuário; convenção de
  browser preserva-os no "limpar dados de navegação".

## Fontes (jun/2026 — cache do cargo, `…/index.crates.io-1949cf8c6b5b557f/`)

- `servo-config-0.2.0/opts.rs`: `Opts.config_dir` (l.61), `temporary_storage` (l.64); `impl Default`
  (l.214-237: `config_dir: None`, `temporary_storage: false`).
- `servo-0.2.0/servo.rs`: `config_dir` → `new_resource_threads` (l.927) e `new_storage_threads`
  (l.935-936, com `temporary_storage`); `ServoBuilder::opts(Opts)` (l.1392); `site_data_manager()`
  (l.1049); `network_manager()` (só cache).
- `servo-0.2.0/lib.rs`: `pub use servo_config::opts::{…, Opts, …}` (l.60); `pub use net_traits::CookieSource` (l.43).
- `servo-0.2.0/site_data_manager.rs`: `StorageType` bitflags (l.20-42); `site_data` (l.130),
  `clear_site_data` (l.206), `clear_cookies` (l.227), `cookies_for_url` (l.240).
- `servo-net-traits-0.2.0/lib.rs`: `enum CookieSource { HTTP, NonHTTP }` (l.1096).
- **Downloads (inviabilidade):** `servo-embedder-traits-0.2.0/lib.rs` (`EmbedderMsg` sem download;
  `WebResourceRequest`/`WebResourceResponse` — só headers de request expostos ao delegate);
  `servo-0.2.0/webview_delegate.rs` (`load_web_resource`/`WebResourceLoad`/`.intercept`,
  `request_navigation` só allow/deny); `servo-net-0.2.0/request_interceptor.rs` (embedder FORNECE a
  resposta); `servo-net-traits-0.2.0/lib.rs` (`fetch_async` interno).
- Harness: `scripts/m6/{verify-persist.sh,verify-clear.sh,pages/persist.html}`.
