use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedInstallPlan {
    pub schema_version: String,
    pub plan_id: String,

    pub probe: super::probe::ProbeReport,
    pub intent: super::intent::InstallIntent,

    pub storage: StoragePlan,
    pub deploy: BootcDeployPlan,
    pub offline_seed: OfflineSeedPlan,
    pub first_boot: FirstBootPlan,

    pub validation: ValidationPlan,
}

impl ResolvedInstallPlan {
    pub fn new() -> Self {
        Self {
            schema_version: super::SCHEMA_VERSION.to_string(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct StoragePlan {
    pub disk: String,
    pub partition_table: PartitionTableKind,
    pub root_discovery: RootDiscovery,
    pub bootloader: BootloaderPlan,
    pub partitions: Vec<PartitionPlan>,
    pub encryption: EncryptionPlan,
    pub mount_plan: Vec<MountPlan>,
    pub destructive_wipe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum PartitionTableKind {
    #[default]
    Gpt,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum RootDiscovery {
    Uuid, // bootc gets root=UUID=...
    #[default]
    Dps, // omit root= and rely on Discoverable Partitions + BLI
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct BootloaderPlan {
    pub kind: BootloaderKind,
    pub efi_partition: String, // partition logical name
    pub xbootldr_partition: Option<String>,
    pub install_bootloader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum BootloaderKind {
    #[default]
    SystemdBoot,
    Grub,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PartitionPlan {
    pub name: String,
    pub purpose: PartitionPurpose,
    pub fs_type: Option<FsType>, // vfat, ext4, xfs, btrfs
    pub size_mib: Option<u64>,   // None => grow remainder
    pub grow: bool,
    pub luks_device_name: Option<String>, // if encrypted
    pub type_guid_hint: Option<String>,   // DPS / ESP / XBOOTLDR GUIDs
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum FsType {
    Vfat,
    Ext4,
    #[default]
    Xfs,
    Btrfs,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum PartitionPurpose {
    Esp,
    XBootldr,
    Boot,
    #[default]
    Root,
    Var,
    Home,
    Data,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum EncryptionPlan {
    #[default]
    None,
    Luks2Tpm2 {
        encrypted_partition: String, // usually root or var
        pcrs: Vec<u8>,               // e.g. [7]
        pin: bool,
        recovery_key: bool,
        separate_boot_required: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct MountPlan {
    pub source: String, // UUID=... or /dev/mapper/...
    pub target: String, // /target, /target/boot, /target/boot/efi
    pub fs_type: String,
    pub mount_options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct BootcDeployPlan {
    pub source_imgref: String,
    pub target_imgref: String,
    pub stateroot: String,
    pub root_mount_spec: Option<String>, // Some("UUID=...") or Some("") for DPS
    pub boot_mount_spec: Option<String>, // e.g. UUID=...
    pub kernel_args: Vec<String>,
    pub run_finalize: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct OfflineSeedPlan {
    pub deployment_path_strategy: DeploymentPathStrategy,
    pub write_hostname: bool,
    pub write_locale: bool,
    pub write_keymap: bool,
    pub write_timezone: bool,
    pub seed_answer_file: bool,
    pub answer_file_path: String, // e.g. /etc/myos/install-answer.json
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum DeploymentPathStrategy {
    #[default]
    ResolveViaOstreeAdmin,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FirstBootPlan {
    pub service_name: String, // e.g. myos-firstboot.service
    pub create_human_user: bool,
    pub consume_answer_file: bool,
    pub delete_answer_file_after_success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ValidationPlan {
    pub require_uefi: bool,
    pub require_secure_boot: bool,
    pub require_tpm2: bool,
    pub permit_fallback_to_unencrypted: bool,
    pub confirm_disk_wipe: bool,
}
