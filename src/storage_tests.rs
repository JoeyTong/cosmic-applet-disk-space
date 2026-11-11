use super::*;
use serde_json::{Value, json};

fn partition(name: &str, uuid: &str, mounts: &[&str], used: u64, total: u64) -> Value {
    json!({
        "name": name, "path": format!("/dev/{name}"), "type": "part",
        "fstype": "ext4", "uuid": uuid, "mountpoints": mounts,
        "fsused": used, "fssize": total, "fsavail": total - used
    })
}

fn disk(name: &str, children: Vec<Value>) -> Value {
    json!({
        "name": name, "kname": name, "path": format!("/dev/{name}"),
        "type": "disk", "rota": false, "rm": false,
        "size": 2000, "children": children
    })
}

fn parse(devices: Vec<Value>) -> Vec<Drive> {
    parse_inventory(
        &serde_json::to_vec(&json!({ "blockdevices": devices })).unwrap(),
        |_| HardwareInfo::default(),
    )
    .unwrap()
}

#[test]
fn btrfs_subvolumes_and_duplicate_mounts_count_once() {
    let mut btrfs = partition(
        "nvme0n1p2",
        "btrfs-fsid",
        &["/", "/home", "/var/log", "/home"],
        300,
        1000,
    );
    btrfs["fstype"] = json!("btrfs");
    let drives = parse(vec![disk(
        "nvme0n1",
        vec![
            partition("nvme0n1p1", "boot", &["/boot"], 20, 100),
            btrfs.clone(),
            btrfs,
        ],
    )]);
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].filesystems.len(), 2);
    assert_eq!(drives[0].usage.unwrap().used_bytes, 320);
    assert_eq!(drives[0].usage.unwrap().total_bytes, 1100);
    assert!(!drives[0].shared);
    assert_eq!(
        drives[0]
            .filesystems
            .iter()
            .find(|fs| fs.filesystem_type == "btrfs")
            .unwrap()
            .mountpoints
            .len(),
        3
    );
}

#[test]
fn aggregates_only_mounted_capacity_not_raw_or_unmounted_partitions() {
    let drives = parse(vec![disk(
        "sda",
        vec![
            partition("sda1", "one", &["/"], 80, 100),
            partition("sda2", "two", &["/home"], 10, 100),
            partition("sda3", "three", &[], 500, 1000),
        ],
    )]);
    let drive = &drives[0];
    assert_eq!(drive.raw_capacity_bytes, 2000);
    assert_eq!(drive.usage.unwrap().total_bytes, 200);
    assert_eq!(drive.usage.unwrap().used_bytes, 90);
    assert!(!drive.warning());
    assert_eq!(drive.status, DriveStatus::Mounted);
}

#[test]
fn warning_boundary_is_exact_and_does_not_overflow() {
    let usage = |used, total| Usage {
        used_bytes: used,
        total_bytes: total,
        available_bytes: None,
    };
    assert!(!usage(7999, 10000).warning());
    assert_eq!(usage(7999, 10000).percent_label(), "79.9%");
    assert!(usage(8000, 10000).warning());
    assert!(usage(100, 100).warning());
    assert!(!usage(0, 0).warning());
    assert!(usage(u64::MAX, u64::MAX).warning());
}

#[test]
fn keeps_unmounted_locked_and_unreadable_drives_without_zero_usage() {
    let mut locked = partition("sdb1", "luks", &[], 0, 100);
    locked["fstype"] = json!("crypto_LUKS");
    let mut unreadable = partition("sdc1", "data", &["/private"], 0, 100);
    unreadable["fsused"] = Value::Null;
    let drives = parse(vec![
        disk("sda", vec![]),
        disk("sdb", vec![locked]),
        disk("sdc", vec![unreadable]),
    ]);
    assert_eq!(
        drives.iter().map(|drive| drive.status).collect::<Vec<_>>(),
        [
            DriveStatus::Unmounted,
            DriveStatus::Locked,
            DriveStatus::Unavailable
        ]
    );
    assert!(
        drives
            .iter()
            .all(|drive| drive.usage.is_none() && !drive.warning())
    );
}

#[test]
fn follows_unlocked_luks_and_lvm_layers() {
    let fs = partition("vg-root", "root", &["/"], 850, 1000);
    let drives = parse(vec![disk(
        "sda",
        vec![json!({
            "name": "sda1", "path": "/dev/sda1", "fstype": "crypto_LUKS",
            "children": [{ "name": "cryptroot", "path": "/dev/mapper/cryptroot",
                "type": "crypt", "fstype": "LVM2_member", "children": [fs] }]
        })],
    )]);
    assert_eq!(drives[0].status, DriveStatus::Mounted);
    assert_eq!(drives[0].filesystems.len(), 1);
    assert!(drives[0].warning());
}

#[test]
fn shared_lvm_or_raid_usage_is_counted_once_per_physical_owner() {
    let mut shared = partition("md0", "raid-fs", &["/data"], 800, 1000);
    shared["type"] = json!("raid1");
    shared["maj:min"] = json!("9:0");
    let drives = parse(vec![
        disk("sda", vec![shared.clone(), shared.clone()]),
        disk("sdb", vec![shared]),
    ]);
    assert_eq!(drives.len(), 2);
    for drive in drives {
        assert!(drive.shared && drive.warning());
        assert_eq!(drive.usage.unwrap().total_bytes, 1000);
        assert_eq!(
            drive.filesystems[0].physical_drives,
            ["/dev/sda", "/dev/sdb"]
        );
        assert!(drive.panel_summary().contains("Shared"));
    }
}

#[test]
fn btrfs_secondary_member_inherits_shared_mount_usage() {
    let mut mounted = partition("sda1", "pool", &["/data"], 900, 1500);
    mounted["fstype"] = json!("btrfs");
    let mut member = partition("sdb1", "pool", &[], 0, 1500);
    member["fstype"] = json!("btrfs");
    let drives = parse(vec![disk("sda", vec![mounted]), disk("sdb", vec![member])]);
    for drive in drives {
        assert!(drive.shared);
        assert_eq!(drive.status, DriveStatus::Mounted);
        assert_eq!(drive.usage.unwrap().used_bytes, 900);
    }
}

#[test]
fn cloned_non_btrfs_uuids_do_not_merge_independent_devices() {
    let drives = parse(vec![
        disk("sda", vec![partition("sda1", "cloned", &["/one"], 10, 100)]),
        disk("sdb", vec![partition("sdb1", "cloned", &["/two"], 90, 100)]),
    ]);
    assert_eq!(drives[0].usage.unwrap().used_bytes, 10);
    assert_eq!(drives[1].usage.unwrap().used_bytes, 90);
    assert!(drives.iter().all(|drive| !drive.shared));
}

#[test]
fn partial_data_is_visible_instead_of_presented_as_complete() {
    let mut unreadable = partition("sda2", "private", &["/private"], 0, 100);
    unreadable["fssize"] = Value::Null;
    let drives = parse(vec![disk(
        "sda",
        vec![partition("sda1", "root", &["/"], 80, 100), unreadable],
    )]);
    assert_eq!(drives[0].status, DriveStatus::Partial);
    assert!(drives[0].panel_summary().contains("Partial"));
    assert_eq!(drives[0].usage.unwrap().total_bytes, 100);
}

#[test]
fn excludes_ram_loop_network_optical_and_mmc_boot_devices() {
    let mut optical = disk("sr0", vec![]);
    optical["type"] = json!("rom");
    let mut network = disk("sdc", vec![]);
    network["tran"] = json!("iscsi");
    let drives = parse(vec![
        disk("zram0", vec![]),
        disk("loop0", vec![]),
        disk("ram0", vec![]),
        disk("nbd0", vec![]),
        disk("mmcblk0boot0", vec![]),
        optical,
        network,
        disk("nvme0n1", vec![]),
    ]);
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].name, "nvme0n1");
}

#[test]
fn all_device_icon_categories_are_distinct_and_hotplug_sata_is_internal() {
    let mut device = BlockDevice {
        name: "sda".into(),
        rota: Some(false),
        ..Default::default()
    };
    let hardware = HardwareInfo::default();
    assert_eq!(classify(&device, &hardware), DriveKind::InternalSsd);
    device.rota = Some(true);
    device.hotplug = Some(true); // A hot-swappable SATA bay is not necessarily external.
    assert_eq!(classify(&device, &hardware), DriveKind::InternalHdd);
    device.tran = Some("usb".into());
    assert_eq!(classify(&device, &hardware), DriveKind::ExternalHdd);
    device.rota = Some(false);
    assert_eq!(classify(&device, &hardware), DriveKind::ExternalSsd);
    device.rm = Some(true);
    assert_eq!(classify(&device, &hardware), DriveKind::UsbFlash);
    let mut hardware = HardwareInfo::default();
    hardware
        .properties
        .insert("ID_DRIVE_FLASH_SD".into(), "1".into());
    assert_eq!(classify(&device, &hardware), DriveKind::SdCard);
    assert_eq!(
        classify(&BlockDevice::default(), &HardwareInfo::default()),
        DriveKind::Unknown
    );
}

#[test]
fn usb_thumb_hint_is_used_even_when_removable_flag_is_false() {
    let device = BlockDevice {
        tran: Some("usb".into()),
        rota: Some(false),
        rm: Some(false),
        ..Default::default()
    };
    let hardware = HardwareInfo {
        properties: [("ID_DRIVE_THUMB".into(), "1".into())].into(),
        ..Default::default()
    };
    assert_eq!(classify(&device, &hardware), DriveKind::UsbFlash);
}

#[test]
fn accepts_old_lsblk_string_numbers_and_boolean_flags() {
    let mut device = disk("sda", vec![]);
    device["rota"] = json!("1");
    device["rm"] = json!(0);
    device["size"] = json!("4096");
    let drives = parse(vec![device]);
    assert_eq!(drives[0].raw_capacity_bytes, 4096);
    assert_eq!(drives[0].kind, DriveKind::InternalHdd);
}

#[test]
fn hot_unplug_removes_device_on_next_snapshot_and_replug_restores_it() {
    let drive = disk(
        "sda",
        vec![partition("sda1", "usb", &["/media/USB"], 10, 100)],
    );
    assert_eq!(parse(vec![drive.clone()]).len(), 1);
    assert!(parse(vec![]).is_empty());
    assert_eq!(parse(vec![drive])[0].usage.unwrap().used_bytes, 10);
}

#[test]
fn malformed_output_is_an_error_and_impossible_stats_are_unavailable() {
    assert!(parse_inventory(b"not JSON", |_| HardwareInfo::default()).is_err());
    assert!(parse_inventory(b"{}", |_| HardwareInfo::default()).is_err());
    let mut fs = partition("sda1", "root", &["/"], 10, 100);
    fs["fsused"] = json!(101);
    assert_eq!(
        parse(vec![disk("sda", vec![fs])])[0].status,
        DriveStatus::Unavailable
    );
}

#[test]
fn panel_capacity_keeps_one_decimal_below_ten() {
    const GIB: u64 = 1024 * 1024 * 1024;
    assert_eq!(format_panel_amount(0.0), "0.0");
    assert_eq!(format_panel_amount(0.348), "0.3");
    assert_eq!(format_panel_amount(9.94), "9.9");
    assert_eq!(format_panel_amount(9.96), "10");
    assert_eq!(format_panel_amount(10.0), "10");
    assert_eq!(format_panel_amount(18.188), "18");
    assert_eq!(
        panel_usage(36 * GIB, 477 * GIB).panel_capacity(),
        "36 / 477 G"
    );
    // Hundreds of GiB on multi-terabyte drives must not collapse to 0.
    assert_eq!(
        panel_usage(449_120_399_360, 3_998_832_615_424).panel_capacity(),
        "0.4 / 3.6 T"
    );
    assert_eq!(
        panel_usage(382_954_582_016, 19_998_449_860_608).panel_capacity(),
        "0.3 / 18 T"
    );
    // Only the side below 10 gains a decimal.
    assert_eq!(
        panel_usage(15 * GIB, 10_200_547_328).panel_capacity(),
        "15 / 9.5 G"
    );
}

fn panel_usage(used: u64, total: u64) -> Usage {
    Usage {
        used_bytes: used,
        total_bytes: total,
        available_bytes: None,
    }
}

#[test]
fn sysfs_virtual_or_remote_disks_are_excluded() {
    let input = serde_json::to_vec(&json!({ "blockdevices": [disk("sda", vec![])] })).unwrap();
    assert!(
        parse_inventory(&input, |_| HardwareInfo {
            virtual_device: true,
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
    assert!(
        parse_inventory(&input, |_| HardwareInfo {
            remote: true,
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
}
