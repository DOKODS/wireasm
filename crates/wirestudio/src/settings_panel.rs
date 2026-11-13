//! The parameters tab: everything about the interface that can be changed.
//!
//! The values are stored in the settings file, so they survive a restart.

use eframe::egui::{self, RichText};

use crate::app::WireStudio;
use crate::i18n::t;
use crate::settings::{SheetStyle, Theme};

impl WireStudio {
    /// The parameters tab.
    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(s("settings.title"));
            ui.label(RichText::new(s("settings.hint")).weak());
            ui.separator();

            // --- Interface -------------------------------------------------
            ui.heading(s("settings.interface"));
            let mut theme = self.theme();
            ui.horizontal(|ui| {
                ui.label(s("settings.theme"));
                egui::ComboBox::from_id_salt("settings_theme")
                    .selected_text(s(theme.label_key()))
                    .show_ui(ui, |ui| {
                        for option in [Theme::Dark, Theme::Light] {
                            if ui
                                .selectable_label(theme == option, s(option.label_key()))
                                .clicked()
                            {
                                theme = option;
                            }
                        }
                    });
            });
            if theme != self.theme() {
                self.set_theme(theme);
            }

            let mut scale = self.ui_scale();
            ui.horizontal(|ui| {
                ui.label(s("settings.ui_scale"));
                let mut value = scale;
                if ui
                    .add(
                        egui::Slider::new(&mut value, 0.75..=1.75).suffix("×"),
                    )
                    .changed()
                {
                    scale = value;
                }
            });
            if (scale - self.ui_scale()).abs() > 1e-6 {
                self.set_ui_scale(scale);
            }

            let mut code_size = self.code_font_size();
            ui.horizontal(|ui| {
                ui.label(s("settings.code_font"));
                let mut value = code_size;
                if ui
                    .add(egui::Slider::new(&mut value, 10.0..=22.0).suffix(" pt"))
                    .changed()
                {
                    code_size = value;
                }
            });
            if (code_size - self.code_font_size()).abs() > 1e-6 {
                self.set_code_font_size(code_size);
            }

            let mut numbers = self.editor.line_numbers;
            if ui
                .checkbox(&mut numbers, s("settings.line_numbers"))
                .changed()
            {
                self.editor.line_numbers = numbers;
                self.settings.save();
            }

            ui.separator();

            // --- Schematic ------------------------------------------------
            ui.heading(s("settings.schematic"));
            let mut style = self.sheet_style();
            ui.horizontal(|ui| {
                ui.label(s("settings.sheet_style"));
                egui::ComboBox::from_id_salt("settings_sheet_style")
                    .selected_text(s(style.label_key()))
                    .show_ui(ui, |ui| {
                        for option in [SheetStyle::Multisim, SheetStyle::Dark] {
                            if ui
                                .selectable_label(style == option, s(option.label_key()))
                                .clicked()
                            {
                                style = option;
                            }
                        }
                    });
            });
            if style != self.sheet_style() {
                self.set_sheet_style(style);
            }

            let mut grid = self.show_grid();
            if ui.checkbox(&mut grid, s("settings.show_grid")).changed() {
                self.set_show_grid(grid);
            }
            let mut numbers = self.show_node_numbers();
            if ui
                .checkbox(&mut numbers, s("settings.show_node_numbers"))
                .changed()
            {
                self.set_show_node_numbers(numbers);
            }
            let mut panels = self.auto_open_panels();
            if ui
                .checkbox(&mut panels, s("settings.auto_open_panels"))
                .changed()
            {
                self.set_auto_open_panels(panels);
            }

            ui.separator();

            // --- Simulation -----------------------------------------------
            ui.heading(s("settings.simulation"));
            let mut dt = self.sim_dt();
            ui.horizontal(|ui| {
                ui.label(s("settings.sim_dt"));
                let mut value = dt;
                if ui
                    .add(egui::Slider::new(&mut value, 1e-6..=1e-2).suffix(" s"))
                    .changed()
                {
                    dt = value;
                }
            });
            if (dt - self.sim_dt()).abs() > 1e-12 {
                self.set_sim_dt(dt);
            }
            let mut stop = self.sim_stop();
            ui.horizontal(|ui| {
                ui.label(s("settings.sim_stop"));
                let mut value = stop;
                if ui
                    .add(egui::Slider::new(&mut value, 1e-4..=1.0).suffix(" s"))
                    .changed()
                {
                    stop = value;
                }
            });
            if (stop - self.sim_stop()).abs() > 1e-12 {
                self.set_sim_stop(stop);
            }

            ui.separator();

            // --- Language and paths ---------------------------------------
            ui.heading(s("settings.misc"));
            let mut lang_code = self.settings.lang.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(s("settings.language"));
                egui::ComboBox::from_id_salt("settings_language")
                    .selected_text(match lang_code.as_str() {
                        "en" => "English",
                        "ru" => "Русский",
                        _ => s("lang.system"),
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(lang_code.is_empty(), s("lang.system"))
                            .clicked()
                        {
                            lang_code.clear();
                        }
                        if ui.selectable_label(lang_code == "en", "English").clicked() {
                            lang_code = "en".to_string();
                        }
                        if ui.selectable_label(lang_code == "ru", "Русский").clicked() {
                            lang_code = "ru".to_string();
                        }
                    });
            });
            if lang_code != self.settings.lang.clone().unwrap_or_default() {
                self.settings.lang = if lang_code.is_empty() {
                    None
                } else {
                    Some(lang_code.clone())
                };
                self.settings.save();
                self.lang = self
                    .settings
                    .lang
                    .as_deref()
                    .and_then(crate::i18n::Lang::from_code)
                    .unwrap_or_else(crate::i18n::Lang::detect);
            }

            ui.horizontal(|ui| {
                ui.label(s("settings.out_dir"));
                let mut dir = self.settings.out_dir.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut dir)
                            .desired_width(200.0)
                            .hint_text("out"),
                    )
                    .changed()
                {
                    self.settings.out_dir = if dir.trim().is_empty() {
                        None
                    } else {
                        Some(dir.trim().to_string())
                    };
                    self.settings.save();
                }
            });

            ui.separator();
            if ui.button(s("settings.reset")).clicked() {
                self.reset_settings();
            }
        });
    }
}
