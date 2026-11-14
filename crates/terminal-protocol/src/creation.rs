//! Admits closed terminal creation payloads with bounded literal launches, profiles, and
//! environment overlays. It validates the request representation without resolving executables
//! or launching a process.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    ColorScheme, CreationId, MessageRole, ProtocolError, ProtocolErrorCode, TerminalConfiguration,
    TerminalIdentity, TerminalPixelGeometry,
    control::{RequiredNullable, SafeInteger, decode_wire, preflight_json, serialize_wire},
    geometry::{
        TerminalPixelGeometryWire, geometry_from_wire, geometry_to_wire, validate_geometry,
    },
    limits::{
        MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_ENVIRONMENT_NAME_BYTES,
        MAXIMUM_ENVIRONMENT_OVERLAY_ENTRIES, MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES,
        MAXIMUM_LAUNCH_ARGUMENTS, MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES, MAXIMUM_RGB_VALUE,
        MAXIMUM_TERMINAL_PROFILE_FEATURES, MAXIMUM_TERMINAL_RESPONSE_BYTES,
        TERMINAL_THEME_PALETTE_ENTRIES,
    },
    terminal_configuration::{
        TerminalConfigurationWire, configuration_from_wire, configuration_to_wire,
        validate_configuration,
    },
    terminal_fact::{
        TerminalIdentityWire, bounded_u16, bounded_u32, identity_from_wire, identity_to_wire,
    },
};

const CREATE_TERMINAL_KIND: u16 = 11;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceAttributes {
    pub primary_conformance_level: u16,
    pub primary_features: Vec<u16>,
    pub secondary_device_type: u16,
    pub secondary_firmware_version: u16,
    pub secondary_rom_cartridge: u16,
    pub tertiary_unit_id: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalProfile {
    pub color_scheme: ColorScheme,
    pub default_background_rgb: u32,
    pub default_cursor_blinking: bool,
    pub default_cursor_rgb: Option<u32>,
    pub default_cursor_shape: CursorShape,
    pub default_foreground_rgb: u32,
    pub device_attributes: DeviceAttributes,
    pub enquiry_response: String,
    pub palette: Vec<u32>,
    pub version_response: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchEnvironmentEntry {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentOverlayEntry {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchStartMode {
    Immediate(TerminalPixelGeometry),
    Deferred,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLaunch {
    pub arguments: Vec<String>,
    pub configuration: Option<TerminalConfiguration>,
    pub current_directory: Option<String>,
    pub environment: Vec<LaunchEnvironmentEntry>,
    pub executable: String,
    pub start_mode: LaunchStartMode,
    pub terminal_profile: TerminalProfile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateTerminal {
    pub creation_id: CreationId,
    pub environment_overlay: Vec<EnvironmentOverlayEntry>,
    pub launch: TerminalLaunch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalCreationControl {
    CreateTerminal(Box<CreateTerminal>),
    TerminalCreated(TerminalIdentity),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedTerminalCreationControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_terminal_creation_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<TerminalCreationControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (CREATE_TERMINAL_KIND, MessageRole::Request) => decode_wire::<CreateTerminalWire>(payload)
            .and_then(create_terminal_from_wire)
            .map(|creation| TerminalCreationControl::CreateTerminal(Box::new(creation))),
        (CREATE_TERMINAL_KIND, MessageRole::Response) => {
            decode_wire::<TerminalIdentityWire>(payload)
                .and_then(identity_from_wire)
                .map(TerminalCreationControl::TerminalCreated)
        }
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select a terminal-creation control payload",
        )),
    }
}

pub fn encode_terminal_creation_control(
    value: &TerminalCreationControl,
) -> Result<EncodedTerminalCreationControl, ProtocolError> {
    let (role, payload) = match value {
        TerminalCreationControl::CreateTerminal(value) => (
            MessageRole::Request,
            serialize_wire(&create_terminal_to_wire(value)?)?,
        ),
        TerminalCreationControl::TerminalCreated(identity) => (
            MessageRole::Response,
            serialize_wire(&identity_to_wire(*identity))?,
        ),
    };
    Ok(EncodedTerminalCreationControl {
        kind: CREATE_TERMINAL_KIND,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateTerminalWire {
    creation_id: String,
    environment_overlay: Vec<EnvironmentOverlayEntryWire>,
    launch: TerminalLaunchWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnvironmentOverlayEntryWire {
    name: String,
    value: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LaunchEnvironmentEntryWire {
    name: String,
    #[serde(default)]
    value: RequiredNullable<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalLaunchWire {
    arguments: Vec<String>,
    #[serde(default)]
    configuration: RequiredNullable<TerminalConfigurationWire>,
    #[serde(default)]
    current_directory: RequiredNullable<String>,
    environment: Vec<LaunchEnvironmentEntryWire>,
    executable: String,
    #[serde(default)]
    cell_pixel_height: RequiredNullable<SafeInteger>,
    #[serde(default)]
    cell_pixel_width: RequiredNullable<SafeInteger>,
    #[serde(default)]
    columns: RequiredNullable<SafeInteger>,
    #[serde(default)]
    rows: RequiredNullable<SafeInteger>,
    terminal_profile: TerminalProfileWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalProfileWire {
    color_scheme: ColorSchemeWire,
    default_background_rgb: SafeInteger,
    #[serde(default)]
    default_cursor_blinking: bool,
    #[serde(default)]
    default_cursor_rgb: RequiredNullable<SafeInteger>,
    #[serde(default)]
    default_cursor_shape: CursorShapeWire,
    default_foreground_rgb: SafeInteger,
    device_attributes: DeviceAttributesWire,
    enquiry_response: String,
    palette: Vec<SafeInteger>,
    version_response: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ColorSchemeWire {
    Dark,
    Light,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CursorShapeWire {
    #[default]
    Block,
    Underline,
    Bar,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeviceAttributesWire {
    primary_conformance_level: SafeInteger,
    primary_features: Vec<SafeInteger>,
    secondary_device_type: SafeInteger,
    secondary_firmware_version: SafeInteger,
    secondary_rom_cartridge: SafeInteger,
    tertiary_unit_id: SafeInteger,
}

fn create_terminal_from_wire(wire: CreateTerminalWire) -> Result<CreateTerminal, ProtocolError> {
    let creation_id = CreationId::parse(&wire.creation_id).map_err(|_error| invalid_identity())?;
    let environment_overlay = wire
        .environment_overlay
        .into_iter()
        .map(|entry| EnvironmentOverlayEntry {
            name: entry.name,
            value: entry.value,
        })
        .collect();
    let value = CreateTerminal {
        creation_id,
        environment_overlay,
        launch: terminal_launch_from_wire(wire.launch)?,
    };
    validate_create_terminal(&value)?;
    Ok(value)
}

fn create_terminal_to_wire(value: &CreateTerminal) -> Result<CreateTerminalWire, ProtocolError> {
    validate_create_terminal(value)?;
    Ok(CreateTerminalWire {
        creation_id: value.creation_id.to_text(),
        environment_overlay: value
            .environment_overlay
            .iter()
            .map(|entry| EnvironmentOverlayEntryWire {
                name: entry.name.clone(),
                value: entry.value.clone(),
            })
            .collect(),
        launch: terminal_launch_to_wire(&value.launch)?,
    })
}

fn terminal_launch_from_wire(wire: TerminalLaunchWire) -> Result<TerminalLaunch, ProtocolError> {
    let TerminalLaunchWire {
        arguments,
        configuration,
        current_directory,
        environment,
        executable,
        cell_pixel_height,
        cell_pixel_width,
        columns,
        rows,
        terminal_profile,
    } = wire;
    let configuration = configuration
        .into_option()?
        .map(configuration_from_wire)
        .transpose()?;
    let current_directory = current_directory.into_option()?;
    let environment = environment
        .into_iter()
        .map(|entry| {
            Ok(LaunchEnvironmentEntry {
                name: entry.name,
                value: entry.value.into_option()?,
            })
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    let start_mode = launch_start_mode_from_wire(
        cell_pixel_height.into_option()?,
        cell_pixel_width.into_option()?,
        columns.into_option()?,
        rows.into_option()?,
    )?;
    Ok(TerminalLaunch {
        arguments,
        configuration,
        current_directory,
        environment,
        executable,
        start_mode,
        terminal_profile: terminal_profile_from_wire(terminal_profile)?,
    })
}

fn terminal_launch_to_wire(value: &TerminalLaunch) -> Result<TerminalLaunchWire, ProtocolError> {
    let geometry = match value.start_mode {
        LaunchStartMode::Immediate(geometry) => Some(geometry_to_wire(geometry)?),
        LaunchStartMode::Deferred => None,
    };
    Ok(TerminalLaunchWire {
        arguments: value.arguments.clone(),
        configuration: RequiredNullable::from_option(
            value.configuration.as_ref().map(configuration_to_wire),
        ),
        current_directory: RequiredNullable::from_option(value.current_directory.clone()),
        environment: value
            .environment
            .iter()
            .map(|entry| LaunchEnvironmentEntryWire {
                name: entry.name.clone(),
                value: RequiredNullable::from_option(entry.value.clone()),
            })
            .collect(),
        executable: value.executable.clone(),
        cell_pixel_height: RequiredNullable::from_option(
            geometry
                .as_ref()
                .map(|value| SafeInteger::new(value.cell_pixel_height.get())),
        ),
        cell_pixel_width: RequiredNullable::from_option(
            geometry
                .as_ref()
                .map(|value| SafeInteger::new(value.cell_pixel_width.get())),
        ),
        columns: RequiredNullable::from_option(
            geometry
                .as_ref()
                .map(|value| SafeInteger::new(value.columns.get())),
        ),
        rows: RequiredNullable::from_option(
            geometry.map(|value| SafeInteger::new(value.rows.get())),
        ),
        terminal_profile: terminal_profile_to_wire(&value.terminal_profile),
    })
}

fn launch_start_mode_from_wire(
    cell_pixel_height: Option<SafeInteger>,
    cell_pixel_width: Option<SafeInteger>,
    columns: Option<SafeInteger>,
    rows: Option<SafeInteger>,
) -> Result<LaunchStartMode, ProtocolError> {
    match (cell_pixel_height, cell_pixel_width, columns, rows) {
        (Some(cell_pixel_height), Some(cell_pixel_width), Some(columns), Some(rows)) => {
            geometry_from_wire(TerminalPixelGeometryWire {
                cell_pixel_height,
                cell_pixel_width,
                columns,
                rows,
            })
            .map(LaunchStartMode::Immediate)
        }
        (None, None, None, None) => Ok(LaunchStartMode::Deferred),
        _ => Err(invalid_field(
            "terminal launch geometry must be either complete or entirely null",
        )),
    }
}

fn terminal_profile_from_wire(wire: TerminalProfileWire) -> Result<TerminalProfile, ProtocolError> {
    Ok(TerminalProfile {
        color_scheme: match wire.color_scheme {
            ColorSchemeWire::Dark => ColorScheme::Dark,
            ColorSchemeWire::Light => ColorScheme::Light,
        },
        default_background_rgb: rgb_from_wire(wire.default_background_rgb)?,
        default_cursor_blinking: wire.default_cursor_blinking,
        default_cursor_rgb: wire
            .default_cursor_rgb
            .into_option()?
            .map(rgb_from_wire)
            .transpose()?,
        default_cursor_shape: match wire.default_cursor_shape {
            CursorShapeWire::Block => CursorShape::Block,
            CursorShapeWire::Underline => CursorShape::Underline,
            CursorShapeWire::Bar => CursorShape::Bar,
        },
        default_foreground_rgb: rgb_from_wire(wire.default_foreground_rgb)?,
        device_attributes: device_attributes_from_wire(wire.device_attributes)?,
        enquiry_response: wire.enquiry_response,
        palette: wire
            .palette
            .into_iter()
            .map(rgb_from_wire)
            .collect::<Result<Vec<_>, ProtocolError>>()?,
        version_response: wire.version_response,
    })
}

fn terminal_profile_to_wire(value: &TerminalProfile) -> TerminalProfileWire {
    TerminalProfileWire {
        color_scheme: match value.color_scheme {
            ColorScheme::Dark => ColorSchemeWire::Dark,
            ColorScheme::Light => ColorSchemeWire::Light,
        },
        default_background_rgb: SafeInteger::new(u64::from(value.default_background_rgb)),
        default_cursor_blinking: value.default_cursor_blinking,
        default_cursor_rgb: RequiredNullable::from_option(
            value
                .default_cursor_rgb
                .map(|color| SafeInteger::new(u64::from(color))),
        ),
        default_cursor_shape: match value.default_cursor_shape {
            CursorShape::Block => CursorShapeWire::Block,
            CursorShape::Underline => CursorShapeWire::Underline,
            CursorShape::Bar => CursorShapeWire::Bar,
        },
        default_foreground_rgb: SafeInteger::new(u64::from(value.default_foreground_rgb)),
        device_attributes: device_attributes_to_wire(&value.device_attributes),
        enquiry_response: value.enquiry_response.clone(),
        palette: value
            .palette
            .iter()
            .map(|color| SafeInteger::new(u64::from(*color)))
            .collect(),
        version_response: value.version_response.clone(),
    }
}

fn device_attributes_from_wire(
    wire: DeviceAttributesWire,
) -> Result<DeviceAttributes, ProtocolError> {
    Ok(DeviceAttributes {
        primary_conformance_level: bounded_u16(
            wire.primary_conformance_level,
            0,
            usize::from(u16::MAX),
        )?,
        primary_features: wire
            .primary_features
            .into_iter()
            .map(|value| bounded_u16(value, 0, usize::from(u16::MAX)))
            .collect::<Result<Vec<_>, ProtocolError>>()?,
        secondary_device_type: bounded_u16(wire.secondary_device_type, 0, usize::from(u16::MAX))?,
        secondary_firmware_version: bounded_u16(
            wire.secondary_firmware_version,
            0,
            usize::from(u16::MAX),
        )?,
        secondary_rom_cartridge: bounded_u16(
            wire.secondary_rom_cartridge,
            0,
            usize::from(u16::MAX),
        )?,
        tertiary_unit_id: bounded_u32(wire.tertiary_unit_id, 0, u32::MAX)?,
    })
}

fn device_attributes_to_wire(value: &DeviceAttributes) -> DeviceAttributesWire {
    DeviceAttributesWire {
        primary_conformance_level: SafeInteger::new(u64::from(value.primary_conformance_level)),
        primary_features: value
            .primary_features
            .iter()
            .map(|feature| SafeInteger::new(u64::from(*feature)))
            .collect(),
        secondary_device_type: SafeInteger::new(u64::from(value.secondary_device_type)),
        secondary_firmware_version: SafeInteger::new(u64::from(value.secondary_firmware_version)),
        secondary_rom_cartridge: SafeInteger::new(u64::from(value.secondary_rom_cartridge)),
        tertiary_unit_id: SafeInteger::new(u64::from(value.tertiary_unit_id)),
    }
}

fn rgb_from_wire(value: SafeInteger) -> Result<u32, ProtocolError> {
    bounded_u32(value, 0, MAXIMUM_RGB_VALUE)
}

fn validate_create_terminal(value: &CreateTerminal) -> Result<(), ProtocolError> {
    validate_terminal_launch(&value.launch)?;
    if value.environment_overlay.len() > MAXIMUM_ENVIRONMENT_OVERLAY_ENTRIES {
        return Err(invalid_field(
            "terminal environment overlay exceeds its entry bound",
        ));
    }
    validate_unique_names(
        value
            .environment_overlay
            .iter()
            .map(|entry| entry.name.as_str()),
        "terminal environment overlay contains a duplicate name",
    )?;
    for entry in &value.environment_overlay {
        validate_environment_name(&entry.name)?;
        validate_text(
            &entry.value,
            MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES,
            false,
            "terminal environment overlay value exceeds its protocol bounds",
        )?;
    }
    Ok(())
}

fn validate_terminal_launch(value: &TerminalLaunch) -> Result<(), ProtocolError> {
    if value.arguments.len() > MAXIMUM_LAUNCH_ARGUMENTS {
        return Err(invalid_field("terminal launch exceeds its argument bound"));
    }
    for argument in &value.arguments {
        validate_launch_text(argument, false)?;
    }
    if let Some(configuration) = &value.configuration {
        validate_configuration(configuration)?;
    }
    if let Some(current_directory) = &value.current_directory {
        validate_launch_text(current_directory, false)?;
    }
    if value.environment.len() > MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES {
        return Err(invalid_field(
            "terminal launch environment exceeds its entry bound",
        ));
    }
    validate_unique_names(
        value.environment.iter().map(|entry| entry.name.as_str()),
        "terminal launch environment contains a duplicate name",
    )?;
    for entry in &value.environment {
        validate_environment_name(&entry.name)?;
        if let Some(entry_value) = &entry.value {
            validate_launch_text(entry_value, false)?;
        }
    }
    validate_launch_text(&value.executable, true)?;
    if let LaunchStartMode::Immediate(geometry) = value.start_mode {
        validate_geometry(geometry)?;
    }
    validate_terminal_profile(&value.terminal_profile)
}

fn validate_terminal_profile(value: &TerminalProfile) -> Result<(), ProtocolError> {
    validate_rgb(value.default_background_rgb)?;
    if let Some(color) = value.default_cursor_rgb {
        validate_rgb(color)?;
    }
    validate_rgb(value.default_foreground_rgb)?;
    if value.device_attributes.primary_features.len() > MAXIMUM_TERMINAL_PROFILE_FEATURES {
        return Err(invalid_field(
            "terminal profile exceeds its primary feature bound",
        ));
    }
    let mut features = HashSet::new();
    if value
        .device_attributes
        .primary_features
        .iter()
        .any(|feature| !features.insert(*feature))
    {
        return Err(invalid_field(
            "terminal profile contains a duplicate primary feature",
        ));
    }
    validate_text(
        &value.enquiry_response,
        MAXIMUM_TERMINAL_RESPONSE_BYTES,
        false,
        "terminal enquiry response exceeds its protocol bounds",
    )?;
    if value.palette.len() != TERMINAL_THEME_PALETTE_ENTRIES {
        return Err(invalid_field(
            "terminal profile palette does not contain exactly 256 colors",
        ));
    }
    for color in &value.palette {
        validate_rgb(*color)?;
    }
    validate_text(
        &value.version_response,
        MAXIMUM_TERMINAL_RESPONSE_BYTES,
        false,
        "terminal version response exceeds its protocol bounds",
    )
}

fn validate_rgb(value: u32) -> Result<(), ProtocolError> {
    if value > MAXIMUM_RGB_VALUE {
        return Err(invalid_field(
            "terminal profile color exceeds unsigned 24-bit RGB",
        ));
    }
    Ok(())
}

fn validate_launch_text(value: &str, require_nonempty: bool) -> Result<(), ProtocolError> {
    validate_text(
        value,
        MAXIMUM_CONTROL_JSON_BYTES,
        require_nonempty,
        "terminal launch text exceeds its protocol bounds",
    )
}

fn validate_environment_name(value: &str) -> Result<(), ProtocolError> {
    validate_text(
        value,
        MAXIMUM_ENVIRONMENT_NAME_BYTES,
        true,
        "terminal environment name exceeds its protocol bounds",
    )?;
    if value.contains('=') {
        return Err(invalid_field(
            "terminal environment name contains an invalid separator",
        ));
    }
    Ok(())
}

fn validate_text(
    value: &str,
    maximum_bytes: usize,
    require_nonempty: bool,
    detail: &'static str,
) -> Result<(), ProtocolError> {
    if (require_nonempty && value.is_empty()) || value.len() > maximum_bytes || value.contains('\0')
    {
        return Err(invalid_field(detail));
    }
    Ok(())
}

fn validate_unique_names<'name>(
    names: impl Iterator<Item = &'name str>,
    detail: &'static str,
) -> Result<(), ProtocolError> {
    let mut seen = HashSet::new();
    for name in names {
        if !seen.insert(name) {
            return Err(invalid_field(detail));
        }
    }
    Ok(())
}

const fn invalid_identity() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidIdentifier,
        "terminal creation identity is not canonical nonzero 128-bit hexadecimal text",
    )
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
