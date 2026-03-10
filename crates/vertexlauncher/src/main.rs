#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

fn main() -> eframe::Result<()> {
    match app::maybe_run_webview_helper() {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
    match app::maybe_run_cli_command() {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }

    app::run()
}
