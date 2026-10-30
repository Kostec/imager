//! Creating directories and files, checked through `ext4-view` and by reading
//! the on-disk counters by hand. Results are kept for `scripts/fsck-check.sh`.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use classic_ext4::{ClassicExt4, Ext4Error};
use common::{counters, fixture, inode_offset, keep, root_entry, u16_at, u32_at, view};

const DROPIN_DIR: &str = "/etc/systemd/system/armbian-firstrun.service.d";

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 + i / 4096) as u8).collect()
}

#[test]
fn creates_dir_and_files_with_mode_owner_and_content() {
    let fx = fixture();
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    fs4.mkdir(DROPIN_DIR, 0o755).unwrap();
    let files: [(&str, u16, Vec<u8>); 4] = [
        (
            "10-wukong.conf",
            0o644,
            b"[Unit]\nBefore=ssh.service\n".to_vec(),
        ),
        ("firstboot.sh", 0o755, b"#!/bin/bash\n".to_vec()),
        ("firstboot.conf", 0o600, b"WK_HOSTNAME='wukong'\n".to_vec()),
        ("user-script.sh", 0o700, pattern(9000)),
    ];
    for (name, mode, data) in &files {
        fs4.write_new_file(&format!("{DROPIN_DIR}/{name}"), data, *mode)
            .unwrap();
    }
    fs4.commit().unwrap();
    keep(&fx.path, "dropin");

    let ext = view(&fx.path);
    let dir = ext.metadata(DROPIN_DIR).unwrap();
    assert!(dir.is_dir());
    assert_eq!(dir.mode() & 0o7777, 0o755);
    assert_eq!((dir.uid(), dir.gid()), (0, 0));
    for (name, mode, data) in &files {
        let path = format!("{DROPIN_DIR}/{name}");
        let meta = ext.metadata(path.as_str()).unwrap();
        assert_eq!(meta.mode() & 0o7777, *mode, "{name}");
        assert!(meta.file_type().is_regular_file(), "{name}");
        assert_eq!((meta.uid(), meta.gid()), (0, 0), "{name}");
        assert_eq!(&ext.read(path.as_str()).unwrap(), data, "{name}");
    }
}

#[test]
fn new_inodes_have_current_times_and_parent_is_touched() {
    let fx = fixture();
    let unix_now = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32
    };
    let before = unix_now();
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    fs4.write_new_file("/stamped", b"x", 0o644).unwrap();
    fs4.commit().unwrap();
    let after = unix_now();

    let img = fs::read(&fx.path).unwrap();
    let inode = inode_offset(&img, root_entry(&img, "stamped"));
    for (field, off) in [
        ("atime", 0x08),
        ("ctime", 0x0C),
        ("mtime", 0x10),
        ("crtime", 0x90),
    ] {
        let t = u32_at(&img, inode + off);
        assert!((before..=after).contains(&t), "{field}={t}");
    }
    assert_eq!(u32_at(&img, inode + 0x14), 0, "dtime");
    assert_eq!(u16_at(&img, inode + 0x1A), 1, "links");
    assert_eq!(
        u32_at(&img, inode + 0x1C),
        8,
        "i_blocks counts 512-byte sectors"
    );
    let root = inode_offset(&img, 2);
    assert!(
        (before..=after).contains(&u32_at(&img, root + 0x10)),
        "parent mtime"
    );
}

#[test]
fn files_of_several_blocks_read_back() {
    let fx = fixture();
    let sizes = [0, 1, 4095, 4096, 4097, 65536, 1024 * 1024];
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    for size in sizes {
        fs4.write_new_file(&format!("/size-{size}"), &pattern(size), 0o644)
            .unwrap();
    }
    fs4.commit().unwrap();
    keep(&fx.path, "sizes");

    let ext = view(&fx.path);
    for size in sizes {
        assert_eq!(
            ext.read(format!("/size-{size}").as_str()).unwrap(),
            pattern(size)
        );
    }
}

#[test]
fn counters_follow_the_allocation() {
    let fx = fixture();
    let start = counters(&fx.path);
    let root = |p: &std::path::Path| {
        let img = fs::read(p).unwrap();
        u16_at(&img, inode_offset(&img, 2) + 0x1A)
    };
    let root_links = root(&fx.path);

    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    fs4.mkdir("/one", 0o755).unwrap(); // 1 block, 1 inode
    fs4.mkdir("/one/two", 0o750).unwrap(); // 1 block, 1 inode
    fs4.write_new_file("/one/f", &pattern(3 * 4096 + 1), 0o644)
        .unwrap(); // 4 blocks, 1 inode
    fs4.write_new_file("/one/empty", b"", 0o644).unwrap(); // 0 blocks, 1 inode
    fs4.commit().unwrap();
    keep(&fx.path, "counters");

    let end = counters(&fx.path);
    assert_eq!(end.gd_free_blocks, start.gd_free_blocks - 6);
    assert_eq!(end.gd_free_inodes, start.gd_free_inodes - 4);
    assert_eq!(end.sb_free_blocks, end.gd_free_blocks);
    assert_eq!(end.sb_free_inodes, end.gd_free_inodes);
    assert_eq!(end.used_dirs, start.used_dirs + 2);
    assert_eq!(root(&fx.path), root_links + 1, "/one's '..' adds a link");

    let img = fs::read(&fx.path).unwrap();
    let one = inode_offset(&img, root_entry(&img, "one"));
    assert_eq!(u16_at(&img, one + 0x1A), 3, "/one: entry, '.', two/..");
}

#[test]
fn directory_grows_when_its_block_is_full() {
    let fx = fixture();
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    fs4.mkdir("/many", 0o755).unwrap();
    let names: BTreeSet<String> = (0..600)
        .map(|i| format!("entry-with-a-reasonably-long-name-{i:04}"))
        .collect();
    for name in &names {
        fs4.write_new_file(&format!("/many/{name}"), name.as_bytes(), 0o644)
            .unwrap();
    }
    assert!(fs4
        .is_regular_file("/many/entry-with-a-reasonably-long-name-0599")
        .unwrap());
    fs4.commit().unwrap();
    keep(&fx.path, "grown");

    let ext = view(&fx.path);
    let mut seen = BTreeSet::new();
    for entry in ext.read_dir("/many").unwrap() {
        let name = entry.unwrap().file_name().as_str().unwrap().to_owned();
        if name != "." && name != ".." {
            seen.insert(name);
        }
    }
    assert_eq!(seen, names);
    for name in &names {
        assert_eq!(
            ext.read(format!("/many/{name}").as_str()).unwrap(),
            name.as_bytes()
        );
    }
    let img = fs::read(&fx.path).unwrap();
    let many = inode_offset(&img, root_entry(&img, "many"));
    let size = u32_at(&img, many + 0x04);
    assert!(size >= 3 * 4096 && size % 4096 == 0, "i_size={size}");
    let depth = u16_at(&img, many + 0x28 + 6);
    assert_eq!(depth, 1, "this many fragments overflow the inode");
    assert_eq!(
        u32_at(&img, many + 0x1C),
        size / 512 + depth * 8,
        "i_blocks counts content and the extent leaf"
    );
}

#[test]
fn a_big_file_spans_block_groups() {
    let fx = fixture();
    let data = pattern(40 * 1024 * 1024);
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    fs4.write_new_file("/big", &data, 0o644).unwrap();
    fs4.commit().unwrap();
    keep(&fx.path, "spanning");
    assert!(view(&fx.path).read("/big").unwrap() == data);
}

#[test]
fn running_out_of_space_is_reported_and_leaves_the_handle_usable() {
    let fx = fixture();
    let start = counters(&fx.path);
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    assert!(matches!(
        fs4.write_new_file("/huge", &pattern(100 * 1024 * 1024), 0o644),
        Err(Ext4Error::NoSpace)
    ));
    assert!(!fs4.exists("/huge").unwrap());
    fs4.write_new_file("/small", b"fits", 0o644).unwrap();
    fs4.commit().unwrap();
    keep(&fx.path, "nospace");

    assert_eq!(counters(&fx.path).gd_free_blocks, start.gd_free_blocks - 1);
    assert_eq!(view(&fx.path).read("/small").unwrap(), b"fits");
}

#[test]
fn existing_files_stay_intact() {
    let fx = fixture();
    let service = "/usr/lib/systemd/system/armbian-firstrun.service";
    let before = view(&fx.path);
    let hello = before.read("/hello.txt").unwrap();
    let blob = before.read("/data/blob.bin").unwrap();
    let unit = before.read(service).unwrap();

    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    fs4.mkdir(DROPIN_DIR, 0o755).unwrap();
    fs4.write_new_file(&format!("{DROPIN_DIR}/a"), &pattern(70000), 0o644)
        .unwrap();
    fs4.commit().unwrap();

    let after = view(&fx.path);
    assert_eq!(after.read("/hello.txt").unwrap(), hello);
    assert_eq!(after.read("/data/blob.bin").unwrap(), blob);
    assert_eq!(after.read(service).unwrap(), unit);
    let link = after
        .read_link("/etc/systemd/system/multi-user.target.wants/armbian-firstrun.service")
        .unwrap();
    assert_eq!(
        link.to_str().unwrap(),
        "/lib/systemd/system/armbian-firstrun.service"
    );
}

#[test]
fn queries_are_lstat_like() {
    let fx = fixture();
    let fs4 = ClassicExt4::open_read_only(&fx.path).unwrap();
    let wants = "/etc/systemd/system/multi-user.target.wants/armbian-firstrun.service";
    assert!(fs4.exists("/").unwrap());
    assert!(fs4.exists("/hello.txt").unwrap());
    assert!(fs4.is_regular_file("/hello.txt").unwrap());
    assert!(fs4.exists("/etc").unwrap());
    assert!(!fs4.is_regular_file("/etc").unwrap());
    assert!(fs4.exists(wants).unwrap(), "a symlink exists");
    assert!(
        !fs4.is_regular_file(wants).unwrap(),
        "a symlink is not followed"
    );
    assert!(!fs4.exists("/nope").unwrap());
    assert!(!fs4.exists("/nope/deeper").unwrap());
    assert!(!fs4.exists("/hello.txt/deeper").unwrap());
    assert!(!fs4.is_regular_file("/nope").unwrap());
    assert!(fs4
        .is_regular_file("/usr/lib/systemd/system/armbian-firstrun.service")
        .unwrap());
    assert!(fs4
        .is_regular_file("/htree/a-rather-long-file-name-to-fill-blocks-399")
        .unwrap());
    assert!(matches!(
        fs4.exists("/lib/systemd/system/armbian-firstrun.service"),
        Err(Ext4Error::Unsupported(_))
    ));
}

#[test]
fn refusals_for_paths() {
    let fx = fixture();
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    let is = |r: Result<(), Ext4Error>| format!("{:?}", r.unwrap_err());
    assert!(is(fs4.mkdir("/etc", 0o755)).starts_with("AlreadyExists"));
    assert!(is(fs4.write_new_file("/hello.txt", b"", 0o644)).starts_with("AlreadyExists"));
    assert!(is(fs4.write_new_file("/etc", b"", 0o644)).starts_with("AlreadyExists"));
    assert!(is(fs4.mkdir("/", 0o755)).starts_with("AlreadyExists"));
    assert!(is(fs4.mkdir("/nope/x", 0o755)).starts_with("NotFound"));
    assert!(is(fs4.write_new_file("/nope/x", b"", 0o644)).starts_with("NotFound"));
    assert!(is(fs4.write_new_file("/hello.txt/x", b"", 0o644)).starts_with("NotFound"));
    assert!(is(fs4.write_new_file("relative", b"", 0o644)).starts_with("Unsupported"));
    assert!(is(fs4.write_new_file("/a/../b", b"", 0o644)).starts_with("Unsupported"));
    assert!(is(fs4.write_new_file("/lib/x", b"", 0o644)).starts_with("Unsupported"));
    assert!(is(fs4.write_new_file("/mode", b"", 0o100644)).starts_with("Unsupported"));
    let long = format!("/{}", "n".repeat(256));
    assert!(is(fs4.write_new_file(&long, b"", 0o644)).starts_with("Unsupported"));
    let exact = format!("/{}", "n".repeat(255));
    fs4.write_new_file(&exact, b"", 0o644).unwrap();
}

#[test]
fn an_indexed_parent_is_refused() {
    let fx = fixture();
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    assert!(matches!(
        fs4.write_new_file("/htree/new", b"x", 0o644),
        Err(Ext4Error::Unsupported(m)) if m.contains("index")
    ));
    assert!(matches!(
        fs4.mkdir("/htree/new", 0o755),
        Err(Ext4Error::Unsupported(_))
    ));
    fs4.mkdir("/usr/lib/systemd/system/x", 0o755)
        .expect("the fixture's small directory has no index");
}

#[test]
fn a_read_only_handle_refuses_changes() {
    let fx = fixture();
    let before = fs::read(&fx.path).unwrap();
    let mut fs4 = ClassicExt4::open_read_only(&fx.path).unwrap();
    assert!(matches!(
        fs4.mkdir("/x", 0o755),
        Err(Ext4Error::Unsupported(_))
    ));
    assert!(matches!(
        fs4.write_new_file("/x", b"", 0o644),
        Err(Ext4Error::Unsupported(_))
    ));
    fs4.commit().unwrap();
    assert!(fs::read(&fx.path).unwrap() == before);
}

#[test]
fn nothing_reaches_the_file_before_commit() {
    let fx = fixture();
    let before = fs::read(&fx.path).unwrap();
    {
        let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
        fs4.mkdir("/pending", 0o755).unwrap();
        fs4.write_new_file("/pending/f", &pattern(50000), 0o644)
            .unwrap();
        assert!(fs4.is_regular_file("/pending/f").unwrap());
        assert!(
            fs::read(&fx.path).unwrap() == before,
            "changed before commit"
        );
    }
    assert!(fs::read(&fx.path).unwrap() == before, "changed by drop");
}

#[test]
fn a_failed_operation_leaves_no_trace_in_the_commit() {
    let fx = fixture();
    let start = counters(&fx.path);
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    assert!(fs4
        .write_new_file("/hello.txt", &pattern(5000), 0o644)
        .is_err());
    assert!(fs4
        .write_new_file("/huge", &pattern(100 * 1024 * 1024), 0o644)
        .is_err());
    fs4.mkdir("/kept", 0o755).unwrap();
    fs4.commit().unwrap();
    let end = counters(&fx.path);
    assert_eq!(end.gd_free_blocks, start.gd_free_blocks - 1);
    assert_eq!(end.gd_free_inodes, start.gd_free_inodes - 1);
}
