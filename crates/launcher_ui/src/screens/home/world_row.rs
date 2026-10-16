use super::*;



pub(super) fn world_meta_line(world: &WorldEntry, now_ms: u64) -> String {
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
