use thiserror::Error;

#[derive(Error, Debug)]
pub enum InstallerError {
    #[error("Failed to create TUI: {0}")]
    Create(std::io::Error),

    #[error("Failed to initialize terminal raw mode: {0}")]
    InitRawMode(std::io::Error),

    #[error("Failed to execute the TUI: {0}")]
    InitExec(std::io::Error),

    #[error("Failed to capture the mouse: {0}")]
    InitMouseCapture(std::io::Error),

    #[error("Failed to initialize the clipboard: {0}")]
    InitPaste(std::io::Error),

    #[error("Failed to deinitialize terminal raw mode: {0}")]
    DeinitRawMode(std::io::Error),

    #[error("Failed to deinitialize the TUI: {0}")]
    DeinitExec(std::io::Error),

    #[error("Failed to release the mouse: {0}")]
    DeinitMouseCapture(std::io::Error),

    #[error("Failed to deinitialize the clipboard: {0}")]
    DeinitPaste(std::io::Error),

    #[error("Failed to suspend the application: {0}")]
    Suspend(std::io::Error),

    #[error("Unable to find {0} directory for {1}")]
    Dir(&'static str, String),

    #[error("Failed to initialize D-Bus connection")]
    DbusInit,
}

pub type InstallerResult<T> = color_eyre::eyre::Result<T>;
