use std::path::PathBuf;
use std::process::Command;
use eframe::egui::{self, Button, ProgressBar, RichText, ScrollArea, Vec2};

use crate::builder::{BuildMsg, BuildRunner};
use crate::config::Config;
use crate::theme::*;

#[derive(PartialEq)]
enum ActiveTab {
    Play,
    Settings,
    Logs,
}

enum BuildState {
    Idle,
    Building {
        runner: BuildRunner,
        progress: f32,
        status: String,
    },
    Success(#[allow(dead_code)] PathBuf),
    Failed(String),
}

pub struct DevilCriesApp {
    config: Config,
    tab: ActiveTab,
    build_state: BuildState,
    log_messages: Vec<String>,
    game_running: bool,
}

impl DevilCriesApp {
    pub fn new() -> Self {
        let config = Config::load();
        Self {
            config,
            tab: ActiveTab::Play,
            build_state: BuildState::Idle,
            log_messages: Vec::new(),
            game_running: false,
        }
    }

    fn poll_builder(&mut self) {
        if let BuildState::Building { runner, progress, status } = &mut self.build_state {
            let mut done = false;
            let mut failed = None;
            let mut success = None;

            while let Ok(msg) = runner.rx.try_recv() {
                match msg {
                    BuildMsg::Progress(msg, p) => {
                        *status = msg;
                        *progress = p;
                    }
                    BuildMsg::Log(line) => {
                        self.log_messages.push(line);
                        if self.log_messages.len() > 1000 {
                            self.log_messages.remove(0);
                        }
                    }
                    BuildMsg::Success(exe) => {
                        success = Some(exe);
                        done = true;
                    }
                    BuildMsg::Failed(err) => {
                        failed = Some(err);
                        done = true;
                    }
                }
            }

            if done {
                if let Some(exe) = success {
                    self.build_state = BuildState::Success(exe);
                    let _ = self.config.save();
                } else if let Some(err) = failed {
                    self.build_state = BuildState::Failed(err);
                }
            }
        }
    }

    fn launch_game(&mut self) {
        let exe = self.config.executable_path();
        if !exe.is_file() {
            return;
        }

        let mut cmd = Command::new(&exe);

        let saves = &self.config.saves_dir;
        let _ = std::fs::create_dir_all(saves);
        cmd.env("KIRIKAE_SAVES", saves);

        cmd.env("KIRIKAE_MUTE", "0");
        cmd.env("KIRIKAE_H264_BACKEND", &self.config.h264_backend);
        cmd.env("KIRIKAE_FFMPEG_THREAD_TYPE", &self.config.thread_type);
        cmd.env("KIRIKAE_FFMPEG_THREADS", self.config.thread_count.to_string());
        cmd.env("KIRIKAE_FFMPEG_QUEUE", "8");

        if self.config.enable_stats {
            cmd.env("KIRIKAE_STATS", "1");
        }

        match cmd.spawn() {
            Ok(_) => {
                self.game_running = true;
            }
            Err(e) => {
                self.log_messages.push(format!("Falha ao iniciar o jogo: {e}"));
            }
        }
    }
}

impl eframe::App for DevilCriesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_builder();

        egui::CentralPanel::default().show(ctx, |ui| {
            // Header
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.heading(
                    RichText::new("DEVIL CRIES RECOMP")
                        .size(28.0)
                        .color(COLOR_CRIMSON)
                        .strong(),
                );
                ui.label(
                    RichText::new("Devil May Cry 3 Special Edition • Recompilação Nativa para PC")
                        .size(13.0)
                        .color(COLOR_TEXT_MUTED),
                );
            });

            ui.add_space(12.0);

            // Tab bar
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.tab == ActiveTab::Play, RichText::new(" ⚔ Jogar ").size(15.0).strong())
                    .clicked()
                {
                    self.tab = ActiveTab::Play;
                }
                if ui
                    .selectable_label(self.tab == ActiveTab::Settings, RichText::new(" ⚙ Configurações ").size(15.0).strong())
                    .clicked()
                {
                    self.tab = ActiveTab::Settings;
                }
                if ui
                    .selectable_label(self.tab == ActiveTab::Logs, RichText::new(" 📋 Logs ").size(15.0).strong())
                    .clicked()
                {
                    self.tab = ActiveTab::Logs;
                }
            });

            ui.separator();
            ui.add_space(8.0);

            // Content
            match self.tab {
                ActiveTab::Play => self.show_play_tab(ui),
                ActiveTab::Settings => self.show_settings_tab(ui),
                ActiveTab::Logs => self.show_logs_tab(ui),
            }
        });
    }
}

impl DevilCriesApp {
    fn show_play_tab(&mut self, ui: &mut egui::Ui) {
        let is_installed = self.config.is_installed();

        if is_installed {
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("✓ JOGO INSTALADO E PRONTO")
                            .size(15.0)
                            .color(COLOR_GOLD)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Executável: {}", self.config.executable_path().display()))
                            .size(12.0)
                            .color(COLOR_TEXT_MUTED),
                    );
                    ui.add_space(12.0);

                    let launch_btn = Button::new(
                        RichText::new("⚔  INICIAR DEVIL MAY CRY 3  ⚔")
                            .size(20.0)
                            .color(COLOR_TEXT_PRIMARY)
                            .strong(),
                    )
                    .fill(COLOR_CRIMSON)
                    .min_size(Vec2::new(340.0, 56.0));

                    if ui.add(launch_btn).clicked() {
                        self.launch_game();
                    }

                    ui.add_space(12.0);
                    ui.label(RichText::new("Controles: Suporte automático a Gamepad (Xbox/PlayStation) • F11 alterna Tela Cheia.").size(12.0).color(COLOR_TEXT_MUTED));
                    ui.add_space(8.0);
                });
            });

            ui.add_space(14.0);

            ui.horizontal(|ui| {
                if ui.button("📁 Abrir Pasta de Saves").clicked() {
                    let _ = std::fs::create_dir_all(&self.config.saves_dir);
                    let _ = Command::new("xdg-open").arg(&self.config.saves_dir).spawn();
                }

                if ui.button("📂 Alterar Pasta do Jogo").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        if folder.join("base").is_file() {
                            self.config.output_dir = folder;
                            let _ = self.config.save();
                        }
                    }
                }

                if ui.button("🔄 Recompilar Jogo").clicked() {
                    if let Some(src) = &self.config.game_source {
                        let runner = BuildRunner::start(src.clone(), self.config.output_dir.clone(), 2);
                        self.build_state = BuildState::Building {
                            runner,
                            progress: 0.0,
                            status: "Iniciando recompilação...".into(),
                        };
                    }
                }
            });
        } else {
            ui.group(|ui| {
                ui.heading(RichText::new("Setup Inicial do Jogo").size(18.0).color(COLOR_TEXT_PRIMARY));
                ui.label(
                    RichText::new("Selecione o arquivo NSP ou a pasta extraída do seu Devil May Cry 3 Special Edition para compilar o executável nativo.")
                        .size(13.0)
                        .color(COLOR_TEXT_MUTED),
                );

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    let path_str = self
                        .config
                        .game_source
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Nenhum arquivo selecionado".into());

                    ui.label(RichText::new("Arquivo do Jogo:").strong());
                    ui.monospace(&path_str);

                    if ui.button("Procurar...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Switch Game (*.nsp, *.nro, *.nso)", &["nsp", "nro", "nso"])
                            .pick_file()
                        {
                            self.config.game_source = Some(path);
                            let _ = self.config.save();
                        }
                    }
                });

                ui.add_space(12.0);

                match &self.build_state {
                    BuildState::Idle => {
                        let can_build = self.config.game_source.is_some();
                        let btn = Button::new(
                            RichText::new("COMPILAR JOGO PARA PC NATIVO")
                                .size(16.0)
                                .color(if can_build { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED })
                                .strong(),
                        )
                        .fill(if can_build { COLOR_CRIMSON } else { COLOR_SURFACE })
                        .min_size(Vec2::new(260.0, 42.0));

                        if ui.add_enabled(can_build, btn).clicked() {
                            if let Some(src) = &self.config.game_source {
                                let runner = BuildRunner::start(src.clone(), self.config.output_dir.clone(), 2);
                                self.build_state = BuildState::Building {
                                    runner,
                                    progress: 0.0,
                                    status: "Iniciando extração e recompilação...".into(),
                                };
                            }
                        }
                    }
                    BuildState::Building { progress, status, .. } => {
                        ui.add_space(8.0);
                        ui.label(RichText::new(status).strong().color(COLOR_GOLD));
                        ui.add(ProgressBar::new(*progress).show_percentage());
                    }
                    BuildState::Success(_) => {
                        ui.label(RichText::new("✓ Compilação concluída com sucesso!").color(COLOR_GOLD).strong());
                    }
                    BuildState::Failed(err) => {
                        ui.label(RichText::new(format!("✕ Falha na compilação: {err}")).color(COLOR_CRIMSON).strong());
                    }
                }
            });
        }
    }

    fn show_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("Configurações Gráficas e de Execução");
            ui.add_space(8.0);

            ui.checkbox(&mut self.config.fullscreen, "Iniciar em Tela Cheia (F11 para alternar)");
            ui.checkbox(&mut self.config.enable_stats, "Exibir Estatísticas e Contador de FPS");

            ui.add_space(8.0);
            ui.label(RichText::new("Threading de Vídeo (H.264):").strong());
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.config.thread_type, "slice".into(), "Slice Threading (Recomendado)");
                ui.radio_value(&mut self.config.thread_type, "frame".into(), "Frame Threading");
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Threads do Decodificador:");
                ui.add(egui::Slider::new(&mut self.config.thread_count, 1..=8));
            });

            ui.add_space(12.0);
            if ui.button("Salvar Configurações").clicked() {
                let _ = self.config.save();
            }
        });
    }

    fn show_logs_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Limpar Logs").clicked() {
                self.log_messages.clear();
            }
        });
        ui.separator();

        ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log_messages {
                    ui.monospace(line);
                }
            });
    }
}
