use std::panic::set_hook;
use std::process::exit;

#[cfg(debug_assertions)]
use better_panic::Settings;
use color_eyre::config::HookBuilder;
#[cfg(not(debug_assertions))]
use human_panic::{Metadata, handle_dump, print_msg};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tracing::error;

use crate::error::InstallerResult;
use crate::tui::Tui;

/// helper function to create a horizontally centered rect
/// using up certain percentage of the available rect `r`
pub fn center_vertical(percent_y: u16, r: Rect) -> Rect {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r)[1]
}

/// helper function to create a vertically centered rect
/// using up certain percentage of the available rect `r`
pub fn center_horizontal(percent_x: u16, r: Rect) -> Rect {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(r)[1]
}

/// helper function to create a centered rect using up certain percentage
/// of the available rect `r`
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    center_horizontal(percent_x, center_vertical(percent_y, r))
}

pub fn initialize_panic_handler() -> InstallerResult<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default()
        .panic_section(format!(
            "This is a bug. Consider reporting it at {}",
            crate::info::PACKAGE_REPO.as_str()
        ))
        .display_location_section(true)
        .display_env_section(true)
        .into_hooks();
    eyre_hook.install()?;
    set_hook(Box::new(move |panic_info| {
        if let Ok(mut t) = Tui::new()
            && let Err(r) = t.exit()
        {
            error!("Unable to exit Terminal: {r:?}");
        }

        let msg = format!("{}", panic_hook.panic_report(panic_info));
        #[cfg(not(debug_assertions))]
        {
            eprintln!("{msg}");
            let author = format!("authored by {}", crate::info::PACKAGE_AUTHORS.as_str());
            let support = format!(
                "You can open a support request at {}",
                crate::info::PACKAGE_REPO.as_str()
            );
            let meta = Metadata::new(
                crate::info::PACKAGE_NAME.clone(),
                crate::info::PACKAGE_VERSION.clone(),
            )
            .authors(author)
            .support(support);

            let file_path = handle_dump(&meta, panic_info);
            print_msg(file_path, &meta)
                .expect("human-panic: printing error message to console failed");
        }
        error!("Error: {}", strip_ansi_escapes::strip_str(msg));

        #[cfg(debug_assertions)]
        {
            // Better Panic stacktrace that is only enabled when debugging.
            Settings::auto()
                .most_recent_first(false)
                .lineno_suffix(true)
                .verbosity(better_panic::Verbosity::Full)
                .create_panic_handler()(panic_info);
        }

        exit(libc::EXIT_FAILURE);
    }));
    Ok(())
}
