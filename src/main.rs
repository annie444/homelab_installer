mod error;
mod info;
mod installer;
mod tui;
mod utils;

#[tokio::main]
async fn main() -> error::InstallerResult<()> {
    utils::initialize_logging()?;
    tui::utils::initialize_panic_handler()?;

    let mut app = tui::app::App::new();
    app.run().await?;
    Ok(())
}
