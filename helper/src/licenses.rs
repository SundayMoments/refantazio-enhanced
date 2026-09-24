use eframe::egui::{self, RichText};

// Render the headings, paragraphs, and component table used by LICENSE.md.
// The repository document remains the source of the wording and attribution.
pub fn project(ui: &mut egui::Ui, selected: &mut Option<(&'static str, &'static str)>) {
    for (index, block) in include_str!("../../LICENSE.md")
        .split("\n\n")
        .flat_map(|block| block.split("\r\n\r\n"))
        .enumerate()
    {
        let block = block.trim();
        if block.starts_with('|') {
            let width = (ui.available_width() - 24.) / 2.;
            egui::Grid::new(("component_licenses", index))
                .num_columns(2)
                .min_col_width(width)
                .max_col_width(width)
                .spacing([24., 14.])
                .striped(true)
                .show(ui, |ui| {
                    for (row, line) in block.lines().enumerate() {
                        if row == 1 {
                            continue; // Markdown table separator.
                        }
                        for cell in line.trim_matches('|').split('|').map(str::trim) {
                            if row == 0 {
                                ui.label(RichText::new(cell).strong());
                            } else if let Some((label, path)) = cell
                                .strip_prefix('[')
                                .and_then(|s| s.strip_suffix(')'))
                                .and_then(|s| s.split_once("]("))
                            {
                                if let Some(text) = embedded(path) {
                                    if ui
                                        .link(label)
                                        .on_hover_text("Read bundled license")
                                        .clicked()
                                    {
                                        *selected = Some((label, text));
                                    }
                                } else {
                                    ui.label(cell);
                                }
                            } else {
                                ui.add(egui::Label::new(cell).wrap());
                            }
                        }
                        ui.end_row();
                    }
                });
        } else if let Some(heading) = block
            .strip_prefix("## ")
            .or_else(|| block.strip_prefix("# "))
        {
            ui.label(RichText::new(heading).strong().size(19.));
        } else if !block.is_empty() {
            let mut text = egui::text::LayoutJob::default();
            for (span, content) in block.split('`').enumerate() {
                text.append(
                    &content.replace("\r\n", " ").replace('\n', " "),
                    0.,
                    egui::TextFormat {
                        font_id: if span % 2 == 1 {
                            egui::TextStyle::Monospace.resolve(ui.style())
                        } else {
                            egui::TextStyle::Body.resolve(ui.style())
                        },
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );
            }
            ui.add(egui::Label::new(text).wrap());
        }
    }
}

pub fn embedded(path: &str) -> Option<&'static str> {
    // Only embedded documents can be opened: no filesystem or browser navigation.
    Some(match path {
        "licenses/Luma-Custom-MIT.txt" => include_str!("../../licenses/Luma-Custom-MIT.txt"),
        "licenses/MetaphorFix-MIT.txt" => include_str!("../../licenses/MetaphorFix-MIT.txt"),
        "licenses/ReShade-BSD.txt" => include_str!("../../licenses/ReShade-BSD.txt"),
        "licenses/SafetyHook-Boost.txt" => include_str!("../../licenses/SafetyHook-Boost.txt"),
        "licenses/Zydis-MIT.txt" => include_str!("../../licenses/Zydis-MIT.txt"),
        "licenses/NVIDIA-DLSS.txt" => include_str!("../../licenses/NVIDIA-DLSS.txt"),
        "licenses/Microsoft-VC-Runtime.txt" => {
            include_str!("../../licenses/Microsoft-VC-Runtime.txt")
        }
        _ => return None,
    })
}

pub fn dialog(ctx: &egui::Context, selected: &mut Option<(&'static str, &'static str)>) {
    let Some((title, text)) = *selected else {
        return;
    };
    let available = ctx.content_rect().size();
    let response = egui::Modal::new(egui::Id::new("license_reader"))
        .frame(egui::Frame::popup(&ctx.style_of(egui::Theme::Dark)).inner_margin(24))
        .show(ctx, |ui| {
            ui.set_width((available.x - 96.).min(720.));
            ui.label(RichText::new(title).strong().size(19.));
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt(title)
                .max_height((available.y - 200.).max(100.))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(text).wrap());
                });
            ui.separator();
            if ui.button("Close").clicked() {
                ui.close();
            }
        });
    if response.should_close() {
        *selected = None;
    }
}
