use std::{error::Error, io};

#[cfg(unix)]
use nix::unistd::{Uid, User};
use orchbee_capability_node_protocol::AbsoluteCwd;
use orchbee_terminal_protocol::{
    ColorScheme, CreationId, CursorShape, DeviceAttributes, LaunchEnvironmentEntry, TerminalProfile,
};

use crate::{
    default_interactive_launch::{default_launch_environment, resolve_default_interactive_launch},
    terminal_host_launch_adapter::ResolvedTerminalLaunchFacts,
    worker_identity::fresh_identity,
};

pub(super) fn resolve(
    explicit_cwd: Option<&AbsoluteCwd>,
    selection: Option<&orchbee_capability_node_protocol::LaunchSelection>,
    environment: &orchbee_capability_node_protocol::LaunchEnvironment,
) -> Result<ResolvedTerminalLaunchFacts, Box<dyn Error>> {
    // An authorized selection is adopted verbatim: no PATH search, no shell
    // resolution, and no silent fallback to the default launch. The default
    // environment entries are protocol-fixed, so an explicit selection does
    // not depend on a usable shell existing on this Worker.
    let (executable, arguments) = match selection {
        Some(selection) => (
            selection.executable().to_owned(),
            selection.arguments().to_vec(),
        ),
        None => {
            let launch = resolve_default_interactive_launch()?;
            (launch.executable, launch.arguments)
        }
    };
    // An explicit authorized path requires no home-directory lookup or translation.
    let cwd = match explicit_cwd {
        Some(cwd) => cwd.clone(),
        None => {
            let home_path = default_home()?;
            let home = home_path.to_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Worker home is not UTF-8")
            })?;
            AbsoluteCwd::new(home)?
        }
    };
    Ok(ResolvedTerminalLaunchFacts::new(
        CreationId::from_bytes(fresh_identity()?)?,
        executable,
        arguments,
        default_launch_environment()
            .into_iter()
            .map(|entry| LaunchEnvironmentEntry {
                name: entry.name,
                value: Some(entry.value),
            })
            .chain(
                environment
                    .entries()
                    .iter()
                    .map(|(name, value)| LaunchEnvironmentEntry {
                        name: name.clone(),
                        value: Some(value.clone()),
                    }),
            )
            .collect(),
        cwd,
        default_terminal_profile(),
        None,
    ))
}

// The native baseline uses the existing Orchbee default profile. This is a
// protocol profile, not a product Launch Profile or a Client appearance setting.
fn default_terminal_profile() -> TerminalProfile {
    let mut palette = vec![
        0x00_00_00, 0xcd_31_31, 0x10_7c_10, 0x94_98_00, 0x04_51_a5, 0xbc_05_bc, 0x05_98_bc,
        0x55_55_55, 0x66_66_66, 0xcd_31_31, 0x14_ce_14, 0xb5_ba_00, 0x04_51_a5, 0xbc_05_bc,
        0x05_98_bc, 0xa5_a5_a5,
    ];
    for red in [0, 95, 135, 175, 215, 255] {
        for green in [0, 95, 135, 175, 215, 255] {
            for blue in [0, 95, 135, 175, 215, 255] {
                palette.push((red << 16) | (green << 8) | blue);
            }
        }
    }
    for index in 0..24 {
        let value = 8 + index * 10;
        palette.push((value << 16) | (value << 8) | value);
    }
    TerminalProfile {
        color_scheme: ColorScheme::Light,
        default_background_rgb: 0xfc_fc_fc,
        default_cursor_blinking: false,
        default_cursor_rgb: Some(0x57_be_f8),
        default_cursor_shape: CursorShape::Block,
        default_foreground_rgb: 0x14_14_14,
        device_attributes: DeviceAttributes {
            primary_conformance_level: 62,
            primary_features: vec![22],
            secondary_device_type: 1,
            secondary_firmware_version: 0,
            secondary_rom_cartridge: 0,
            tertiary_unit_id: 0,
        },
        enquiry_response: String::new(),
        palette,
        version_response: "Orchbee".to_owned(),
    }
}

#[cfg(unix)]
fn default_home() -> io::Result<std::path::PathBuf> {
    Ok(User::from_uid(Uid::effective())?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Worker account is unavailable"))?
        .dir)
}
#[cfg(windows)]
fn default_home() -> io::Result<std::path::PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .ok_or(io::ErrorKind::NotFound)?;
    if !home.is_absolute() || !home.is_dir() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(home)
}
