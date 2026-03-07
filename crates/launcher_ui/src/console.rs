use std::collections::{HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_CONSOLE_LINES: usize = 4000;
const DEFAULT_TAB_ID: &str = "vertexlauncher";
const DEFAULT_TAB_LABEL: &str = "VertexLauncher";
const INSTANCE_TAB_PRUNE_GRACE: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub struct ConsoleTabSnapshot {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct ConsoleSnapshot {
    pub tabs: Vec<ConsoleTabSnapshot>,
    pub active_tab_id: String,
    pub active_lines: Vec<String>,
}

#[derive(Debug)]
struct ConsoleTab {
    id: String,
    label: String,
    instance_root: Option<String>,
    missing_since: Option<Instant>,
    lines: VecDeque<String>,
}

#[derive(Debug)]
struct ConsoleState {
    tabs: Vec<ConsoleTab>,
    active_tab_id: String,
}

static CONSOLE_STATE: OnceLock<Mutex<ConsoleState>> = OnceLock::new();

fn store() -> &'static Mutex<ConsoleState> {
    CONSOLE_STATE.get_or_init(|| {
        Mutex::new(ConsoleState {
            tabs: vec![ConsoleTab {
                id: DEFAULT_TAB_ID.to_owned(),
                label: DEFAULT_TAB_LABEL.to_owned(),
                instance_root: None,
                missing_since: None,
                lines: VecDeque::new(),
            }],
            active_tab_id: DEFAULT_TAB_ID.to_owned(),
        })
    })
}

pub fn push_line(line: impl Into<String>) {
    push_line_to_tab(DEFAULT_TAB_ID, line);
}

pub fn push_line_to_tab(tab_id: &str, line: impl Into<String>) {
    let Ok(mut lines) = store().lock() else {
        return;
    };
    let Some(tab) = lines.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };
    tab.lines.push_back(line.into());
    while tab.lines.len() > MAX_CONSOLE_LINES {
        let _ = tab.lines.pop_front();
    }
}

pub fn ensure_instance_tab(instance_name: &str, username: &str, instance_root: &str) -> String {
    let trimmed_instance = instance_name.trim();
    let trimmed_user = username.trim();
    let instance = if trimmed_instance.is_empty() {
        "Instance"
    } else {
        trimmed_instance
    };
    let user = if trimmed_user.is_empty() {
        "Player"
    } else {
        trimmed_user
    };
    let label = format!("{instance} for {user}");
    let id = format!(
        "instance:{}:{}",
        instance.to_ascii_lowercase(),
        user.to_ascii_lowercase()
    );
    let normalized_instance_root = instance_root.trim().to_owned();

    let Ok(mut state) = store().lock() else {
        return id;
    };
    if let Some(existing) = state.tabs.iter_mut().find(|tab| tab.id == id) {
        existing.instance_root = if normalized_instance_root.is_empty() {
            None
        } else {
            Some(normalized_instance_root.clone())
        };
        existing.missing_since = None;
    } else {
        state.tabs.push(ConsoleTab {
            id: id.clone(),
            label,
            instance_root: if normalized_instance_root.is_empty() {
                None
            } else {
                Some(normalized_instance_root)
            },
            missing_since: None,
            lines: VecDeque::new(),
        });
    }
    state.active_tab_id = id.clone();
    id
}

pub fn prune_instance_tabs(active_instance_roots: &[String]) {
    let Ok(mut state) = store().lock() else {
        return;
    };
    let now = Instant::now();
    let active_roots: HashSet<&str> = active_instance_roots.iter().map(String::as_str).collect();

    for tab in &mut state.tabs {
        let Some(root) = tab.instance_root.as_deref() else {
            continue;
        };
        if active_roots.contains(root) {
            tab.missing_since = None;
        } else if tab.missing_since.is_none() {
            tab.missing_since = Some(now);
        }
    }

    state.tabs.retain(|tab| {
        let Some(_) = tab.instance_root.as_deref() else {
            return true;
        };
        tab.missing_since.is_none_or(|missing_since| {
            now.duration_since(missing_since) < INSTANCE_TAB_PRUNE_GRACE
        })
    });

    if state.tabs.is_empty() {
        state.tabs.push(ConsoleTab {
            id: DEFAULT_TAB_ID.to_owned(),
            label: DEFAULT_TAB_LABEL.to_owned(),
            instance_root: None,
            missing_since: None,
            lines: VecDeque::new(),
        });
    }

    if !state.tabs.iter().any(|tab| tab.id == state.active_tab_id) {
        state.active_tab_id = DEFAULT_TAB_ID.to_owned();
    }
}

pub fn set_active_tab(tab_id: &str) {
    let Ok(mut state) = store().lock() else {
        return;
    };
    if state.tabs.iter().any(|tab| tab.id == tab_id) {
        state.active_tab_id = tab_id.to_owned();
    }
}

pub fn snapshot() -> ConsoleSnapshot {
    let Ok(state) = store().lock() else {
        return ConsoleSnapshot {
            tabs: vec![ConsoleTabSnapshot {
                id: DEFAULT_TAB_ID.to_owned(),
                label: DEFAULT_TAB_LABEL.to_owned(),
            }],
            active_tab_id: DEFAULT_TAB_ID.to_owned(),
            active_lines: Vec::new(),
        };
    };

    let active_lines = state
        .tabs
        .iter()
        .find(|tab| tab.id == state.active_tab_id)
        .map(|tab| tab.lines.iter().cloned().collect())
        .unwrap_or_default();
    let tabs = state
        .tabs
        .iter()
        .map(|tab| ConsoleTabSnapshot {
            id: tab.id.clone(),
            label: tab.label.clone(),
        })
        .collect();

    ConsoleSnapshot {
        tabs,
        active_tab_id: state.active_tab_id.clone(),
        active_lines,
    }
}
