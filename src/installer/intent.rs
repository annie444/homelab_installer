use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct InstallIntent {
    pub product_profile: ProductProfile,
    pub target_disk: String,
    pub target_image: String, // source imgref
    pub update_image: String, // target imgref
    pub hostname: String,
    pub locale: String,
    pub keymap: String,
    pub timezone: String,
    pub user: Option<UserIntent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum ProductProfile {
    DesktopSecureUefi,
    DesktopUefiNoTpm,
    #[default]
    ServerSecureUefi,
    ServerUefiNoTpm,
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UserIntent {
    pub username: String,
    pub display_name: Option<String>,
    pub password_hash: Option<String>,
    pub ssh_authorized_keys: Vec<String>,
    pub groups: Vec<String>,
    pub make_admin: bool,
}
