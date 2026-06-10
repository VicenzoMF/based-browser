//! BasedBrowser — janela do produto (Slint). **Marco M1**: o Slint hospeda o motor Servo
//! exibindo, via cópia-CPU, uma página renderizada (URL fixa). Ver `.specs/project/ROADMAP.md`
//! (M1), `docs/adr/0003-*` (arquitetura da integração) e `crates/servo-poc` (prova de conceito do M0).
//!
//! T1 (este passo): só o Slint, isolado — uma janela exibindo um `slint::Image` placeholder,
//! para provar a UI/loop antes de puxar o motor (Servo entra no T2/T3).

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

slint::slint! {
    export component MainWindow inherits Window {
        in property <image> frame;
        title: "BasedBrowser";
        // Tamanho inicial da viewport web (casará com o tamanho do buffer do Servo no T3).
        preferred-width: 1024px;
        preferred-height: 768px;
        Image {
            source: root.frame;
            width: 100%;
            height: 100%;
            // O conteúdo já vem no tamanho do frame; não esticar/borrar.
            image-fit: contain;
        }
    }
}

/// Placeholder do T1: um buffer RGBA8 de cor sólida, no mesmo caminho de dados que o Servo usará
/// no T3 (`SharedPixelBuffer<Rgba8Pixel>` → `Image::from_rgba8`).
fn placeholder_frame(width: u32, height: u32) -> Image {
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let fill = Rgba8Pixel {
        r: 28,
        g: 30,
        b: 38,
        a: 255,
    };
    for pixel in buffer.make_mut_slice() {
        *pixel = fill;
    }
    Image::from_rgba8(buffer)
}

fn main() -> Result<(), slint::PlatformError> {
    let app = MainWindow::new()?;
    app.set_frame(placeholder_frame(1024, 768));
    app.run()
}
