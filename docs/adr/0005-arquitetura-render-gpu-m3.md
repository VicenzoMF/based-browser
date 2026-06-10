# ADR-0005: Arquitetura de render GPU do M3 (texture sharing zero-copy via memória externa Vulkan↔GL)

- **Status:** Proposed (o interop GL↔Vulkan é validado num gate no M3; promove a `Accepted`
  quando o pipeline zero-copy fechar com benchmark — ou registra o bloqueio + fallback se não fechar)
- **Data:** 2026-06-10
- **Relaciona-se com / supersede:** **supersede a parte de "transporte de frame" do ADR-0003**
  (cópia-CPU `read_to_image` → `SharedPixelBuffer` → `Image::from_rgba8`). **Mantém** do ADR-0003: o
  `OffscreenRenderingContext` de hardware, o Slint dono do loop/janela, o init lazy do contexto do
  Servo (L-004). **Mantém** o ADR-0004 (input/chrome/resize) intacto — o caminho de input independe
  do readback. Mantém o pin do ADR-0002 (`servo =0.2.0`, toolchain 1.92.0).

## Contexto

No M1/M2 o frame viaja Servo→Slint por **cópia-CPU**: a cada tick, `read_to_image` (readback GL
caro) → `SharedPixelBuffer` novo → `Image::from_rgba8` → `set_frame`. Esse roundtrip CPU por frame é
a causa estrutural do travamento medido no M2 (STATE L-005). O M1/M2 escolheram deliberadamente o
`OffscreenRenderingContext` de **hardware** (ADR-0003/AD-007) para que o M3 trocasse **só o
readback** por compartilhamento de textura GPU.

A pesquisa do M3 (jun/2026) confirmou **na fonte** (cache do cargo + arquivos reais do exemplo
oficial `slint-ui/slint/examples/servo`):

- **Servo 0.2.0** (`servo-paint-api-0.2.0/rendering_context.rs`): o `OffscreenRenderingContext`
  renderiza num FBO de GL com **textura RGBA real** em `COLOR_ATTACHMENT0`. `framebuffer_id`/
  `texture_id` são privados (sem getter), mas `prepare_for_rendering()` (trait público) faz bind do
  FBO → `glGetIntegerv(GL_FRAMEBUFFER_BINDING)` o expõe. `glow_gl_api() -> Arc<glow::Context>`
  (glow **0.17**), `gleam_gl_api()`, e `WindowRenderingContext::surfman_details() -> (Device,
  Context)` (surfman 0.12.x). **Não há API pública de dma-buf/FD** (EGLImage é interno ao surfman) →
  o FD de memória externa nasce no lado Vulkan, não no surfman.
- **Slint 1.16.1** (cache): `impl TryFrom<wgpu_28::Texture> for slint::Image` (exige
  `Rgba8Unorm`/`Rgba8UnormSrgb` + uso `TEXTURE_BINDING | RENDER_ATTACHMENT`), feature
  **`unstable-wgpu-28`**. `slint::wgpu_28::wgpu` **re-exporta o próprio crate wgpu** que o Slint usa
  (`pub use wgpu_28 as wgpu`) → dá pra usar a API wgpu pelo Slint sem dep `wgpu` separada (sem
  mismatch de versão). `BackendSelector::require_wgpu_28(WGPUConfiguration::{Automatic(WGPUSettings)|
  Manual{instance,adapter,device,queue}}).select()`. `set_rendering_notifier` expõe
  `GraphicsAPI::WGPU28 { instance, device, queue }`.
- **Exemplo oficial** (`examples/servo/.../rendering_context/vulkan.rs`, master): no Linux faz
  exatamente o zero-copy — Vulkan cria imagem com `ExternalMemoryImageCreateInfo(OPAQUE_FD)`,
  exporta FD (`ash::khr::external_memory_fd::get_memory_fd`); o GL do Servo importa
  (`glCreateMemoryObjectsEXT`/`glImportMemoryFdEXT`/`glTexStorageMem2DEXT`); cada frame
  `glBlitFramebuffer` (com Y invertido) do FBO do Servo → essa textura; embrulha a imagem Vulkan
  como `wgpu::Texture` via `create_texture_from_hal::<Vulkan>(texture_from_raw(...,
  TextureMemory::External))`; o VkDevice cru vem de `wgpu_device.as_hal::<Vulkan>()`. **Version
  skew:** o exemplo usa `servo 0.1.0` + wgpu-29; nós usamos `servo 0.2.0` + wgpu-28 (API wgpu-hal
  quase idêntica 28↔29; confirmar a 28 no gate).

## Decisão

1. **Renderer do Slint: femtovg/GL → `renderer-femtovg-wgpu` (Vulkan no Linux) + `unstable-wgpu-28`.**
   É decisão arquitetural (este ADR). Reformula a tensão "dois GL na mesma janela" do L-004: passa a
   ser **GL (surfman/Servo) + Vulkan (wgpu/Slint)** numa janela só, com ponte por memória externa —
   ao custo de introduzir o interop GL↔Vulkan (o ponto mais difícil do projeto).

2. **Transporte de frame zero-copy (substitui a cópia-CPU):**
   - Servo renderiza no `OffscreenRenderingContext` (mantido — preserva o resize do M2, ADR-0004).
   - Imagem Vulkan com memória externa → FD; importada no contexto GL do Servo como textura GL
     compartilhada (`gpu_bridge.rs`); embrulhada como `wgpu::Texture` para o Slint.
   - Hot path: `paint()` → origem do blit = FBO do offscreen (`prepare_for_rendering()` +
     `GL_FRAMEBUFFER_BINDING`) → `glBlitFramebuffer` (**flip Y**) p/ a textura compartilhada →
     sincroniza (`glFinish`/fence v1) → `Image::try_from(wgpu_texture)` → `set_frame`.
     `read_to_image` sai do caminho quente.
   - Resize: continua só no offscreen via `webview.resize` (ADR-0004) **+** recria a textura
     compartilhada no novo tamanho.

3. **Dono do device GPU:** preferir o device wgpu criado pelo Slint (capturado via
   `set_rendering_notifier`/`Automatic`), do qual extraímos o VkDevice cru por `as_hal::<Vulkan>()`
   (como o exemplo). Se o device automático não habilitar as extensões Vulkan de memória externa
   (`VK_KHR_external_memory_fd`), criar o device manualmente e injetar via
   `require_wgpu_28(Manual{…})`.

4. **Deps (Cargo.toml do crate `basedbrowser` — não é config protegida; registrado aqui):** features
   do slint `unstable-wgpu-28` + `renderer-femtovg-wgpu`; usar a API wgpu via `slint::wgpu_28::wgpu`
   (sem dep `wgpu` separada); `glow = "0.17"` (casar com o servo 0.2.0); `ash = "0.38"` (casar com a
   versão que o wgpu-hal da wgpu-28 usa) + loader (`gl`/`get_proc_address`) para as entry-points
   `*EXT` de memória externa que o glow 0.17 não expõe. `servo =0.2.0`, `rust-toolchain.toml` e os
   lints raiz permanecem **intocados**.

5. **`unsafe` isolado:** todo o FFI GL/Vulkan/FD vive em `gpu_bridge.rs`, com cada bloco justificado
   por `#[expect(unsafe_code, reason = "…")]` (a config raiz tem `unsafe_code = "warn"`).

## Alternativas rejeitadas

- **`BorrowedOpenGLTextureBuilder` (import de textura GL pura, sem Vulkan):** exigiria o contexto GL
  do surfman (Servo) **compartilhar texturas** com o contexto GL do femtovg (Slint). O
  `WindowRenderingContext::new` do servo 0.2.0 não expõe criação com share-group, e a textura do
  offscreen vive no contexto do surfman, não no do femtovg. O exemplo oficial escolheu Vulkan
  **justamente** porque o share GL não está disponível. → infeasível sem forkar o servo.
- **Forkar/patchar o servo** p/ expor getters do FBO/textura: viola o pin protegido (ADR-0002) e o
  embedding fino (lição do Verso, L-001). Só como último recurso documentado.
- **`SoftwareRenderingContext`:** já rejeitado no ADR-0003 (rasterização CPU descartável).

## Consequências

- (+) Elimina o readback+upload CPU por frame (ataca a causa do L-005); base p/ benchmark do M3.
- (+) Mantém input/chrome/resize do M2 e o `OffscreenRenderingContext` (troca só o transporte).
- (−) Introduz interop GL↔Vulkan: `unsafe` FFI (ash + wgpu-hal + entry-points `*EXT`), risco alto.
- (−) Coexistência surfman/GL + wgpu/Vulkan na mesma janela (L-004 reformulado); init lazy ainda vale.
- (−) 1ª build com as features wgpu recompila bastante (puxa wgpu).
- **Gate + fallback:** se o interop não fechar no timebox, manter a cópia-CPU funcionando (no
  femtovg-wgpu ou revertendo o renderer), sem build quebrado, e registrar o bloqueio aqui + STATE.

## Fontes (jun/2026)

- Servo 0.2.0: `servo-paint-api-0.2.0/rendering_context.rs` (`OffscreenRenderingContext`,
  `RenderingContext`, `WindowRenderingContext::surfman_details`), surfman 0.12.x, glow 0.17 (cache).
- Slint 1.16.1: `i-slint-core-1.16.1/graphics/wgpu_28.rs` (`TryFrom<wgpu_28::Texture>`,
  `WGPUConfiguration`/`WGPUSettings`), `i-slint-backend-selector-1.16.1/api.rs` (`BackendSelector`),
  `i-slint-core-1.16.1/api.rs` (`set_rendering_notifier`, `GraphicsAPI::WGPU28`) (cache).
- Exemplo oficial: `github.com/slint-ui/slint` master, `examples/servo/Cargo.toml` +
  `src/webview/rendering_context/{vulkan.rs,gpu_rendering_context.rs,servo_rendering_adapter.rs}`.
