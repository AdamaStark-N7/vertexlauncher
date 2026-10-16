use super::*;

pub(in crate::screens::content_browser) fn open_detail_page(
    state: &mut ContentBrowserState,
    entry: &BrowserProjectEntry,
) {
    let same_entry = state
        .detail_entry
        .as_ref()
        .is_some_and(|current| current.dedupe_key == entry.dedupe_key);
    state.current_view = ContentBrowserPage::Detail;
    if !same_entry {
        state.detail_entry = Some(entry.clone());
        state.detail_tab = ContentDetailTab::Overview;
        state.detail_versions.clear();
        state.detail_versions_project_key = None;
        state.detail_versions_error = None;
        state.detail_versions_in_flight = false;
        state.detail_loader_filter = state.loader;
        state.detail_minecraft_version_filter = state.minecraft_version_filter.clone();
    }
    request_detail_versions(state);
}






