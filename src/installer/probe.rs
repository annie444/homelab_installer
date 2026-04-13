use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use dmidecode::bitfield::BitField;
use dmidecode::{EntryPoint, Structure};
use libblkid_rs::BlkidProbe;
use serde::{Deserialize, Serialize};
use tokio::fs;
use udev::{Device, Enumerator};
use zbus::{Connection, Proxy};
use zvariant::OwnedObjectPath;

use crate::error::InstallerResult;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProbeReport {
    pub firmware: FirmwareInfo,
    pub security: SecurityInfo,
    pub disks: DisksInfo,
    pub network: NetworkInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FirmwareInfo {
    pub boot_mode: BootMode, // Uefi | Bios
    pub secure_boot_enabled: bool,
    pub efi_system_partition_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SecurityInfo {
    pub tpm2_present: bool,
    pub tpm2_device: Option<String>, // e.g. /dev/tpmrm0
    pub tpm2_pcr_banks: Vec<String>, // e.g. ["sha256"]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DisksInfo {
    pub devices: Vec<DiskInfo>,
}

impl DisksInfo {
    pub async fn probe() -> InstallerResult<Self> {
        let mut enumerator = Enumerator::new()?;
        enumerator.match_subsystem("block")?;

        let mut flat: Vec<DiskInfo> = Vec::new();

        for dev in enumerator.scan_devices()? {
            // Only keep things with a device node; this drops a lot of irrelevant sysfs-only entries.
            if dev.devnode().is_none() {
                continue;
            }

            let blkid = match dev.devnode() {
                Some(devnode) => BlkidInfo::from_devnode(devnode).unwrap_or_default(),
                None => BlkidInfo::default(),
            };

            flat.push(DiskInfo::from_udev(dev, blkid));
        }

        let devices = build_tree(flat);
        Ok(Self { devices })
    }
}

fn build_tree(flat: Vec<DiskInfo>) -> Vec<DiskInfo> {
    let mut by_sysname: HashMap<String, DiskInfo> =
        flat.into_iter().map(|d| (d.sysname.clone(), d)).collect();

    let child_parent: Vec<(String, String)> = by_sysname
        .values()
        .filter_map(|d| {
            d.parent_sysname
                .as_ref()
                .map(|p| (d.sysname.clone(), p.clone()))
        })
        .collect();

    let mut attached = HashSet::new();

    for (child_name, parent_name) in child_parent {
        let child = match by_sysname.remove(&child_name) {
            Some(c) => c,
            None => continue,
        };

        if let Some(parent) = by_sysname.get_mut(&parent_name) {
            parent.children.push(child);
            attached.insert(child_name);
        } else {
            // Put it back if parent wasn't found.
            by_sysname.insert(child_name, child);
        }
    }

    let mut roots: Vec<_> = by_sysname
        .into_values()
        .filter(|d| !attached.contains(&d.sysname))
        .collect();

    roots.sort_by(|a, b| a.sysname.cmp(&b.sysname));
    sort_children_recursive(&mut roots);
    roots
}

fn sort_children_recursive(nodes: &mut [DiskInfo]) {
    for node in nodes.iter_mut() {
        node.children.sort_by(|a, b| a.sysname.cmp(&b.sysname));
        sort_children_recursive(&mut node.children);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DiskInfo {
    pub sysname: String,          // e.g. "nvme0n1", "nvme0n1p1"
    pub devnode: Option<PathBuf>, // e.g. /dev/nvme0n1
    pub syspath: PathBuf,         // e.g. /sys/devices/...
    pub devtype: Option<String>,  // "disk", "partition", ...
    pub parent_sysname: Option<String>,

    pub major: Option<u32>,
    pub minor: Option<u32>,

    pub bus: Option<String>,                  // ID_BUS
    pub transport: Option<DiskTransportInfo>, // derived
    pub id_path: Option<String>,              // ID_PATH
    pub model: Option<String>,                // ID_MODEL / ID_MODEL_FROM_DATABASE
    pub serial: Option<String>,               // ID_SERIAL_SHORT / ID_SERIAL

    pub fs_type: Option<String>,    // TYPE
    pub fs_uuid: Option<String>,    // UUID
    pub part_uuid: Option<String>,  // PARTUUID when available
    pub part_label: Option<String>, // PARTLABEL when available
    pub label: Option<String>,      // LABEL

    pub is_whole_disk: bool,
    pub is_partition: bool,

    pub children: Vec<DiskInfo>,
}

impl DiskInfo {
    pub fn from_udev(dev: Device, blkid: BlkidInfo) -> Self {
        let sysname = dev.sysname().to_string_lossy().into_owned();
        let syspath = dev.syspath().to_path_buf();
        let devnode = dev.devnode().map(PathBuf::from);
        let devtype = dev.devtype().map(|s| s.to_string_lossy().into_owned());

        let parent_sysname = dev
            .parent()
            .map(|p| p.sysname().to_string_lossy().into_owned());

        let major_minor = dev.devnum().map(split_devnum_linux);

        let bus = prop(&dev, "ID_BUS");
        let id_path = prop(&dev, "ID_PATH");
        let model = prop(&dev, "ID_MODEL_FROM_DATABASE").or_else(|| prop(&dev, "ID_MODEL"));
        let serial = prop(&dev, "ID_SERIAL_SHORT").or_else(|| prop(&dev, "ID_SERIAL"));

        let transport =
            DiskTransportInfo::derive_transport(&dev, bus.as_deref(), id_path.as_deref());

        let is_partition = matches!(devtype.as_deref(), Some("partition"));
        let is_whole_disk = matches!(devtype.as_deref(), Some("disk"));

        Self {
            sysname,
            devnode,
            syspath,
            devtype,
            parent_sysname,

            major: major_minor.map(|(maj, _)| maj),
            minor: major_minor.map(|(_, min)| min),

            bus,
            transport,
            id_path,
            model,
            serial,

            fs_type: blkid.fs_type,
            fs_uuid: blkid.fs_uuid,
            part_uuid: blkid.part_uuid,
            part_label: blkid.part_label,
            label: blkid.label,

            is_whole_disk,
            is_partition,
            children: Vec::new(),
        }
    }
}

fn prop(dev: &Device, key: &str) -> Option<String> {
    dev.property_value(key)
        .map(|v| v.to_string_lossy().into_owned())
}

fn split_devnum_linux(devnum: u64) -> (u32, u32) {
    let major = ((devnum >> 8) & 0xfff) | ((devnum >> 32) & 0xfffff000);
    let minor = (devnum & 0xff) | ((devnum >> 12) & 0xffffff00);
    (major as u32, minor as u32)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BlkidInfo {
    fs_type: Option<String>,
    fs_uuid: Option<String>,
    part_uuid: Option<String>,
    part_label: Option<String>,
    label: Option<String>,
}

impl BlkidInfo {
    pub fn from_devnode<P: AsRef<Path>>(devnode: P) -> InstallerResult<Self> {
        let file = File::open(devnode)?;
        let fd = file.as_raw_fd();

        let mut probe = BlkidProbe::new()?;
        probe.set_device(fd, 0, 0)?;
        probe.enable_superblocks(true)?;
        probe.enable_partitions(true)?;

        let _ = probe.do_safeprobe()?;

        let fs_type = probe.lookup_value("TYPE").ok();
        let fs_uuid = probe.lookup_value("UUID").ok();
        let label = probe.lookup_value("LABEL").ok();

        // PARTUUID / PARTLABEL may be available directly as tags, depending on what blkid found.
        let mut part_uuid = probe.lookup_value("PARTUUID").ok();
        let mut part_label = probe.lookup_value("PARTLABEL").ok();

        // If direct tags are missing, ask the probed partition list.
        if (part_uuid.is_none() || part_label.is_none())
            && let Ok(mut parts) = probe.get_partitions()
        {
            // For a partition device this list is usually about the attached partition context.
            // The exact iterator methods depend on the crate version, so adapt this to the
            // concrete API you have locally.
            let num_parts = parts.number_of_partitions()?;
            if num_parts > 0 {
                for part_idx in 0..num_parts {
                    let part = parts.get_partition_by_partno(part_idx)?;
                    if part_uuid.is_none() {
                        part_uuid = part.get_uuid().ok().flatten().map(|u| u.to_string());
                    }
                    if part_label.is_none() {
                        part_label = part.get_name().ok().flatten();
                    }
                    if part_uuid.is_some() && part_label.is_some() {
                        break;
                    }
                }
            }
        }

        Ok(Self {
            fs_type,
            fs_uuid,
            part_uuid,
            part_label,
            label,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum DiskTransportInfo {
    Nvme,
    #[default]
    Sata,
    Virtio,
    Usb,
    Nfs,
    Scsi,
    Iscsi,
    FibreChannel,
    Unknown(String),
}

impl From<&str> for DiskTransportInfo {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "nvme" => Self::Nvme,
            "ata" | "sata" => Self::Sata,
            "virtio" => Self::Virtio,
            "usb" => Self::Usb,
            "scsi" => Self::Scsi,
            "nfs" | "nfsv1" | "nfsv2" | "nfsv3" => Self::Nfs,
            "iscsi" => Self::Iscsi,
            "fibrechannel" | "fc" => Self::FibreChannel,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl DiskTransportInfo {
    pub fn derive_transport(
        dev: &Device,
        bus: Option<&str>,
        id_path: Option<&str>,
    ) -> Option<Self> {
        if let Some(bus) = bus {
            return Some(bus.into());
        }

        let sysname = dev.sysname().to_string_lossy();

        if sysname.starts_with("nvme") {
            return Some(Self::Nvme);
        }
        if sysname.starts_with("vd") {
            return Some(Self::Virtio);
        }

        if let Some(path) = id_path {
            if path.contains("usb") {
                return Some(Self::Usb);
            }
            if path.contains("nvme") {
                return Some(Self::Nvme);
            }
            if path.contains("virtio") {
                return Some(Self::Virtio);
            }
            if path.contains("ata") || path.contains("sata") {
                return Some(Self::Sata);
            }
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NetworkInfo {
    pub has_link: bool,
    pub active_ifaces: Vec<String>,
}

impl NetworkInfo {
    pub async fn probe() -> InstallerResult<Self> {
        let conn = Connection::system().await?;
        let proxy = Proxy::new(
            &conn,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await?;
        let devices: Vec<OwnedObjectPath> = proxy.call("GetDevices", &()).await?;
        let mut active_ifaces = Vec::new();
        let mut has_link = false;
        for dev in devices {
            let dev_proxy = Proxy::new(
                &conn,
                "org.freedesktop.NetworkManager",
                dev.as_str(),
                "org.freedesktop.NetworkManager.Device",
            )
            .await?;
            let iface: String = dev_proxy.get_property("Interface").await?;
            let state: u32 = dev_proxy.get_property("State").await?;
            if state == 100 {
                active_ifaces.push(iface);
                has_link = true;
            }
        }
        Ok(Self {
            has_link,
            active_ifaces,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum BootMode {
    #[default]
    Uefi,
    Bios,
}

impl BootMode {
    pub async fn probe() -> InstallerResult<Self> {
        if Path::new("/sys/firmware/efi").exists() {
            return Ok(BootMode::Uefi);
        }
        let buf = fs::read("/sys/firmware/dmi/tables/smbios_entry_point").await?;
        let dmi = fs::read("/sys/firmware/dmi/tables/DMI").await?;
        let entry = EntryPoint::search(&buf)?;
        for structure in entry.structures(&dmi) {
            let structure = structure?;
            match structure {
                Structure::Bios(bios) => match bios.bios_characteristics_exttension_2 {
                    Some(ext) if (ext.value() & 0x08) != 0 => {
                        return Ok(BootMode::Uefi);
                    }
                    Some(_) | None => {
                        return Ok(BootMode::Bios);
                    }
                },
                _ => continue,
            };
        }
        Ok(BootMode::Bios)
    }
}
