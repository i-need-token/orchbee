//! Gives the window launcher the application icon, so the entry that the Start menu shortcut and the
//! uninstall list take their icon from shows Orchbee's. On Windows MSVC targets it writes a compiled
//! resource file, the icon group of `resources/brand/ico/orchbee.ico` and each of its images, into
//! `OUT_DIR` and passes it to the linker, which accepts such a file as input; no resource compiler
//! runs. Other targets get no resource.
use std::{env, fs, path::PathBuf};

const ICON: &str = "../../resources/brand/ico/orchbee.ico";
/// `RT_ICON` and `RT_GROUP_ICON`.
const ICON_TYPE: u16 = 3;
const GROUP_ICON_TYPE: u16 = 14;
/// `MOVEABLE | DISCARDABLE` for an image, and also `PURE` for the group, as the resource compiler
/// writes them.
const ICON_FLAGS: u16 = 0x1010;
const GROUP_FLAGS: u16 = 0x1030;
/// English (United States), the resource compiler's default language.
const LANGUAGE: u16 = 0x0409;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICON}");
    let windows = env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let msvc = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|target| target == "msvc");
    if !(windows && msvc) {
        return;
    }
    let icon = fs::read(ICON).unwrap_or_else(|error| panic!("read {ICON}: {error}"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let resources = out.join("orchbee-desktop-launcher.res");
    fs::write(&resources, compiled_resources(&icon))
        .unwrap_or_else(|error| panic!("write {}: {error}", resources.display()));
    println!("cargo:rustc-link-arg-bins={}", resources.display());
}

/// The compiled resource file for an `.ico` file: the empty leading entry every such file starts
/// with, one `RT_ICON` per image, numbered from 1, and the `RT_GROUP_ICON` 1 that lists them.
fn compiled_resources(icon: &[u8]) -> Vec<u8> {
    let field = |offset: usize, length: usize| -> &[u8] {
        icon.get(offset..offset + length)
            .unwrap_or_else(|| panic!("{ICON} ends before byte {}", offset + length))
    };
    let word = |offset: usize| u16::from_le_bytes(field(offset, 2).try_into().expect("two bytes"));
    let double =
        |offset: usize| u32::from_le_bytes(field(offset, 4).try_into().expect("four bytes"));
    assert!(word(0) == 0 && word(2) == 1, "{ICON} is not an icon file");
    let count = word(4);
    let mut file = Vec::new();
    push_entry(&mut file, 0, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&count.to_le_bytes());
    for index in 0..count {
        let entry = 6 + 16 * usize::from(index);
        let size = double(entry + 8);
        let offset = double(entry + 12);
        let image = field(
            usize::try_from(offset).expect("an offset fits in usize"),
            usize::try_from(size).expect("a size fits in usize"),
        );
        let id = index + 1;
        push_entry(&mut file, ICON_TYPE, id, ICON_FLAGS, image);
        // Width, height, color count, reserved, planes, bit count and size carry over; the image
        // identifier replaces the file offset.
        group.extend_from_slice(field(entry, 12));
        group.extend_from_slice(&id.to_le_bytes());
    }
    push_entry(&mut file, GROUP_ICON_TYPE, 1, GROUP_FLAGS, &group);
    file
}

/// Appends one resource entry with an ordinal type and name: its 32-byte header, its data, and the
/// padding to the next four-byte boundary.
fn push_entry(file: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    let length = u32::try_from(data.len()).expect("a resource fits in four bytes");
    file.extend_from_slice(&length.to_le_bytes());
    file.extend_from_slice(&32_u32.to_le_bytes());
    for ordinal in [kind, name] {
        file.extend_from_slice(&0xFFFF_u16.to_le_bytes());
        file.extend_from_slice(&ordinal.to_le_bytes());
    }
    // Data version, memory flags, language, version and characteristics.
    file.extend_from_slice(&0_u32.to_le_bytes());
    file.extend_from_slice(&flags.to_le_bytes());
    file.extend_from_slice(&(if kind == 0 { 0 } else { LANGUAGE }).to_le_bytes());
    file.extend_from_slice(&0_u32.to_le_bytes());
    file.extend_from_slice(&0_u32.to_le_bytes());
    file.extend_from_slice(data);
    file.resize(file.len().next_multiple_of(4), 0);
}
