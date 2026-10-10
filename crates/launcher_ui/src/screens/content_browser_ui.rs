use super::*;
use textui_egui::interaction;

#[path = "content_browser_ui/content_browser_ui_metrics.rs"]
mod content_browser_ui_metrics;
#[path = "content_browser_ui/controls.rs"]
mod controls;
#[path = "content_browser_ui/detail.rs"]
mod detail;
#[path = "content_browser_ui/icon_button_outcome.rs"]
mod icon_button_outcome;
#[path = "content_browser_ui/matching.rs"]
mod matching;
#[path = "content_browser_ui/render_results_outcome.rs"]
mod render_results_outcome;
#[path = "content_browser_ui/result_tile_inner_outcome.rs"]
mod result_tile_inner_outcome;
#[path = "content_browser_ui/result_tile_outcome.rs"]
mod result_tile_outcome;
#[path = "content_browser_ui/results.rs"]
mod results;
#[path = "content_browser_ui/search_tag_chips.rs"]
mod search_tag_chips;
#[path = "content_browser_ui/widgets.rs"]
mod widgets;

use self::content_browser_ui_metrics::ContentBrowserUiMetrics;
pub(super) use self::controls::render_controls;
pub(super) use self::detail::{open_detail_page, render_detail_page};
use self::icon_button_outcome::IconButtonOutcome;
pub(super) use self::matching::{
    browser_loader_from_modloader, installed_project_for_entry, normalize_search_key,
    parse_content_type,
};
use self::matching::{version_matches_game_version, version_matches_loader};
pub(super) use self::render_results_outcome::RenderResultsOutcome;
use self::result_tile_inner_outcome::ResultTileInnerOutcome;
use self::result_tile_outcome::ResultTileOutcome;
use self::results::render_chip;
pub(super) use self::results::render_results;
use self::search_tag_chips::{SearchTagChipIds, add_search_tag, render_search_tag_chips};
use self::widgets::{render_browser_thumbnail, render_rounded_icon_button};

const SEARCH_TAG_CHIP_IDS: SearchTagChipIds = SearchTagChipIds {
    remove_icon_uri_prefix: "bytes://content-browser/tag-remove",
    label_id: "content_browser_search_tag",
};
