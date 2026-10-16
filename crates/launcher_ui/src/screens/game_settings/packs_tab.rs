use super::*;



pub(super) fn status_text(
    status: PackStatus,
    support: Option<PackSupport>,
    version: Option<Ver>,
) -> String {
    let format = version.and_then(mc_options::packs::resource_format_for);
    let format_note = format.map(|f| {
        format!(
            "Minecraft {} uses format {}.{}",
            version.map_or(String::new(), |v| format!("{}.{}", v.0, v.1)),
            f.major,
            f.minor
        )
    });
    match status {
        PackStatus::Compatible => "Compatible with this version".to_owned(),
        PackStatus::Incompatible => format!(
            "Not declared for this version{}",
            format_note.map(|n| format!(" ({n})")).unwrap_or_default()
        ),
        PackStatus::Unknown => {
            if support.is_none() {
                "No readable pack.mcmeta; compatibility unknown".to_owned()
            } else {
                "Compatibility unknown for this version".to_owned()
            }
        }
        PackStatus::ForcedAllow => "Marked as working here".to_owned(),
        PackStatus::ForcedDeny => "Marked as not working here".to_owned(),
    }
}


