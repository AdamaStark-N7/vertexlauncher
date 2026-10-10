use super::*;

const QUICK_LAUNCH_SOURCE: &str = "home/quick_launch";

/// Copies a quick-launch string built for the selected user, or warns when nobody is signed in.
fn copy_quick_launch_text(
    ctx: &egui::Context,
    auth: &PlayerAuthContext<'_>,
    sign_in_message: &str,
    copied_message: &str,
    build: impl FnOnce(&str) -> String,
) {
    let Some(user) = selected_quick_launch_user(auth) else {
        notification::warn!(QUICK_LAUNCH_SOURCE, "{}", sign_in_message);
        return;
    };
    ctx.copy_text(build(user.as_str()));
    notification::info!(QUICK_LAUNCH_SOURCE, "{}", copied_message);
}

pub(super) fn copy_instance_launch_command(
    ctx: &egui::Context,
    instance_id: &str,
    auth: &PlayerAuthContext<'_>,
) {
    copy_quick_launch_text(
        ctx,
        auth,
        "Sign in before copying an instance command line.",
        "Copied instance command line to clipboard.",
        |user| {
            build_quick_launch_command(QuickLaunchCommandMode::Pack, instance_id, user, None, None)
        },
    );
}

pub(super) fn copy_instance_steam_launch_options(
    ctx: &egui::Context,
    instance_id: &str,
    auth: &PlayerAuthContext<'_>,
) {
    copy_quick_launch_text(
        ctx,
        auth,
        "Sign in before copying Steam launch options.",
        "Copied Steam launch options to clipboard.",
        |user| {
            build_quick_launch_steam_options(
                QuickLaunchCommandMode::Pack,
                instance_id,
                user,
                None,
                None,
            )
        },
    );
}

pub(super) fn copy_world_launch_command(
    ctx: &egui::Context,
    world: &WorldEntry,
    auth: &PlayerAuthContext<'_>,
) {
    copy_quick_launch_text(
        ctx,
        auth,
        "Sign in before copying a world command line.",
        "Copied world command line to clipboard.",
        |user| {
            build_quick_launch_command(
                QuickLaunchCommandMode::World,
                world.instance_id.as_str(),
                user,
                Some(world.world_id.as_str()),
                None,
            )
        },
    );
}

pub(super) fn copy_world_steam_launch_options(
    ctx: &egui::Context,
    world: &WorldEntry,
    auth: &PlayerAuthContext<'_>,
) {
    copy_quick_launch_text(
        ctx,
        auth,
        "Sign in before copying Steam launch options.",
        "Copied Steam launch options to clipboard.",
        |user| {
            build_quick_launch_steam_options(
                QuickLaunchCommandMode::World,
                world.instance_id.as_str(),
                user,
                Some(world.world_id.as_str()),
                None,
            )
        },
    );
}

pub(super) fn copy_server_launch_command(
    ctx: &egui::Context,
    server: &ServerEntry,
    auth: &PlayerAuthContext<'_>,
) {
    copy_quick_launch_text(
        ctx,
        auth,
        "Sign in before copying a server command line.",
        "Copied server command line to clipboard.",
        |user| {
            build_quick_launch_command(
                QuickLaunchCommandMode::Server,
                server.instance_id.as_str(),
                user,
                None,
                Some(server.address.as_str()),
            )
        },
    );
}

pub(super) fn copy_server_steam_launch_options(
    ctx: &egui::Context,
    server: &ServerEntry,
    auth: &PlayerAuthContext<'_>,
) {
    copy_quick_launch_text(
        ctx,
        auth,
        "Sign in before copying Steam launch options.",
        "Copied Steam launch options to clipboard.",
        |user| {
            build_quick_launch_steam_options(
                QuickLaunchCommandMode::Server,
                server.instance_id.as_str(),
                user,
                None,
                Some(server.address.as_str()),
            )
        },
    );
}
