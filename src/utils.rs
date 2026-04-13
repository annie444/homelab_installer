use std::{env, fs, path::PathBuf};

use directories::ProjectDirs;
use lazy_static::lazy_static;
use tracing::debug;
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    self, EnvFilter, Layer, fmt, layer::SubscriberExt, registry, util::SubscriberInitExt,
};

use crate::error::{InstallerError, InstallerResult};

lazy_static! {
    pub static ref PROJECT_NAME: String =
        crate::info::CRATE_NAME.as_str().to_uppercase().to_string();
    pub static ref DATA_FOLDER: Option<PathBuf> =
        std::env::var(format!("{}_DATA", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
    pub static ref CONFIG_FOLDER: Option<PathBuf> =
        std::env::var(format!("{}_CONFIG", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
    pub static ref LOG_ENV: String = format!("{}_LOGLEVEL", PROJECT_NAME.clone());
    pub static ref LOG_FILE: String = format!("{}.log", crate::info::PACKAGE_NAME.clone());
}

fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "annieehler", env!("CARGO_PKG_NAME"))
}

pub fn get_data_dir() -> InstallerResult<PathBuf> {
    let directory = if let Some(s) = DATA_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        return Err(InstallerError::Dir("data", PROJECT_NAME.to_string()))?;
    };
    Ok(directory)
}

pub fn get_config_dir() -> InstallerResult<PathBuf> {
    let directory = if let Some(s) = CONFIG_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        return Err(InstallerError::Dir("config", PROJECT_NAME.to_string()))?;
    };
    Ok(directory)
}

pub fn initialize_logging() -> InstallerResult<()> {
    let directory = get_data_dir()?;
    fs::create_dir_all(directory.clone())?;

    let log_path = directory.join(LOG_FILE.clone());
    dbg!(&log_path);
    let log_file = fs::File::create(log_path)?;
    let log_filter = env::var("RUST_LOG")
        .or_else(|_| env::var(LOG_ENV.clone()))
        .unwrap_or_else(|_| format!("{}=info", env!("CARGO_CRATE_NAME")));
    dbg!(&log_filter);
    let file_subscriber = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false)
        .with_filter(EnvFilter::builder().parse_lossy(log_filter.as_str()));
    registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .init();
    debug!(filter = %log_filter, "Initialized logging");
    Ok(())
}

/// Similar to the `std::dbg!` macro, but generates `tracing` events rather
/// than printing to stdout.
///
/// By default, the verbosity level for the generated events is `DEBUG`, but
/// this can be customized.
macro_rules! dbg {
    (target: $target:expr, level: $level:expr, $ex:expr) => {{
        match $ex {
            value => {
                tracing::event!(target: $target, $level, ?value, stringify!($ex));
                value
            }
        }
    }};
    (level: $level:expr, $ex:expr) => {
        crate::utils::dbg!(target: module_path!(), level: $level, $ex)
    };
    (target: $target:expr, $ex:expr) => {
        crate::utils::dbg!(target: $target, level: tracing::Level::DEBUG, $ex)
    };
    ($ex:expr) => {
        crate::utils::dbg!(level: tracing::Level::DEBUG, $ex)
    };
}
