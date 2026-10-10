#[path = "notification/emitters.rs"]
mod emitters;
#[path = "notification/natural_language.rs"]
pub mod natural_language;
#[path = "notification/popups.rs"]
mod popups;
#[path = "notification/store.rs"]
mod store;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use config::NotificationEasing;

use crate::assets;

pub use emitters::{
    NotificationBuilder, emit, emit_progress, emit_replace, emit_spinner, emit_with_actions,
    emit_with_icon, retract_from_stack,
};
pub use natural_language::{count_word, join_natural};
pub use popups::render_popups;
use store::{drain_notifications, entry_fade_opacity, entry_ttl};

const NOTIFICATION_TTL: Duration = Duration::from_secs(7);
/// Notifications carrying buttons stay up longer so there is time to reach them.
const NOTIFICATION_ACTION_TTL: Duration = Duration::from_secs(15);
const PROGRESS_NOTIFICATION_STALE_TTL: Duration = Duration::from_secs(14);
const NOTIFICATION_MAX_STACK: usize = 8;
const NOTIFICATION_EXPIRY_BAR_HEIGHT: f32 = 4.0;

/// User-configurable presentation settings for notification popups.
#[derive(Debug, Clone, Copy)]
pub struct NotificationDisplaySettings {
    pub expiry_bars_empty_left: bool,
    pub fade_out: Duration,
    pub easing: NotificationEasing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Log,
    Info,
    Warn,
    Error,
}

/// A button attached to a notification. The callback runs on the UI thread when it is clicked.
#[derive(Clone)]
pub struct NotificationAction {
    pub label: String,
    pub callback: Arc<dyn Fn(&egui::Context) + Send + Sync>,
    /// Draws the label in the theme's danger color, for destructive actions.
    pub danger: bool,
}

impl NotificationAction {
    pub fn new(
        label: impl Into<String>,
        callback: impl Fn(&egui::Context) + Send + Sync + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            callback: Arc::new(callback),
            danger: false,
        }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

impl std::fmt::Debug for NotificationAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationAction")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// How a notification with its own details merges into similar ones that arrive while it is
/// still showing, instead of piling up as separate popups.
///
/// Notifications sharing a `key` become one popup. Each arrival adds its `items`, and the
/// message is rewritten from *all* items by `render`, so the text stays natural ("got two new
/// capes: A and B") rather than being appended to. Notifications with no details of their own
/// need no spec: identical ones collapse automatically and show an "xN" badge by the icon.
#[derive(Clone)]
pub struct StackSpec {
    pub key: String,
    pub items: Vec<String>,
    pub render: Arc<dyn Fn(&[String]) -> String + Send + Sync>,
    /// Takes `items` back out of the stack instead of adding them (see [`retract_from_stack`]).
    pub remove: bool,
}

impl std::fmt::Debug for StackSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackSpec")
            .field("key", &self.key)
            .field("items", &self.items)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub severity: Severity,
    pub source: String,
    pub message: String,
    pub progress: Option<f32>,
    pub spinner: bool,
    pub replace_key: Option<String>,
    pub actions: Vec<NotificationAction>,
    /// SVG shown instead of the severity icon.
    pub icon: Option<&'static [u8]>,
    pub stack: Option<StackSpec>,
    pub when: Instant,
}

#[derive(Debug, Clone)]
struct NotificationEntry {
    id: u64,
    severity: Severity,
    source: String,
    message: String,
    progress: Option<f32>,
    spinner: bool,
    replace_key: Option<String>,
    actions: Vec<NotificationAction>,
    icon: Option<&'static [u8]>,
    stack: Option<StackSpec>,
    count: u32,
    last_seen: Instant,
}

#[derive(Default)]
struct NotificationStore {
    entries: Vec<NotificationEntry>,
    next_id: u64,
}

struct NotificationCenter {
    tx: mpsc::Sender<Notification>,
    rx: Mutex<mpsc::Receiver<Notification>>,
    store: Mutex<NotificationStore>,
}

static NOTIFICATION_CENTER: OnceLock<NotificationCenter> = OnceLock::new();
static STREAMER_MODE_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_streamer_mode(enabled: bool) {
    STREAMER_MODE_ENABLED.store(enabled, Ordering::Relaxed);
}

fn streamer_mode_enabled() -> bool {
    STREAMER_MODE_ENABLED.load(Ordering::Relaxed)
}

fn center() -> &'static NotificationCenter {
    NOTIFICATION_CENTER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Notification>();
        NotificationCenter {
            tx,
            rx: Mutex::new(rx),
            store: Mutex::new(NotificationStore::default()),
        }
    })
}

fn icon_for_severity(severity: Severity) -> &'static [u8] {
    match severity {
        Severity::Log => assets::LOG_SVG,
        Severity::Info => assets::INFO_CIRCLE_SVG,
        Severity::Warn => assets::WARN_SVG,
        Severity::Error => assets::ERROR_SVG,
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Log => "Log",
        Severity::Info => "Info",
        Severity::Warn => "Warn",
        Severity::Error => "Error",
    }
}

#[macro_export]
macro_rules! __notification_log {
    ($source:expr, $fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Log,
            $source,
            format!($fmt $(, $arg )*),
        )
    };
    ($fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Log,
            module_path!(),
            format!($fmt $(, $arg )*),
        )
    };
}

#[macro_export]
macro_rules! __notification_info {
    ($source:expr, $fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Info,
            $source,
            format!($fmt $(, $arg )*),
        )
    };
    ($fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Info,
            module_path!(),
            format!($fmt $(, $arg )*),
        )
    };
}

#[macro_export]
macro_rules! __notification_warn {
    ($source:expr, $fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Warn,
            $source,
            format!($fmt $(, $arg )*),
        )
    };
    ($fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Warn,
            module_path!(),
            format!($fmt $(, $arg )*),
        )
    };
}

#[macro_export]
macro_rules! __notification_error {
    ($source:expr, $fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Error,
            $source,
            format!($fmt $(, $arg )*),
        )
    };
    ($fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit(
            $crate::notification::Severity::Error,
            module_path!(),
            format!($fmt $(, $arg )*),
        )
    };
}

pub use crate::__notification_error as error;
pub use crate::__notification_info as info;
pub use crate::__notification_log as log;
pub use crate::__notification_warn as warn;

#[macro_export]
macro_rules! __notification_progress {
    ($severity:expr, $source:expr, $progress:expr, $fmt:literal $(, $arg:expr )* $(,)?) => {
        $crate::notification::emit_progress(
            $severity,
            $source,
            format!($fmt $(, $arg )*),
            $progress,
        )
    };
}

pub use crate::__notification_progress as progress;
