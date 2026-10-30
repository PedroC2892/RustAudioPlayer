//! Visual identity from DESIGN.md: graphite canvas, charcoal placard, one
//! cinnabar red accent, serif body with a bold geometric sans for headings,
//! pill buttons, flat sharp surfaces. Also draws the app icon.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText,
    Shadow, Stroke, TextStyle, vec2,
};

/// Canvas: gray-black, used by every panel.
pub const GRAPHITE: Color32 = Color32::from_rgb(0x16, 0x16, 0x18);
/// Raised surface: the track table placard, inputs, menus.
pub const CHARCOAL: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x23);
/// Hairlines, dividers and hover fills.
pub const SMOKE: Color32 = Color32::from_rgb(0x34, 0x34, 0x3a);
/// Text: warm light gray, never pure white.
pub const ASH: Color32 = Color32::from_rgb(0xe8, 0xe6, 0xe3);
/// The single accent: playing state, progress, active toggles, main play button.
pub const RED: Color32 = Color32::from_rgb(0xd9, 0x4a, 0x3d);
/// Ash at 30% over graphite, for the record grooves in the icon.
const GROOVE: Color32 = Color32::from_rgb(0x55, 0x54, 0x55);

pub const PILL: CornerRadius = CornerRadius::same(255);

/// Ash at reduced opacity, for secondary text on graphite.
pub fn muted(alpha: f32) -> Color32 {
    ASH.gamma_multiply(alpha)
}

fn display_family() -> FontFamily {
    FontFamily::Name("display".into())
}

/// Heading text: bold sans with -0.02em tracking.
pub fn display(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text)
        .family(display_family())
        .size(size)
        .extra_letter_spacing(-0.02 * size)
        .color(ASH)
}

pub fn display_font(size: f32) -> FontId {
    FontId::new(size, display_family())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "serif".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Fraunces-Soft-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "display".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/InterTight-Bold.ttf"
        ))),
    );
    // egui's own fonts stay behind ours as fallback for symbols and emoji.
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "serif".into());
    let fallback = proportional[1..].to_vec();
    fonts.families.insert(
        display_family(),
        std::iter::once("display".to_owned())
            .chain(fallback)
            .collect(),
    );
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Heading, display_font(20.0)),
            (TextStyle::Monospace, FontId::monospace(12.0)),
        ]
        .into();
        style.interaction.selectable_labels = false;
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(14.0, 5.0);
        style.spacing.menu_margin = Margin::same(8);
        style.spacing.interact_size.y = 26.0;

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = GRAPHITE;
        v.window_fill = GRAPHITE;
        v.faint_bg_color = GRAPHITE;
        v.extreme_bg_color = CHARCOAL;
        v.text_edit_bg_color = Some(CHARCOAL);
        v.window_stroke = Stroke::new(1.0, SMOKE);
        v.window_corner_radius = CornerRadius::ZERO;
        v.menu_corner_radius = CornerRadius::ZERO;
        v.window_shadow = Shadow::NONE;
        v.popup_shadow = Shadow::NONE;
        v.hyperlink_color = ASH;
        v.error_fg_color = RED;
        v.warn_fg_color = RED;
        v.selection.bg_fill = RED.gamma_multiply(0.35);
        v.selection.stroke = Stroke::new(1.0, ASH);

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = GRAPHITE;
        w.noninteractive.weak_bg_fill = GRAPHITE;
        w.noninteractive.bg_stroke = Stroke::new(1.0, SMOKE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, ASH);
        w.noninteractive.corner_radius = CornerRadius::ZERO;
        // Ghost pills: transparent fill, hairline outline, never a solid button.
        for (state, fill, stroke, text) in [
            (&mut w.inactive, Color32::TRANSPARENT, SMOKE, ASH),
            (&mut w.hovered, SMOKE.gamma_multiply(0.5), ASH, ASH),
            (&mut w.active, SMOKE, ASH, ASH),
            (&mut w.open, SMOKE.gamma_multiply(0.5), ASH, ASH),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, stroke);
            state.fg_stroke = Stroke::new(1.0, text);
            state.corner_radius = PILL;
            state.expansion = 0.0;
        }
    });
}

/// RGBA pixels of the app icon at `size` x `size`: a graphite record with
/// ash grooves and a red label. 4x4 supersampling for smooth edges.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let to_unit = |p: u32, s: u32| {
                        (p as f32 + (s as f32 + 0.5) / SS as f32) / size as f32 * 2.0 - 1.0
                    };
                    let r = to_unit(x, sx).hypot(to_unit(y, sy));
                    if let Some(c) = icon_color(r) {
                        acc[0] += f32::from(c.r());
                        acc[1] += f32::from(c.g());
                        acc[2] += f32::from(c.b());
                        acc[3] += 1.0;
                    }
                }
            }
            let rgb = |i: usize| if acc[3] > 0.0 { acc[i] / acc[3] } else { 0.0 };
            let alpha = acc[3] / (SS * SS) as f32 * 255.0;
            out.extend([rgb(0), rgb(1), rgb(2), alpha].map(|c| c.round() as u8));
        }
    }
    out
}

/// Color at distance `r` from the center (1.0 = edge), `None` = transparent.
fn icon_color(r: f32) -> Option<Color32> {
    let ring = |center: f32, width: f32| (r - center).abs() < width / 2.0;
    Some(match r {
        r if r > 0.97 => return None,
        r if r < 0.06 => GRAPHITE,
        r if r < 0.34 => RED,
        _ if ring(0.89, 0.03) => muted(0.6),
        _ if ring(0.76, 0.016) || ring(0.64, 0.016) || ring(0.52, 0.016) => GROOVE,
        _ => GRAPHITE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_is_round_with_a_label() {
        let size = 64;
        let px = icon_rgba(size);
        assert_eq!(px.len(), (size * size * 4) as usize);
        let at = |x: u32, y: u32| &px[((y * size + x) * 4) as usize..][..4];
        assert_eq!(at(0, 0)[3], 0, "corner is transparent");
        assert_eq!(at(32, 32), &[GRAPHITE.r(), GRAPHITE.g(), GRAPHITE.b(), 255]);
        assert_eq!(at(32, 24), &[RED.r(), RED.g(), RED.b(), 255]);
    }
}
