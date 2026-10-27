use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke, Vec2};

#[derive(Clone, Copy)]
pub struct Palette {
    pub surface: Color32,
    pub list: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub selected: Color32,
    pub hover: Color32,
}
impl Palette {
    pub fn current_from_context(ctx: &egui::Context) -> Self {
        Self::new(ctx.style().visuals.dark_mode)
    }
    pub fn current(ui: &egui::Ui) -> Self {
        Self::new(ui.visuals().dark_mode)
    }
    fn new(dark: bool) -> Self {
        let rgb = |hex: u32| Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        if dark {
            Self {
                surface: rgb(0x141918),
                list: rgb(0x1e2523),
                ink: rgb(0xeaf3ee),
                muted: rgb(0x9bada4),
                line: rgb(0x34423b),
                accent: rgb(0x91e5b9),
                selected: rgb(0x253e31),
                hover: Color32::from_white_alpha(21),
            }
        } else {
            Self {
                surface: rgb(0xeaf3ee),
                list: Color32::WHITE,
                ink: rgb(0x1a1a1a),
                muted: rgb(0x666666),
                line: rgb(0xdddddd),
                accent: rgb(0x23794f),
                selected: rgb(0xe0f2e7),
                hover: Color32::from_black_alpha(12),
            }
        }
    }
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Segoe Semibold".into()))
}
pub fn title(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(semibold(size))
}
pub fn install_fonts(ctx: &egui::Context) {
    let directory = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("Fonts");
    let mut fonts = FontDefinitions::default();
    for (name, filename, family) in [
        ("Segoe UI", "segoeui.ttf", FontFamily::Proportional),
        (
            "Segoe Semibold",
            "seguisb.ttf",
            FontFamily::Name("Segoe Semibold".into()),
        ),
        (
            "Segoe Icons",
            "segmdl2.ttf",
            FontFamily::Name("Segoe Icons".into()),
        ),
    ] {
        if let Ok(bytes) = std::fs::read(directory.join(filename)) {
            fonts
                .font_data
                .insert(name.into(), FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
    }
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    for name in ["Segoe Semibold", "Segoe Icons"] {
        fonts
            .families
            .entry(FontFamily::Name(name.into()))
            .or_default()
            .extend(fallback.clone());
    }
    ctx.set_fonts(fonts);
}
pub fn configure(ctx: &egui::Context) {
    let p = Palette::new(ctx.style().visuals.dark_mode);
    ctx.style_mut(|s| {
        s.text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
        s.text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        s.text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(10.5));
        s.text_styles
            .insert(egui::TextStyle::Heading, semibold(20.0));
        s.spacing.item_spacing = Vec2::new(8.0, 0.0);
        s.spacing.button_padding = Vec2::new(14.0, 9.0);
        s.spacing.interact_size.y = 22.0;
        s.spacing.icon_width = 18.0;
        s.spacing.icon_width_inner = 10.0;
        s.spacing.icon_spacing = 8.0;
        s.spacing.combo_width = 160.0;
        s.visuals.panel_fill = p.surface;
        s.visuals.window_fill = p.list;
        s.visuals.extreme_bg_color = p.list;
        s.visuals.override_text_color = Some(p.ink);

        s.visuals.selection.bg_fill = p.accent;
        s.visuals.selection.stroke = Stroke::new(1.0_f32, p.surface);
        s.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
        s.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.ink);
        for (widget, fill) in [
            (
                &mut s.visuals.widgets.inactive,
                if s.visuals.dark_mode {
                    p.list
                } else {
                    Color32::from_rgb(253, 253, 253)
                },
            ),
            (
                &mut s.visuals.widgets.hovered,
                if s.visuals.dark_mode {
                    p.selected
                } else {
                    Color32::from_rgb(246, 246, 246)
                },
            ),
            (
                &mut s.visuals.widgets.active,
                if s.visuals.dark_mode {
                    p.surface
                } else {
                    Color32::from_rgb(240, 240, 240)
                },
            ),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = Stroke::new(1.0_f32, p.line);
            widget.fg_stroke = Stroke::new(1.0_f32, p.ink);
            widget.corner_radius = egui::CornerRadius::same(9);
            widget.expansion = 0.0;
        }
    });
}
pub fn button(ui: &mut egui::Ui, label: &str, glyph: Option<char>) -> egui::Response {
    let p = Palette::current(ui);
    let mut text = egui::text::LayoutJob::default();
    if let Some(glyph) = glyph {
        text.append(
            &glyph.to_string(),
            0.0,
            egui::TextFormat {
                font_id: FontId::new(13.0, FontFamily::Name("Segoe Icons".into())),
                color: p.ink,
                ..Default::default()
            },
        );
    }
    if !label.is_empty() {
        text.append(
            label,
            if glyph.is_some() { 7.0 } else { 0.0 },
            egui::TextFormat {
                font_id: FontId::proportional(13.0),
                color: p.ink,
                ..Default::default()
            },
        );
    }
    ui.add(egui::Button::new(text).min_size(egui::vec2(36.0, 36.0)))
}
pub fn rule(ui: &mut egui::Ui) {
    let p = Palette::current(ui);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0, p.line);
}
pub fn muted(ui: &mut egui::Ui, text: impl Into<String>, size: f32) -> egui::Response {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .size(size)
                .color(Palette::current(ui).muted),
        )
        .wrap(),
    )
}
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(22.0);
    rule(ui);
    ui.add_space(18.0);
    ui.label(title(text, 14.0).color(Palette::current(ui).accent));
    ui.add_space(6.0);
}
pub fn toggle(ui: &mut egui::Ui, value: &mut bool, text: &str) {
    ui.add_space(9.0);
    ui.checkbox(value, text);
    ui.add_space(9.0);
}
pub fn ellipsis(
    ui: &mut egui::Ui,
    text: &str,
    width: f32,
    size: f32,
    bold: bool,
    muted: bool,
) -> egui::Response {
    let font = if bold {
        semibold(size)
    } else {
        FontId::proportional(size)
    };
    let color = if muted {
        Palette::current(ui).muted
    } else {
        Palette::current(ui).ink
    };
    ui.allocate_ui_with_layout(
        egui::vec2(width.max(0.0), size + 2.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width.max(0.0));
            ui.set_max_width(width.max(0.0));
            ui.add(egui::Label::new(RichText::new(text).font(font).color(color)).truncate())
                .on_hover_text(text)
        },
    )
    .inner
}

pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let p = Palette::current(ui);
    ui.add(
        egui::Button::new(title(label, 13.0).color(p.surface))
            .fill(p.accent)
            .stroke(Stroke::NONE)
            .min_size(egui::vec2(36.0, 36.0)),
    )
}
