use super::*;

#[path = "content_browser_ui/content_browser_ui_metrics.rs"]
mod content_browser_ui_metrics;
#[path = "content_browser_ui/detail.rs"]
mod detail;
#[path = "content_browser_ui/matching.rs"]
mod matching;
#[path = "content_browser_ui/search_tag_chips.rs"]
mod search_tag_chips;

pub(super) use self::detail::open_detail_page;
pub(super) use self::matching::{
    browser_loader_from_modloader, installed_project_for_entry, normalize_search_key,
    parse_content_type,
};
pub(in crate::screens) use self::matching::{version_matches_game_version, version_matches_loader};
pub(super) use self::search_tag_chips::add_search_tag;

