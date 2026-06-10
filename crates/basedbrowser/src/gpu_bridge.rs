//! Ponte de textura GPU **zero-copy** entre o Servo (render em GL via surfman) e o Slint (render em
//! wgpu/Vulkan) — o núcleo do Marco **M3** (ADR-0005). Substitui o readback CPU (`read_to_image`)
//! por compartilhamento de **memória externa Vulkan** importada como textura GL.
//!
//! Fluxo (Linux):
//! 1. Cria uma imagem **Vulkan** (`R8G8B8A8_UNORM`, tiling OPTIMAL) com `VkExternalMemoryImageCreateInfo`
//!    (handle `OPAQUE_FD`); aloca memória dedicada exportável (`VkExportMemoryAllocateInfo`) e exporta
//!    um **FD** com `vkGetMemoryFdKHR`.
//! 2. Importa esse FD no **contexto GL do Servo** (`glCreateMemoryObjectsEXT` + `glImportMemoryFdEXT`)
//!    e cria uma **textura GL** sobre essa memória (`glTexStorageMem2DEXT`, tiling OPTIMAL p/ casar com
//!    a imagem Vulkan), anexada a um FBO (alvo de blit).
//! 3. Embrulha a MESMA imagem Vulkan como `wgpu::Texture`
//!    (`create_texture_from_hal::<Vulkan>(texture_from_raw(..., TextureMemory::External))`), de onde
//!    sai um `slint::Image` (`Image::try_from`). A imagem Vulkan e a textura GL apontam para a **mesma
//!    memória** → o que o GL escreve, o Slint vê, sem cópia pela CPU.
//!
//! Por frame, o chamador faz `glBlitFramebuffer` do FBO do Servo → o FBO desta textura (com flip Y) e
//! sincroniza (`glFinish`). Todo o `unsafe` (FFI Vulkan/GL/FD) está isolado neste módulo, justificado
//! bloco a bloco por comentários `SAFETY:`. As entry-points `GL_EXT_memory_object[_fd]` não estão nas
//! bindings do crate `gl`, então são carregadas à mão via `get_proc_address` do surfman; os handles
//! Vulkan crus vêm do device wgpu do Slint via `as_hal::<Vulkan>()`.

// Módulo de interop FFI GPU: cada bloco `unsafe` é justificado por um comentário `SAFETY:` local.
#![expect(
    unsafe_code,
    reason = "interop FFI GPU do M3 (Vulkan/GL/FD); cada bloco unsafe tem justificativa SAFETY local — ADR-0005"
)]

use std::ffi::c_void;
use std::sync::OnceLock;

use ash::vk;
use slint::wgpu_28::wgpu;
use wgpu::hal::api::Vulkan;

// --- Constantes de GL_EXT_memory_object / GL_EXT_memory_object_fd (ausentes no crate `gl`) ---
const GL_TEXTURE_TILING_EXT: u32 = 0x9580;
const GL_OPTIMAL_TILING_EXT: u32 = 0x9584;
const GL_HANDLE_TYPE_OPAQUE_FD_EXT: u32 = 0x9586;

// --- Assinaturas das entry-points GL_EXT_memory_object[_fd] (carregadas via get_proc_address) ---
type PfnCreateMemoryObjects = unsafe extern "system" fn(n: i32, memory_objects: *mut u32);
type PfnImportMemoryFd =
    unsafe extern "system" fn(memory: u32, size: u64, handle_type: u32, fd: i32);
type PfnTexStorageMem2D = unsafe extern "system" fn(
    target: u32,
    levels: i32,
    internal_format: u32,
    width: i32,
    height: i32,
    memory: u32,
    offset: u64,
);
type PfnDeleteMemoryObjects = unsafe extern "system" fn(n: i32, memory_objects: *const u32);

/// Ponteiros para as entry-points `GL_EXT_memory_object[_fd]`. Ponteiros de função são `Send`+`Sync`.
struct GlMemExt {
    create_memory_objects: PfnCreateMemoryObjects,
    import_memory_fd: PfnImportMemoryFd,
    tex_storage_mem_2d: PfnTexStorageMem2D,
    delete_memory_objects: PfnDeleteMemoryObjects,
}

/// `None` = pelo menos uma entry-point ausente (extensão não suportada → fallback CPU).
static GL_MEM_EXT: OnceLock<Option<GlMemExt>> = OnceLock::new();

fn gl_mem_ext() -> Option<&'static GlMemExt> {
    GL_MEM_EXT.get().and_then(Option::as_ref)
}

/// Carrega um ponteiro de função GL por nome; `None` se a entry-point não existir.
fn load_proc<T, F: FnMut(&str) -> *const c_void>(get: &mut F, name: &str) -> Option<T> {
    let ptr = get(name);
    if ptr.is_null() {
        None
    } else {
        // SAFETY: `ptr` não-nulo é o endereço da entry-point GL `name`, cuja ABI casa com `T`
        // (`extern "system"`, tamanho de ponteiro). `transmute_copy` reinterpreta o ponteiro como o
        // ponteiro de função `T` — uso padrão de carregamento dinâmico de GL.
        Some(unsafe { std::mem::transmute_copy::<*const c_void, T>(&ptr) })
    }
}

/// Carrega as entry-points GL (core via crate `gl` + as `*EXT` de memória externa à mão), uma vez,
/// usando o `get_proc_address` do surfman do Servo. Deve rodar com o contexto do Servo corrente.
pub fn load_gl_with<F: FnMut(&str) -> *const c_void>(mut get_proc: F) {
    gl::load_with(&mut get_proc);
    let ext = (|| {
        Some(GlMemExt {
            create_memory_objects: load_proc(&mut get_proc, "glCreateMemoryObjectsEXT")?,
            import_memory_fd: load_proc(&mut get_proc, "glImportMemoryFdEXT")?,
            tex_storage_mem_2d: load_proc(&mut get_proc, "glTexStorageMem2DEXT")?,
            delete_memory_objects: load_proc(&mut get_proc, "glDeleteMemoryObjectsEXT")?,
        })
    })();
    let _ = GL_MEM_EXT.set(ext);
}

/// Lê o FBO atualmente ligado a `GL_FRAMEBUFFER` (o offscreen do Servo, após `prepare_for_rendering`).
#[must_use]
pub fn bound_framebuffer() -> gl::types::GLuint {
    let mut id: gl::types::GLint = 0;
    // SAFETY: query de estado GL no contexto corrente; escreve um único `GLint`.
    unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &raw mut id) };
    u32::try_from(id).unwrap_or(0)
}

/// Textura compartilhada GPU (memória Vulkan vista por GL e por wgpu/Slint). Mantém os handles GL
/// (textura/FBO/memory-object) p/ o blit e a limpeza, e a `wgpu::Texture` (que, ao ser destruída,
/// libera a imagem/memória Vulkan via drop-callback). A `slint::Image` é derivada sob demanda.
pub struct SharedFrameTexture {
    wgpu_texture: wgpu::Texture,
    gl_texture: gl::types::GLuint,
    gl_fbo: gl::types::GLuint,
    gl_memory_object: gl::types::GLuint,
    width: u32,
    height: u32,
}

impl SharedFrameTexture {
    /// Tamanho (em px) desta textura compartilhada.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Deriva um `slint::Image` que referencia a textura wgpu (zero-copy). Criado a cada frame para
    /// sinalizar ao Slint que há conteúdo novo (a memória subjacente é atualizada in-place pelo blit).
    pub fn slint_image(&self) -> Result<slint::Image, String> {
        slint::Image::try_from(self.wgpu_texture.clone())
            .map_err(|e| format!("Image::try_from(wgpu): {e:?}"))
    }

    /// Faz `glBlitFramebuffer` de `src_fbo` (FBO renderado pelo Servo) para o FBO desta textura, com
    /// **flip vertical** (GL é bottom-left; a textura/Slint é top-left), e sincroniza com `glFinish`
    /// para garantir que o Slint (Vulkan) só amostre depois das escritas GL terminarem. Requer o
    /// contexto GL do Servo corrente.
    pub fn blit_from(&self, src_fbo: gl::types::GLuint) {
        let w = i32::try_from(self.width).unwrap_or(i32::MAX);
        let h = i32::try_from(self.height).unwrap_or(i32::MAX);
        // SAFETY: chamadas GL puras no contexto corrente do Servo. `src_fbo` e `self.gl_fbo` são FBOs
        // válidos e completos nesse contexto; o blit lê/escreve apenas regiões dentro de `w`x`h`.
        unsafe {
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, src_fbo);
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, self.gl_fbo);
            // dst com Y invertido (h -> 0) = flip vertical.
            gl::BlitFramebuffer(0, 0, w, h, 0, h, w, 0, gl::COLOR_BUFFER_BIT, gl::NEAREST);
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, 0);
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
            gl::Finish();
        }
    }

    /// Lê de volta o conteúdo da textura compartilhada (glReadPixels no FBO dela) e salva em PNG
    /// (sufixo `.gpu.png`), sobrescrevendo — evidência de que o zero-copy carrega os pixels certos (a
    /// captura de janela está bloqueada no Wayland). FORA do caminho quente. Requer contexto corrente.
    /// `log` controla o eprintln (uma vez).
    pub fn dump_shared(&self, base_path: &str, log: bool) {
        let Some(image) = self.read_back() else {
            if log {
                eprintln!("[m3] read_back da textura GPU falhou");
            }
            return;
        };
        let path = format!("{base_path}.gpu.png");
        match image.save(&path) {
            Ok(()) => {
                if log {
                    eprintln!(
                        "[m3] textura GPU compartilhada salva em {path} ({}x{})",
                        image.width(),
                        image.height()
                    );
                }
            }
            Err(e) => eprintln!("[m3] falha ao salvar dump GPU: {e}"),
        }
    }

    /// Lê o conteúdo da textura compartilhada via `glReadPixels` no FBO dela. A textura já está na
    /// orientação que o Slint amostra (o blit aplicou o flip GL→top-left), e o `glReadPixels` lê na
    /// mesma ordem — então a imagem resultante é um proxy fiel do que a janela exibe (sem flip extra).
    #[must_use]
    fn read_back(&self) -> Option<image::RgbaImage> {
        let w = i32::try_from(self.width).ok()?;
        let h = i32::try_from(self.height).ok()?;
        let mut pixels = vec![0u8; (self.width as usize) * (self.height as usize) * 4];
        // SAFETY: lê o FBO desta textura (válido/completo) no contexto corrente; o buffer tem
        // exatamente width*height*4 bytes, casando com a região lida.
        unsafe {
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.gl_fbo);
            gl::ReadPixels(
                0,
                0,
                w,
                h,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                pixels.as_mut_ptr().cast::<c_void>(),
            );
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, 0);
        }
        image::RgbaImage::from_raw(self.width, self.height, pixels)
    }

    /// Libera os objetos GL (textura/FBO/memory-object) — requer o contexto do Servo corrente. A
    /// imagem/memória Vulkan é liberada quando a `wgpu::Texture` é destruída (drop-callback). Chamar
    /// no resize antes de criar a nova textura, evitando vazamento.
    pub fn destroy(self) {
        // SAFETY: deletar objetos GL válidos no contexto corrente. Após isto, os ids não são mais
        // usados (self é consumido). A `wgpu_texture` é dropada ao fim, disparando a limpeza Vulkan.
        unsafe {
            gl::DeleteFramebuffers(1, &raw const self.gl_fbo);
            gl::DeleteTextures(1, &raw const self.gl_texture);
            if let Some(ext) = gl_mem_ext() {
                (ext.delete_memory_objects)(1, &raw const self.gl_memory_object);
            }
        }
    }

    /// Cria a textura compartilhada de `width`x`height`. `device`/`instance` são o device/instance
    /// wgpu do Slint (de onde extraímos os handles Vulkan crus). Requer o contexto GL do Servo
    /// corrente (p/ as chamadas GL de import) e as entry-points já carregadas ([`load_gl_with`]).
    #[expect(
        clippy::too_many_lines,
        reason = "pipeline linear de interop (Vulkan -> FD -> GL -> wgpu); dividir esconderia o fluxo"
    )]
    pub fn new(
        device: &wgpu::Device,
        instance: &wgpu::Instance,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let ext = gl_mem_ext()
            .ok_or("GL_EXT_memory_object_fd indisponível (entry-points não carregadas)")?;
        let handle_type = vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;

        // --- Handles Vulkan crus a partir do device/instance wgpu do Slint ---
        // SAFETY: `as_hal::<Vulkan>` é seguro de chamar; só retorna Some se o backend for Vulkan.
        // Clonamos `ash::Device`/`ash::Instance` (wrappers baratos, válidos enquanto o device wgpu
        // viver) e copiamos o `vk::PhysicalDevice` (handle Copy) p/ usar fora do guard.
        let hal_device_guard =
            unsafe { device.as_hal::<Vulkan>() }.ok_or("device wgpu não é Vulkan")?;
        let ash_device = hal_device_guard.raw_device().clone();
        let physical_device = hal_device_guard.raw_physical_device();
        let ash_instance = {
            let hal_instance =
                unsafe { instance.as_hal::<Vulkan>() }.ok_or("instance wgpu não é Vulkan")?;
            hal_instance.shared_instance().raw_instance().clone()
        };

        // --- 1. Imagem Vulkan com memória externa exportável ---
        let mut external_info =
            vk::ExternalMemoryImageCreateInfo::default().handle_types(handle_type);
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external_info);
        // SAFETY: `image_info` é válido e vive até o fim da chamada; criamos uma imagem nova.
        let image = unsafe { ash_device.create_image(&image_info, None) }
            .map_err(|e| format!("vkCreateImage: {e}"))?;

        // SAFETY: `image` acabou de ser criada neste device.
        let requirements = unsafe { ash_device.get_image_memory_requirements(image) };
        // SAFETY: `physical_device` é válido (veio do device wgpu).
        let mem_props =
            unsafe { ash_instance.get_physical_device_memory_properties(physical_device) };
        let mem_type_index = find_memory_type(
            &mem_props,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .ok_or("nenhum tipo de memória DEVICE_LOCAL compatível")?;

        let mut export_info = vk::ExportMemoryAllocateInfo::default().handle_types(handle_type);
        let mut dedicated_info = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(mem_type_index)
            .push_next(&mut export_info)
            .push_next(&mut dedicated_info);
        // SAFETY: `alloc_info` válido; aloca memória nova, dedicada à `image`, exportável.
        let memory = unsafe { ash_device.allocate_memory(&alloc_info, None) }.map_err(|e| {
            // SAFETY: limpa a imagem órfã antes de propagar o erro.
            unsafe { ash_device.destroy_image(image, None) };
            format!("vkAllocateMemory: {e}")
        })?;
        // SAFETY: liga a memória recém-alocada à imagem (offset 0).
        if let Err(e) = unsafe { ash_device.bind_image_memory(image, memory, 0) } {
            // SAFETY: desfaz a alocação e a imagem antes de propagar.
            unsafe {
                ash_device.free_memory(memory, None);
                ash_device.destroy_image(image, None);
            }
            return Err(format!("vkBindImageMemory: {e}"));
        }

        // --- 2. Exporta o FD e importa no GL do Servo ---
        let external_memory_fd =
            ash::khr::external_memory_fd::Device::new(&ash_instance, &ash_device);
        let fd_info = vk::MemoryGetFdInfoKHR::default()
            .memory(memory)
            .handle_type(handle_type);
        // SAFETY: `memory` é exportável (OPAQUE_FD). O FD é consumido por `glImportMemoryFdEXT`.
        let fd = unsafe { external_memory_fd.get_memory_fd(&fd_info) }.map_err(|e| {
            // SAFETY: desfaz memória + imagem se a exportação falhar.
            unsafe {
                ash_device.free_memory(memory, None);
                ash_device.destroy_image(image, None);
            }
            format!("vkGetMemoryFdKHR: {e}")
        })?;

        let w_i = i32::try_from(width).map_err(|_| "largura > i32::MAX")?;
        let h_i = i32::try_from(height).map_err(|_| "altura > i32::MAX")?;
        // SAFETY: chamadas GL no contexto corrente do Servo. `glImportMemoryFdEXT` toma posse do `fd`.
        // A textura usa tiling OPTIMAL p/ casar com a imagem Vulkan (layout idêntico na mesma memória).
        let (gl_texture, gl_memory_object) = unsafe {
            let mut mem_object: gl::types::GLuint = 0;
            (ext.create_memory_objects)(1, &raw mut mem_object);
            (ext.import_memory_fd)(
                mem_object,
                requirements.size,
                GL_HANDLE_TYPE_OPAQUE_FD_EXT,
                fd,
            );
            let mut texture: gl::types::GLuint = 0;
            gl::GenTextures(1, &raw mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexParameteri(
                gl::TEXTURE_2D,
                GL_TEXTURE_TILING_EXT,
                i32::try_from(GL_OPTIMAL_TILING_EXT).unwrap_or(0),
            );
            (ext.tex_storage_mem_2d)(gl::TEXTURE_2D, 1, gl::RGBA8, w_i, h_i, mem_object, 0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            (texture, mem_object)
        };

        // FBO com a textura compartilhada anexada (alvo do blit por frame).
        // SAFETY: cria/configura um FBO; checa completude antes de prosseguir.
        let gl_fbo = unsafe {
            let mut fbo: gl::types::GLuint = 0;
            gl::GenFramebuffers(1, &raw mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                gl_texture,
                0,
            );
            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            if status != gl::FRAMEBUFFER_COMPLETE {
                gl::DeleteFramebuffers(1, &raw const fbo);
                gl::DeleteTextures(1, &raw const gl_texture);
                (ext.delete_memory_objects)(1, &raw const gl_memory_object);
                // imagem/memória Vulkan ainda não pertencem ao wgpu — liberar aqui.
                ash_device.free_memory(memory, None);
                ash_device.destroy_image(image, None);
                return Err(format!("FBO compartilhado incompleto: 0x{status:x}"));
            }
            fbo
        };

        // --- 3. Embrulha a imagem Vulkan como wgpu::Texture (zero-copy) ---
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: Some("basedbrowser-shared-frame"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::wgt::TextureUses::COLOR_TARGET | wgpu::wgt::TextureUses::RESOURCE,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        // A drop-callback libera a imagem/memória Vulkan quando o wgpu termina de usar a textura.
        let cb_device = ash_device.clone();
        let drop_callback: wgpu::hal::DropCallback = Box::new(move || {
            // SAFETY: chamado pelo wgpu após parar de usar a textura; `image`/`memory` foram criados
            // por nós e não têm outros usuários (o GL apenas importou um FD duplicado da memória).
            unsafe {
                cb_device.destroy_image(image, None);
                cb_device.free_memory(memory, None);
            }
        });
        // SAFETY: `image` foi criada respeitando `hal_desc`; passamos `TextureMemory::External`
        // (nós donos da memória) + `drop_callback`, então o wgpu não assume posse da memória.
        let hal_texture = unsafe {
            hal_device_guard.texture_from_raw(
                image,
                &hal_desc,
                Some(drop_callback),
                wgpu::hal::vulkan::TextureMemory::External,
            )
        };
        let wgpu_desc = wgpu::TextureDescriptor {
            label: Some("basedbrowser-shared-frame"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        // SAFETY: `hal_texture` foi criada acima a partir de `image`, coerente com `wgpu_desc`.
        let wgpu_texture =
            unsafe { device.create_texture_from_hal::<Vulkan>(hal_texture, &wgpu_desc) };

        Ok(Self {
            wgpu_texture,
            gl_texture,
            gl_fbo,
            gl_memory_object,
            width,
            height,
        })
    }
}

/// Acha um índice de tipo de memória que satisfaça `type_bits` (da `MemoryRequirements`) e contenha
/// `flags` (ex.: `DEVICE_LOCAL`).
fn find_memory_type(
    props: &vk::PhysicalDeviceMemoryProperties,
    type_bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    (0..props.memory_type_count).find(|&i| {
        let supported = type_bits & (1 << i) != 0;
        let has_flags = props.memory_types[i as usize]
            .property_flags
            .contains(flags);
        supported && has_flags
    })
}
