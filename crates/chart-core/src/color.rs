/// Plain 8-bit per-channel premultiplied-alpha RGBA color.
///
/// Chart-core stays renderer-agnostic, so colors live here as raw bytes.
/// Renderers (egui, wgpu shaders, …) convert at their boundary — see
/// `apps/desktop/.../indicators/color_bridge.rs` for the egui side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    pub const fn from_rgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    pub const fn to_array(self) -> [u8; 4] {
        self.0
    }
}
