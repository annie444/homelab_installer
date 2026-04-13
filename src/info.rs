use lazy_static::lazy_static;
use std::env;

lazy_static! {
    pub static ref PACKAGE_AUTHORS: String = env!("CARGO_PKG_AUTHORS").to_string();
    pub static ref CRATE_NAME: String = env!("CARGO_CRATE_NAME").to_string();
    pub static ref PACKAGE_NAME: String = env!("CARGO_PKG_NAME").to_string();
    pub static ref PACKAGE_REPO: String = env!("CARGO_PKG_REPOSITORY").to_string();
    pub static ref PACKAGE_VERSION: String = env!("CARGO_PKG_VERSION").to_string();
}
