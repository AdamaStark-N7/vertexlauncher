mod app;
mod assets;
mod screens;
mod ui;
mod window_effects;

fn main() -> eframe::Result<()> {
    match app::maybe_run_webview_helper() {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }

    app::run()
}
