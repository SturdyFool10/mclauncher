use config::{
    Config, ConfigFormat, LoadConfigResult, create_default_config, load_config, save_config,
};
use eframe::{self, egui};
use egui::CentralPanel;
use textui::TextUi;

use crate::{screens, ui, window_effects};

use self::auth_state::{AuthState, REPAINT_INTERVAL};
use self::config_format_modal::ModalAction;
use self::fonts::FontController;

mod auth_state;
mod config_format_modal;
mod fonts;
mod native_options;

struct VertexApp {
    fonts: FontController,
    config: Config,
    theme_catalog: ui::theme::ThemeCatalog,
    theme: ui::theme::Theme,
    show_config_format_modal: bool,
    selected_config_format: ConfigFormat,
    default_config_format: ConfigFormat,
    config_creation_error: Option<String>,
    active_screen: screens::AppScreen,
    profile_shortcuts: Vec<ui::sidebar::ProfileShortcut>,
    selected_profile_id: Option<String>,
    auth: AuthState,
    text_ui: TextUi,
}

impl VertexApp {
    fn new(cc: &eframe::CreationContext<'_>, config_state: LoadConfigResult) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let (mut config, show_config_format_modal, selected_config_format, default_config_format) =
            match config_state {
                LoadConfigResult::Loaded(config) => {
                    (config, false, ConfigFormat::Json, ConfigFormat::Json)
                }
                LoadConfigResult::Missing { default_format } => {
                    (Config::default(), true, default_format, default_format)
                }
            };

        config.normalize();
        window_effects::apply(cc, config.window_blur_enabled());

        let theme_catalog = ui::theme::ThemeCatalog::load();
        if !theme_catalog.contains(config.theme_id()) {
            config.set_theme_id(theme_catalog.default_theme_id().to_owned());
        }
        let theme = theme_catalog.resolve(config.theme_id()).clone();

        let mut text_ui = TextUi::new();
        FontController::register_included_fonts(&mut text_ui);

        let mut app = Self {
            fonts: FontController::new(config.ui_font_family()),
            config,
            theme_catalog,
            theme,
            show_config_format_modal,
            selected_config_format,
            default_config_format,
            config_creation_error: None,
            active_screen: screens::AppScreen::Library,
            profile_shortcuts: Vec::new(),
            selected_profile_id: None,
            auth: AuthState::load(),
            text_ui,
        };

        app.fonts.ensure_selected_font_is_available(&mut app.config);
        app.fonts
            .apply_from_config(&cc.egui_ctx, &app.config, &mut app.text_ui);
        app
    }

    fn create_config_with_choice(&mut self, choice: ConfigFormat) {
        match create_default_config(choice) {
            Ok(config) => {
                self.config = config;
                self.config.normalize();
                self.fonts
                    .ensure_selected_font_is_available(&mut self.config);
                self.show_config_format_modal = false;
                self.config_creation_error = None;
            }
            Err(err) => {
                self.config_creation_error = Some(format!("Failed to create config: {err}"));
            }
        }
    }

    fn sync_theme_from_config(&mut self) {
        if !self.theme_catalog.contains(self.config.theme_id()) {
            self.config
                .set_theme_id(self.theme_catalog.default_theme_id().to_owned());
        }

        let resolved = self.theme_catalog.resolve(self.config.theme_id());
        if self.theme.id != resolved.id {
            self.theme = resolved.clone();
        }
    }
}

impl eframe::App for VertexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.text_ui.begin_frame(ctx);
        self.auth.poll();
        if self.auth.should_request_repaint() {
            ctx.request_repaint_after(REPAINT_INTERVAL);
        }

        let previous_config = self.config.clone();
        self.sync_theme_from_config();
        self.theme.apply(ctx, self.config.window_blur_enabled());
        self.fonts
            .ensure_selected_font_is_available(&mut self.config);
        self.fonts
            .apply_from_config(ctx, &self.config, &mut self.text_ui);

        let device_prompt = self.auth.device_prompt();
        let top_bar_output = ui::top_bar::render(
            ctx,
            self.active_screen,
            &mut self.text_ui,
            ui::top_bar::ProfileUiModel {
                display_name: self.auth.display_name(),
                avatar_png: self.auth.avatar_png(),
                sign_in_in_progress: self.auth.sign_in_in_progress(),
                status_message: self.auth.status_message(),
                device_user_code: device_prompt.map(|prompt| prompt.user_code.as_str()),
                verification_uri: device_prompt.map(|prompt| prompt.verification_uri.as_str()),
                verification_uri_complete: device_prompt
                    .and_then(|prompt| prompt.verification_uri_complete.as_deref()),
            },
        );

        if top_bar_output.start_sign_in {
            self.auth.start_sign_in();
        }
        if top_bar_output.sign_out {
            self.auth.sign_out();
        }

        let sidebar_output = ui::sidebar::render(ctx, self.active_screen, &self.profile_shortcuts);

        if let Some(next_screen) = sidebar_output.selected_screen {
            self.active_screen = next_screen;
        }
        if let Some(profile_id) = sidebar_output.selected_profile_id {
            self.selected_profile_id = Some(profile_id);
        }

        CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::ZERO)
                    .outer_margin(egui::Margin::ZERO)
                    .stroke(egui::Stroke::new(
                        1.0,
                        ctx.style().visuals.widgets.noninteractive.bg_stroke.color,
                    )),
            )
            .show(ctx, |ui| {
                screens::render(
                    ui,
                    self.active_screen,
                    self.selected_profile_id.as_deref(),
                    &mut self.config,
                    self.fonts.available_ui_fonts(),
                    self.theme_catalog.themes(),
                    &mut self.text_ui,
                );
            });

        if self.show_config_format_modal {
            match config_format_modal::render(
                ctx,
                &mut self.text_ui,
                &mut self.selected_config_format,
                self.config_creation_error.as_deref(),
            ) {
                ModalAction::None => {}
                ModalAction::Cancel => self.create_config_with_choice(self.default_config_format),
                ModalAction::Create(choice) => self.create_config_with_choice(choice),
            }
        }

        self.config.normalize();
        self.fonts
            .ensure_selected_font_is_available(&mut self.config);
        if self.config != previous_config {
            if let Err(err) = save_config(&self.config) {
                eprintln!("Failed to save config: {err}");
            }
            self.fonts
                .apply_from_config(ctx, &self.config, &mut self.text_ui);
        }

        ui::top_bar::handle_window_resize(ctx);
    }
}

pub fn run() -> eframe::Result<()> {
    let config_state = load_config();
    let startup_config = match &config_state {
        LoadConfigResult::Loaded(config) => config.clone(),
        LoadConfigResult::Missing { .. } => Config::default(),
    };

    let options = native_options::build(&startup_config);

    eframe::run_native(
        "Vertex Launcher",
        options,
        Box::new(move |cc| Ok(Box::new(VertexApp::new(cc, config_state)))),
    )
}
