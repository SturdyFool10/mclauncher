use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use config::Config;
use egui::{Color32, Layout, Ui};
use flate2::read::GzDecoder;
use instances::{InstanceStore, instance_root_path, set_world_favorite};
use textui::{LabelOptions, TextUi};

use crate::assets;

use super::{AppScreen, PendingLaunchIntent, queue_launch_intent};

const HOME_SCAN_INTERVAL: Duration = Duration::from_secs(3);
const SERVER_PING_REFRESH_INTERVAL: Duration = Duration::from_secs(20);
const SERVER_PING_CONNECT_TIMEOUT: Duration = Duration::from_millis(350);
const SERVER_PINGS_PER_SCAN: usize = 3;
const INSTANCE_ROW_HEIGHT: f32 = 34.0;
const ACTIVITY_ROW_HEIGHT: f32 = 54.0;
const ENTRY_ICON_SIZE: f32 = 14.0;

#[derive(Debug, Clone, Default)]
pub struct HomeOutput {
    pub requested_screen: Option<AppScreen>,
    pub selected_instance_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct HomeState {
    worlds: Vec<WorldEntry>,
    servers: Vec<ServerEntry>,
    server_pings: HashMap<String, ServerPingSnapshot>,
    last_scan_at: Option<Instant>,
    scanned_instance_count: usize,
}

#[derive(Debug, Clone)]
struct WorldEntry {
    instance_id: String,
    instance_name: String,
    world_id: String,
    world_name: String,
    game_mode: Option<String>,
    hardcore: Option<bool>,
    cheats_enabled: Option<bool>,
    difficulty: Option<String>,
    version_name: Option<String>,
    last_used_at_ms: Option<u64>,
    favorite: bool,
}

#[derive(Debug, Clone)]
struct ServerEntry {
    instance_id: String,
    instance_name: String,
    server_name: String,
    address: String,
    host: String,
    port: u16,
    last_used_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
enum ServerPingStatus {
    Unknown,
    Offline,
    Online { latency_ms: u64 },
}

#[derive(Debug, Clone)]
struct ServerPingSnapshot {
    status: ServerPingStatus,
    checked_at: Instant,
}

#[derive(Debug, Clone)]
struct ServerDatEntry {
    name: String,
    ip: String,
}

#[derive(Debug, Clone, Default)]
struct WorldMetadata {
    level_name: Option<String>,
    game_mode: Option<String>,
    hardcore: Option<bool>,
    cheats_enabled: Option<bool>,
    difficulty: Option<String>,
    version_name: Option<String>,
    last_played_ms: Option<u64>,
}

enum HomeEntryRef<'a> {
    World(&'a WorldEntry),
    Server(&'a ServerEntry),
}

impl HomeEntryRef<'_> {
    fn last_used_at_ms(&self) -> Option<u64> {
        match self {
            Self::World(world) => world.last_used_at_ms,
            Self::Server(server) => server.last_used_at_ms,
        }
    }

    fn primary_label(&self) -> &str {
        match self {
            Self::World(world) => world.world_name.as_str(),
            Self::Server(server) => server.server_name.as_str(),
        }
    }
}

pub fn render(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    config: &Config,
    streamer_mode: bool,
) -> HomeOutput {
    let mut output = HomeOutput::default();
    let state_id = ui.make_persistent_id("home_screen_state");
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<HomeState>(state_id))
        .unwrap_or_default();

    let should_scan = state
        .last_scan_at
        .is_none_or(|last| last.elapsed() >= HOME_SCAN_INTERVAL)
        || state.scanned_instance_count != instances.instances.len();
    if should_scan {
        refresh_home_state(&mut state, instances, config);
    }
    ui.ctx().request_repaint_after(Duration::from_millis(250));

    let mut heading_style = LabelOptions::default();
    heading_style.font_size = 28.0;
    heading_style.line_height = 34.0;
    heading_style.weight = 700;
    heading_style.color = ui.visuals().text_color();
    let _ = text_ui.label(ui, "home_heading", "Home", &heading_style);
    ui.add_space(10.0);

    let mut requested_rescan = false;
    render_instance_usage(ui, text_ui, instances, &mut output);
    ui.add_space(12.0);
    render_activity_feed(
        ui,
        text_ui,
        instances,
        &state,
        streamer_mode,
        &mut output,
        &mut requested_rescan,
    );

    if requested_rescan {
        refresh_home_state(&mut state, instances, config);
    }

    ui.ctx().data_mut(|data| data.insert_temp(state_id, state));
    output
}

fn render_instance_usage(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &InstanceStore,
    output: &mut HomeOutput,
) {
    let mut title_style = LabelOptions::default();
    title_style.font_size = 18.0;
    title_style.line_height = 24.0;
    title_style.weight = 700;
    title_style.color = ui.visuals().text_color();
    let _ = text_ui.label(ui, "home_usage_title", "Most Used Instances", &title_style);
    ui.add_space(6.0);

    let mut items = instances.instances.clone();
    items.sort_by(|a, b| {
        b.launch_count
            .cmp(&a.launch_count)
            .then_with(|| b.last_launched_at_ms.cmp(&a.last_launched_at_ms))
            .then_with(|| a.name.cmp(&b.name))
    });
    if items.is_empty() {
        let _ = text_ui.label(
            ui,
            "home_usage_empty",
            "No instances yet.",
            &LabelOptions {
                color: ui.visuals().weak_text_color(),
                wrap: true,
                ..LabelOptions::default()
            },
        );
        return;
    }

    let max_height = (ui.available_height() * (1.0 / 3.0)).clamp(140.0, 340.0);
    let now_ms = current_time_millis();
    egui::ScrollArea::vertical()
        .id_salt("home_instances_scroll")
        .max_height(max_height)
        .show(ui, |ui| {
            for (index, instance) in items.iter().enumerate() {
                let row_response = render_clickable_entry_row(
                    ui,
                    ("home_instance_row", index),
                    INSTANCE_ROW_HEIGHT,
                    |ui| {
                        render_entry_thumbnail(
                            ui,
                            ("home_instance_thumb", index),
                            assets::LIBRARY_SVG,
                            40.0,
                            18.0,
                        );
                        ui.add_space(8.0);
                        let _ = text_ui.label(
                            ui,
                            ("home_usage_name", index),
                            instance.name.as_str(),
                            &LabelOptions {
                                weight: 600,
                                color: ui.visuals().text_color(),
                                wrap: false,
                                ..LabelOptions::default()
                            },
                        );
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            let usage_line = format!(
                                "{} launches | {}",
                                instance.launch_count,
                                format_time_ago(instance.last_launched_at_ms, now_ms)
                            );
                            let _ = text_ui.label(
                                ui,
                                ("home_usage_count", index),
                                usage_line.as_str(),
                                &LabelOptions {
                                    color: ui.visuals().weak_text_color(),
                                    wrap: false,
                                    ..LabelOptions::default()
                                },
                            );
                        });
                    },
                );
                if row_response.clicked() {
                    queue_launch_intent(
                        ui.ctx(),
                        PendingLaunchIntent {
                            nonce: current_time_millis(),
                            instance_id: instance.id.clone(),
                            quick_play_singleplayer: None,
                            quick_play_multiplayer: None,
                        },
                    );
                    output.selected_instance_id = Some(instance.id.clone());
                    output.requested_screen = Some(AppScreen::Library);
                }
                ui.add_space(3.0);
            }
        });
}

fn render_activity_feed(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    instances: &mut InstanceStore,
    state: &HomeState,
    _streamer_mode: bool,
    output: &mut HomeOutput,
    requested_rescan: &mut bool,
) {
    let mut title_style = LabelOptions::default();
    title_style.font_size = 18.0;
    title_style.line_height = 24.0;
    title_style.weight = 700;
    title_style.color = ui.visuals().text_color();
    let _ = text_ui.label(ui, "home_activity_title", "Worlds & Servers", &title_style);
    ui.add_space(6.0);

    if state.worlds.is_empty() && state.servers.is_empty() {
        let _ = text_ui.label(
            ui,
            "home_activity_empty",
            "No worlds or servers found in any instance.",
            &LabelOptions {
                color: ui.visuals().weak_text_color(),
                wrap: true,
                ..LabelOptions::default()
            },
        );
        return;
    }

    let now_ms = current_time_millis();
    let mut favorites: Vec<&WorldEntry> =
        state.worlds.iter().filter(|world| world.favorite).collect();
    favorites.sort_by(|a, b| {
        b.last_used_at_ms
            .unwrap_or(0)
            .cmp(&a.last_used_at_ms.unwrap_or(0))
            .then_with(|| a.world_name.cmp(&b.world_name))
    });

    let mut entries: Vec<HomeEntryRef<'_>> = state
        .worlds
        .iter()
        .filter(|world| !world.favorite)
        .map(HomeEntryRef::World)
        .collect();
    entries.extend(state.servers.iter().map(HomeEntryRef::Server));
    entries.sort_by(|a, b| {
        b.last_used_at_ms()
            .unwrap_or(0)
            .cmp(&a.last_used_at_ms().unwrap_or(0))
            .then_with(|| a.primary_label().cmp(b.primary_label()))
    });

    egui::ScrollArea::vertical()
        .id_salt("home_activity_scroll")
        .max_height(ui.available_height().max(180.0))
        .show(ui, |ui| {
            if !favorites.is_empty() {
                let _ = text_ui.label(
                    ui,
                    "home_activity_favorites_title",
                    "Favorites",
                    &LabelOptions {
                        weight: 700,
                        color: ui.visuals().text_color(),
                        wrap: false,
                        ..LabelOptions::default()
                    },
                );
                ui.add_space(4.0);
                for (index, world) in favorites.iter().enumerate() {
                    render_world_row(
                        ui,
                        text_ui,
                        world,
                        now_ms,
                        ("home_favorite_world", index),
                        instances,
                        output,
                        requested_rescan,
                    );
                    ui.add_space(2.0);
                }
                ui.separator();
                ui.add_space(8.0);
            }

            if entries.is_empty() {
                let _ = text_ui.label(
                    ui,
                    "home_activity_recent_empty",
                    "No recent worlds or servers.",
                    &LabelOptions {
                        color: ui.visuals().weak_text_color(),
                        wrap: true,
                        ..LabelOptions::default()
                    },
                );
                return;
            }

            let _ = text_ui.label(
                ui,
                "home_activity_recent_title",
                "Recent",
                &LabelOptions {
                    weight: 700,
                    color: ui.visuals().text_color(),
                    wrap: false,
                    ..LabelOptions::default()
                },
            );
            ui.add_space(4.0);
            for (index, entry) in entries.into_iter().enumerate() {
                match entry {
                    HomeEntryRef::World(world) => {
                        render_world_row(
                            ui,
                            text_ui,
                            world,
                            now_ms,
                            ("home_recent_world", index),
                            instances,
                            output,
                            requested_rescan,
                        );
                    }
                    HomeEntryRef::Server(server) => {
                        render_server_row(
                            ui,
                            text_ui,
                            server,
                            state
                                .server_pings
                                .get(&normalize_server_address(&server.address)),
                            now_ms,
                            ("home_recent_server", index),
                            output,
                        );
                    }
                }
                ui.add_space(2.0);
            }
        });
}

fn render_world_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    world: &WorldEntry,
    now_ms: u64,
    id_source: impl std::hash::Hash + Copy,
    instances: &mut InstanceStore,
    output: &mut HomeOutput,
    requested_rescan: &mut bool,
) {
    ui.horizontal(|ui| {
        let star_label = if world.favorite { "[*]" } else { "[ ]" };
        if ui
            .small_button(star_label)
            .on_hover_text("Toggle world favorite")
            .clicked()
        {
            let _ = set_world_favorite(
                instances,
                world.instance_id.as_str(),
                world.world_id.as_str(),
                !world.favorite,
            );
            *requested_rescan = true;
        }
        let row_response =
            render_clickable_entry_row(ui, (id_source, "row"), ACTIVITY_ROW_HEIGHT, |ui| {
                render_entry_thumbnail(ui, (id_source, "thumb"), assets::HOME_SVG, 34.0, 34.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    let _ = text_ui.label(
                        ui,
                        (id_source, "name"),
                        world.world_name.as_str(),
                        &LabelOptions {
                            weight: 600,
                            color: ui.visuals().text_color(),
                            wrap: false,
                            ..LabelOptions::default()
                        },
                    );
                    let _ = text_ui.label(
                        ui,
                        (id_source, "meta"),
                        world_meta_line(world, now_ms).as_str(),
                        &LabelOptions {
                            color: ui.visuals().weak_text_color(),
                            wrap: false,
                            ..LabelOptions::default()
                        },
                    );
                });
            });
        if row_response.clicked() {
            queue_launch_intent(
                ui.ctx(),
                PendingLaunchIntent {
                    nonce: current_time_millis(),
                    instance_id: world.instance_id.clone(),
                    quick_play_singleplayer: Some(world.world_id.clone()),
                    quick_play_multiplayer: None,
                },
            );
            output.selected_instance_id = Some(world.instance_id.clone());
            output.requested_screen = Some(AppScreen::Library);
        }
    });
}

fn world_meta_line(world: &WorldEntry, now_ms: u64) -> String {
    let mut parts = vec![
        format!("instance {}", world.instance_name),
        format!("folder {}", world.world_id),
        format!(
            "last used {}",
            format_time_ago(world.last_used_at_ms, now_ms)
        ),
    ];
    if let Some(game_mode) = world.game_mode.as_deref() {
        parts.push(game_mode.to_owned());
    }
    if let Some(difficulty) = world.difficulty.as_deref() {
        parts.push(format!("difficulty {difficulty}"));
    }
    if let Some(hardcore) = world.hardcore {
        parts.push(if hardcore {
            "hardcore".to_owned()
        } else {
            "non-hardcore".to_owned()
        });
    }
    if let Some(cheats_enabled) = world.cheats_enabled {
        parts.push(if cheats_enabled {
            "cheats on".to_owned()
        } else {
            "cheats off".to_owned()
        });
    }
    if let Some(version_name) = world.version_name.as_deref() {
        parts.push(format!("version {version_name}"));
    }
    parts.join(" | ")
}

fn render_server_row(
    ui: &mut Ui,
    text_ui: &mut TextUi,
    server: &ServerEntry,
    ping: Option<&ServerPingSnapshot>,
    now_ms: u64,
    id_source: impl std::hash::Hash + Copy,
    output: &mut HomeOutput,
) {
    let server_meta = server_meta_line(server, ping, now_ms);
    let row_response =
        render_clickable_entry_row(ui, (id_source, "row"), ACTIVITY_ROW_HEIGHT, |ui| {
            render_entry_thumbnail(ui, (id_source, "thumb"), assets::TERMINAL_SVG, 34.0, 34.0);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                let _ = text_ui.label(
                    ui,
                    (id_source, "name"),
                    server.server_name.as_str(),
                    &LabelOptions {
                        weight: 600,
                        color: ui.visuals().text_color(),
                        wrap: false,
                        ..LabelOptions::default()
                    },
                );
                let _ = text_ui.label(
                    ui,
                    (id_source, "meta"),
                    server_meta.as_str(),
                    &LabelOptions {
                        color: ui.visuals().weak_text_color(),
                        wrap: false,
                        ..LabelOptions::default()
                    },
                );
            });
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                render_server_ping_icon(ui, ping);
            });
        });
    if row_response.clicked() {
        queue_launch_intent(
            ui.ctx(),
            PendingLaunchIntent {
                nonce: current_time_millis(),
                instance_id: server.instance_id.clone(),
                quick_play_singleplayer: None,
                quick_play_multiplayer: Some(server.address.clone()),
            },
        );
        output.selected_instance_id = Some(server.instance_id.clone());
        output.requested_screen = Some(AppScreen::Library);
    }
}

fn server_meta_line(
    server: &ServerEntry,
    ping: Option<&ServerPingSnapshot>,
    now_ms: u64,
) -> String {
    let address = if server.port == 25565 {
        server.host.clone()
    } else {
        format!("{}:{}", server.host, server.port)
    };
    let ping_text = match ping.map(|value| value.status) {
        Some(ServerPingStatus::Online { latency_ms }) => format!("reachable {latency_ms}ms"),
        Some(ServerPingStatus::Offline) => "offline".to_owned(),
        _ => "status unknown".to_owned(),
    };
    format!(
        "{} | {} | {} | last used {}",
        server.instance_name,
        address,
        ping_text,
        format_time_ago(server.last_used_at_ms, now_ms)
    )
}

fn render_clickable_entry_row(
    ui: &mut Ui,
    id_source: impl std::hash::Hash,
    height: f32,
    add_contents: impl FnOnce(&mut Ui),
) -> egui::Response {
    let width = ui.available_width().max(1.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if response.is_pointer_button_down_on() {
        visuals.widgets.active.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.widgets.inactive.weak_bg_fill
    };
    let stroke = if response.hovered() {
        visuals.widgets.hovered.bg_stroke
    } else {
        visuals.widgets.inactive.bg_stroke
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink2(egui::vec2(8.0, 5.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(id_source)
            .max_rect(inner)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| add_contents(ui),
    );
    response
}

fn render_entry_thumbnail(
    ui: &mut Ui,
    id_source: impl std::hash::Hash,
    icon_svg: &'static [u8],
    width: f32,
    height: f32,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let visuals = ui.visuals();
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        visuals.selection.bg_fill.gamma_multiply(0.16),
        visuals.widgets.inactive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(((height - ENTRY_ICON_SIZE) * 0.5).max(0.0));
            let themed_svg = apply_color_to_svg(icon_svg, ui.visuals().text_color());
            let uri = format!("bytes://home/entry-thumb/{:?}.svg", ui.id().with(id_source));
            ui.add(
                egui::Image::from_bytes(uri, themed_svg)
                    .fit_to_exact_size(egui::vec2(ENTRY_ICON_SIZE, ENTRY_ICON_SIZE)),
            );
        });
    });
}

fn render_server_ping_icon(ui: &mut Ui, ping: Option<&ServerPingSnapshot>) {
    let (icon, color, tip) = ping_icon_for_status(ping.map(|snapshot| snapshot.status));
    let themed_svg = apply_color_to_svg(icon, color);
    let uri = format!(
        "bytes://home/server-ping/{:?}-{:02x}{:02x}{:02x}.svg",
        ping.map(|value| value.status),
        color.r(),
        color.g(),
        color.b()
    );
    ui.add(
        egui::Image::from_bytes(uri, themed_svg)
            .fit_to_exact_size(egui::vec2(16.0, 16.0))
            .sense(egui::Sense::hover()),
    )
    .on_hover_text(tip);
}

fn ping_icon_for_status(status: Option<ServerPingStatus>) -> (&'static [u8], Color32, String) {
    match status.unwrap_or(ServerPingStatus::Unknown) {
        ServerPingStatus::Unknown => (
            assets::ANTENNA_BARS_OFF_SVG,
            Color32::from_rgb(145, 145, 145),
            "Ping unknown".to_owned(),
        ),
        ServerPingStatus::Offline => (
            assets::ANTENNA_BARS_OFF_SVG,
            Color32::from_rgb(205, 90, 90),
            "Server offline".to_owned(),
        ),
        ServerPingStatus::Online { latency_ms } => {
            let (icon, color) = if latency_ms <= 80 {
                (assets::ANTENNA_BARS_5_SVG, Color32::from_rgb(80, 190, 110))
            } else if latency_ms <= 140 {
                (assets::ANTENNA_BARS_4_SVG, Color32::from_rgb(120, 190, 90))
            } else if latency_ms <= 220 {
                (assets::ANTENNA_BARS_3_SVG, Color32::from_rgb(195, 175, 90))
            } else if latency_ms <= 320 {
                (assets::ANTENNA_BARS_2_SVG, Color32::from_rgb(210, 145, 80))
            } else {
                (assets::ANTENNA_BARS_1_SVG, Color32::from_rgb(220, 110, 80))
            };
            (icon, color, format!("Latency: {latency_ms}ms"))
        }
    }
}

fn apply_color_to_svg(svg_bytes: &[u8], color: Color32) -> Vec<u8> {
    let color_hex = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());
    let svg = String::from_utf8_lossy(svg_bytes).replace("currentColor", color_hex.as_str());
    svg.into_bytes()
}

fn refresh_home_state(state: &mut HomeState, instances: &InstanceStore, config: &Config) {
    let installations_root = PathBuf::from(config.minecraft_installations_root());
    state.worlds = collect_worlds(instances, installations_root.as_path());
    state.servers = collect_servers(instances, installations_root.as_path());
    refresh_server_pings(state);
    state.scanned_instance_count = instances.instances.len();
    state.last_scan_at = Some(Instant::now());
}

fn collect_worlds(instances: &InstanceStore, installations_root: &Path) -> Vec<WorldEntry> {
    let mut worlds = Vec::new();
    for instance in &instances.instances {
        let root = instance_root_path(installations_root, instance);
        let saves_dir = root.join("saves");
        let Ok(entries) = fs::read_dir(saves_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let world_id = entry.file_name().to_string_lossy().to_string();
            if world_id.trim().is_empty() {
                continue;
            }
            let level_dat_path = path.join("level.dat");
            let metadata = parse_world_metadata(level_dat_path.as_path()).unwrap_or_default();
            let world_name = metadata
                .level_name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(world_id.as_str())
                .to_owned();
            let last_used_at_ms = metadata
                .last_played_ms
                .or_else(|| modified_millis(level_dat_path.as_path()))
                .or_else(|| modified_millis(path.as_path()));
            worlds.push(WorldEntry {
                instance_id: instance.id.clone(),
                instance_name: instance.name.clone(),
                world_id: world_id.clone(),
                world_name,
                game_mode: metadata.game_mode,
                hardcore: metadata.hardcore,
                cheats_enabled: metadata.cheats_enabled,
                difficulty: metadata.difficulty,
                version_name: metadata.version_name,
                last_used_at_ms,
                favorite: instance.favorite_world_ids.iter().any(|id| id == &world_id),
            });
        }
    }
    worlds.sort_by(|a, b| {
        b.last_used_at_ms
            .unwrap_or(0)
            .cmp(&a.last_used_at_ms.unwrap_or(0))
            .then_with(|| a.world_name.cmp(&b.world_name))
    });
    worlds
}

fn collect_servers(instances: &InstanceStore, installations_root: &Path) -> Vec<ServerEntry> {
    let mut servers = Vec::new();
    for instance in &instances.instances {
        let root = instance_root_path(installations_root, instance);
        let servers_dat = root.join("servers.dat");
        let last_used_at_ms = modified_millis(servers_dat.as_path());
        let parsed = parse_servers_dat(servers_dat.as_path()).unwrap_or_default();
        for server in parsed {
            let (host, port) = split_server_address(server.ip.as_str());
            servers.push(ServerEntry {
                instance_id: instance.id.clone(),
                instance_name: instance.name.clone(),
                server_name: server.name,
                address: server.ip,
                host,
                port,
                last_used_at_ms,
            });
        }
    }
    servers.sort_by(|a, b| {
        b.last_used_at_ms
            .unwrap_or(0)
            .cmp(&a.last_used_at_ms.unwrap_or(0))
            .then_with(|| a.server_name.cmp(&b.server_name))
    });
    servers
}

fn refresh_server_pings(state: &mut HomeState) {
    let known_addresses: HashSet<String> = state
        .servers
        .iter()
        .map(|server| normalize_server_address(server.address.as_str()))
        .collect();
    state
        .server_pings
        .retain(|address, _| known_addresses.contains(address));

    let mut stale_addresses = Vec::new();
    for server in &state.servers {
        let key = normalize_server_address(server.address.as_str());
        let stale = state
            .server_pings
            .get(&key)
            .is_none_or(|snapshot| snapshot.checked_at.elapsed() >= SERVER_PING_REFRESH_INTERVAL);
        if stale && !stale_addresses.iter().any(|candidate| candidate == &key) {
            stale_addresses.push(key);
        }
    }

    for address in stale_addresses.into_iter().take(SERVER_PINGS_PER_SCAN) {
        let status = probe_server_status(address.as_str());
        state.server_pings.insert(
            address,
            ServerPingSnapshot {
                status,
                checked_at: Instant::now(),
            },
        );
    }
}

fn normalize_server_address(address: &str) -> String {
    address.trim().to_ascii_lowercase()
}

fn split_server_address(address: &str) -> (String, u16) {
    let trimmed = address.trim();
    if trimmed.is_empty() {
        return (String::new(), 25565);
    }
    if let Ok(socket) = trimmed.parse::<SocketAddr>() {
        return (socket.ip().to_string(), socket.port());
    }
    if let Some(host) = trimmed
        .strip_prefix('[')
        .and_then(|value| value.split(']').next())
        && let Some(port) = trimmed
            .rsplit_once(':')
            .and_then(|(_, value)| value.parse().ok())
    {
        return (host.to_owned(), port);
    }
    if let Some((host, port)) = trimmed.rsplit_once(':')
        && host.contains('.')
        && let Ok(port) = port.parse::<u16>()
    {
        return (host.to_owned(), port);
    }
    (trimmed.to_owned(), 25565)
}

fn probe_server_status(address: &str) -> ServerPingStatus {
    let (host, port) = split_server_address(address);
    if host.is_empty() {
        return ServerPingStatus::Unknown;
    }
    let start = Instant::now();
    let mut saw_target = false;
    if let Ok(ip) = host.parse::<IpAddr>() {
        saw_target = true;
        if TcpStream::connect_timeout(&SocketAddr::new(ip, port), SERVER_PING_CONNECT_TIMEOUT)
            .is_ok()
        {
            return ServerPingStatus::Online {
                latency_ms: start.elapsed().as_millis() as u64,
            };
        }
    } else if let Ok(candidates) = (host.as_str(), port).to_socket_addrs() {
        for candidate in candidates {
            saw_target = true;
            if TcpStream::connect_timeout(&candidate, SERVER_PING_CONNECT_TIMEOUT).is_ok() {
                return ServerPingStatus::Online {
                    latency_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
    } else {
        return ServerPingStatus::Unknown;
    }

    if saw_target {
        ServerPingStatus::Offline
    } else {
        ServerPingStatus::Unknown
    }
}

fn parse_world_metadata(path: &Path) -> Option<WorldMetadata> {
    let data = read_nbt_file(path)?;
    parse_world_metadata_from_nbt(data.as_slice()).ok()
}

fn parse_world_metadata_from_nbt(bytes: &[u8]) -> Result<WorldMetadata, ()> {
    let mut cursor = NbtCursor::new(bytes);
    let root_tag = cursor.read_u8()?;
    if root_tag != 10 {
        return Err(());
    }
    let _ = cursor.read_string()?;
    let mut metadata = WorldMetadata::default();
    loop {
        let tag = cursor.read_u8()?;
        if tag == 0 {
            break;
        }
        let key = cursor.read_string()?;
        if tag == 10 && key == "Data" {
            parse_world_data_compound(&mut cursor, &mut metadata)?;
        } else {
            skip_nbt_payload(&mut cursor, tag)?;
        }
    }
    Ok(metadata)
}

fn parse_world_data_compound(
    cursor: &mut NbtCursor<'_>,
    metadata: &mut WorldMetadata,
) -> Result<(), ()> {
    loop {
        let tag = cursor.read_u8()?;
        if tag == 0 {
            return Ok(());
        }
        let key = cursor.read_string()?;
        match (tag, key.as_str()) {
            (8, "LevelName") => metadata.level_name = Some(cursor.read_string()?),
            (3, "GameType") => metadata.game_mode = Some(game_mode_label(cursor.read_i32()?)),
            (1, "hardcore") => metadata.hardcore = Some(cursor.read_u8()? != 0),
            (1, "allowCommands") => metadata.cheats_enabled = Some(cursor.read_u8()? != 0),
            (1, "Difficulty") => metadata.difficulty = Some(difficulty_label(cursor.read_u8()?)),
            (4, "LastPlayed") => {
                let last_played = cursor.read_i64()?;
                if last_played > 0 {
                    metadata.last_played_ms = Some(last_played as u64);
                }
            }
            (10, "Version") => parse_world_version_compound(cursor, metadata)?,
            _ => skip_nbt_payload(cursor, tag)?,
        }
    }
}

fn parse_world_version_compound(
    cursor: &mut NbtCursor<'_>,
    metadata: &mut WorldMetadata,
) -> Result<(), ()> {
    loop {
        let tag = cursor.read_u8()?;
        if tag == 0 {
            return Ok(());
        }
        let key = cursor.read_string()?;
        match (tag, key.as_str()) {
            (8, "Name") => metadata.version_name = Some(cursor.read_string()?),
            _ => skip_nbt_payload(cursor, tag)?,
        }
    }
}

fn game_mode_label(game_type: i32) -> String {
    match game_type {
        0 => "survival".to_owned(),
        1 => "creative".to_owned(),
        2 => "adventure".to_owned(),
        3 => "spectator".to_owned(),
        other => format!("mode {other}"),
    }
}

fn difficulty_label(value: u8) -> String {
    match value {
        0 => "peaceful".to_owned(),
        1 => "easy".to_owned(),
        2 => "normal".to_owned(),
        3 => "hard".to_owned(),
        other => format!("difficulty {other}"),
    }
}

fn read_nbt_file(path: &Path) -> Option<Vec<u8>> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return Some(Vec::new());
    }
    if bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).ok()?;
        return Some(out);
    }
    Some(bytes)
}

fn parse_servers_dat(path: &Path) -> Option<Vec<ServerDatEntry>> {
    let data = read_nbt_file(path)?;
    parse_servers_from_nbt(data.as_slice()).ok()
}

fn parse_servers_from_nbt(bytes: &[u8]) -> Result<Vec<ServerDatEntry>, ()> {
    let mut cursor = NbtCursor::new(bytes);
    let root_tag = cursor.read_u8()?;
    if root_tag != 10 {
        return Err(());
    }
    let _ = cursor.read_string()?;
    let mut servers = Vec::new();
    parse_compound_for_servers(&mut cursor, &mut servers)?;
    Ok(servers)
}

fn parse_compound_for_servers(
    cursor: &mut NbtCursor<'_>,
    servers: &mut Vec<ServerDatEntry>,
) -> Result<(), ()> {
    loop {
        let tag = cursor.read_u8()?;
        if tag == 0 {
            return Ok(());
        }
        let name = cursor.read_string()?;
        if tag == 9 && name == "servers" {
            parse_servers_list(cursor, servers)?;
        } else {
            skip_nbt_payload(cursor, tag)?;
        }
    }
}

fn parse_servers_list(cursor: &mut NbtCursor<'_>, out: &mut Vec<ServerDatEntry>) -> Result<(), ()> {
    let item_tag = cursor.read_u8()?;
    let len = cursor.read_i32()?;
    if len <= 0 {
        return Ok(());
    }
    let len = len as usize;
    for _ in 0..len {
        if item_tag == 10 {
            if let Some(entry) = parse_server_compound(cursor)? {
                out.push(entry);
            }
        } else {
            skip_nbt_payload(cursor, item_tag)?;
        }
    }
    Ok(())
}

fn parse_server_compound(cursor: &mut NbtCursor<'_>) -> Result<Option<ServerDatEntry>, ()> {
    let mut name = String::new();
    let mut ip = String::new();
    loop {
        let tag = cursor.read_u8()?;
        if tag == 0 {
            break;
        }
        let key = cursor.read_string()?;
        match (tag, key.as_str()) {
            (8, "name") => name = cursor.read_string()?,
            (8, "ip") => ip = cursor.read_string()?,
            _ => skip_nbt_payload(cursor, tag)?,
        }
    }
    if ip.trim().is_empty() {
        return Ok(None);
    }
    if name.trim().is_empty() {
        name = ip.clone();
    }
    Ok(Some(ServerDatEntry { name, ip }))
}

fn skip_nbt_payload(cursor: &mut NbtCursor<'_>, tag: u8) -> Result<(), ()> {
    match tag {
        0 => Ok(()),
        1 => cursor.skip(1),
        2 => cursor.skip(2),
        3 => cursor.skip(4),
        4 => cursor.skip(8),
        5 => cursor.skip(4),
        6 => cursor.skip(8),
        7 => {
            let len = cursor.read_i32()?;
            if len < 0 {
                return Err(());
            }
            cursor.skip(len as usize)
        }
        8 => {
            let len = cursor.read_u16()? as usize;
            cursor.skip(len)
        }
        9 => {
            let nested_tag = cursor.read_u8()?;
            let len = cursor.read_i32()?;
            if len < 0 {
                return Err(());
            }
            for _ in 0..(len as usize) {
                skip_nbt_payload(cursor, nested_tag)?;
            }
            Ok(())
        }
        10 => loop {
            let nested = cursor.read_u8()?;
            if nested == 0 {
                break Ok(());
            }
            let _ = cursor.read_string()?;
            skip_nbt_payload(cursor, nested)?;
        },
        11 => {
            let len = cursor.read_i32()?;
            if len < 0 {
                return Err(());
            }
            cursor.skip((len as usize) * 4)
        }
        12 => {
            let len = cursor.read_i32()?;
            if len < 0 {
                return Err(());
            }
            cursor.skip((len as usize) * 8)
        }
        _ => Err(()),
    }
}

#[derive(Debug)]
struct NbtCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> NbtCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn skip(&mut self, len: usize) -> Result<(), ()> {
        if self.pos.saturating_add(len) > self.bytes.len() {
            return Err(());
        }
        self.pos += len;
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8, ()> {
        if self.pos >= self.bytes.len() {
            return Err(());
        }
        let value = self.bytes[self.pos];
        self.pos += 1;
        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, ()> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_i32(&mut self) -> Result<i32, ()> {
        let bytes = self.read_exact(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i64(&mut self) -> Result<i64, ()> {
        let bytes = self.read_exact(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_string(&mut self) -> Result<String, ()> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_exact(len)?;
        Ok(String::from_utf8_lossy(bytes).to_string())
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], ()> {
        if self.pos.saturating_add(len) > self.bytes.len() {
            return Err(());
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.bytes[start..start + len])
    }
}

fn modified_millis(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    Some(
        modified
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default(),
    )
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn format_time_ago(timestamp_ms: Option<u64>, now_ms: u64) -> String {
    let Some(timestamp_ms) = timestamp_ms else {
        return "never".to_owned();
    };
    let seconds = now_ms.saturating_sub(timestamp_ms) / 1000;
    if seconds < 60 {
        return format!("{seconds}s ago");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    format!("{days}d ago")
}
