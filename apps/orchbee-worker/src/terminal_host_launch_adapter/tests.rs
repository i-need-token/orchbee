use std::{
    num::NonZeroU64,
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostCreationId, HostEpoch, HostTerminalId,
    HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId, StartTransition,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId, TerminalLaunchMode,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    WorkerGeneration, WorkerId,
};
use orchbee_local_site_protocol::WorkerId as LocalSiteWorkerId;
use orchbee_terminal_protocol::{
    ColorScheme, CursorShape, DeviceAttributes, MAXIMUM_LAUNCH_ARGUMENTS,
    MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES, MAXIMUM_TYPED_FAILURE_CODE_BYTES, ProtocolErrorCode,
    TERMINAL_THEME_PALETTE_ENTRIES,
};

use super::*;
use crate::terminal_authorization::{
    AuthorizedHostDispatch, AuthorizedHostOperationKernel, HostOperationAdmission,
};

const CURRENT_LOCAL_WORKER_ID: LocalSiteWorkerId = LocalSiteWorkerId::from_bytes([3; 16]);
const PRODUCT_TERMINAL: u8 = 11;
const HOST_CREATION: u8 = 17;
const HOST_EPOCH: u8 = 12;
const HOST_TERMINAL: u8 = 13;

#[test]
fn immediate_launch_maps_stable_identity_explicit_cwd_and_all_resolved_facts() -> TestResult {
    let geometry = geometry(132, 43, 9, 18)?;
    let dispatch = launch_dispatch(
        Some(AbsoluteCwd::new("/workspace/repository")?),
        TerminalLaunchMode::Immediate(geometry),
    )?;
    let facts = resolved_facts("/Users/worker")?;

    let request = map_launch_dispatch(&dispatch, facts, None)?;

    assert_eq!(request.creation_id.as_bytes(), &[HOST_CREATION; 16]);
    assert_ne!(request.creation_id.as_bytes(), &[PRODUCT_TERMINAL; 16]);
    assert!(request.environment_overlay.is_empty());
    assert_eq!(request.launch.executable, "/bin/zsh");
    assert!(request.launch.arguments.is_empty());
    assert_eq!(request.launch.environment, default_environment());
    assert_eq!(
        request.launch.current_directory.as_deref(),
        Some("/workspace/repository")
    );
    assert_eq!(
        request.launch.configuration,
        Some(TerminalConfiguration {
            scrollback_bytes: 8 * 1024 * 1024,
        })
    );
    assert_eq!(request.launch.terminal_profile, terminal_profile());
    assert_eq!(
        request.launch.start_mode,
        LaunchStartMode::Immediate(protocol_geometry(geometry))
    );
    Ok(())
}

#[test]
fn deferred_launch_uses_resolved_home_and_carries_no_geometry() -> TestResult {
    let dispatch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;

    let request = map_launch_dispatch(&dispatch, resolved_facts("/home/worker")?, None)?;

    assert_eq!(
        request.launch.current_directory.as_deref(),
        Some("/home/worker")
    );
    assert_eq!(request.launch.start_mode, LaunchStartMode::Deferred);
    assert!(request.environment_overlay.is_empty());
    Ok(())
}

#[test]
fn resolved_host_creation_identity_is_stable_and_independent_from_product_terminal() -> TestResult {
    let first = map_launch_dispatch(
        &launch_dispatch(None, TerminalLaunchMode::Deferred)?,
        resolved_facts("/home/worker")?,
        None,
    )?;
    let second = map_launch_dispatch(
        &launch_dispatch(
            Some(AbsoluteCwd::new("/different")?),
            TerminalLaunchMode::Immediate(geometry(80, 24, 8, 16)?),
        )?,
        resolved_facts("/another-home")?,
        None,
    )?;

    assert_eq!(first.creation_id, second.creation_id);
    assert_eq!(first.creation_id.as_bytes(), &[HOST_CREATION; 16]);
    assert_ne!(first.creation_id.as_bytes(), &[PRODUCT_TERMINAL; 16]);
    Ok(())
}

#[test]
fn start_maps_only_the_exact_host_terminal_and_complete_authorized_geometry() -> TestResult {
    let target = capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?;
    let geometry = geometry(160, 51, 7, 15)?;
    let dispatch = start_dispatch(target, geometry)?;

    let request = map_start_dispatch(&dispatch)?;

    assert_eq!(request.terminal_id.as_bytes(), &[HOST_TERMINAL; 16]);
    assert_eq!(request.geometry, protocol_geometry(geometry));
    Ok(())
}

#[test]
fn start_preserves_a_target_creation_identity_distinct_from_the_product_terminal() -> TestResult {
    let target = capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?;
    let geometry = geometry(80, 24, 8, 16)?;
    let dispatch = start_dispatch(target, geometry)?;

    let request = map_start_dispatch(&dispatch)?;
    assert_eq!(request.terminal_id.as_bytes(), &[HOST_TERMINAL; 16]);
    assert_ne!(target.creation_id().as_bytes(), [PRODUCT_TERMINAL; 16]);
    Ok(())
}

#[test]
fn request_and_response_functions_reject_the_opposite_authorized_operation_kind() -> TestResult {
    let launch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;
    let start = start_dispatch(
        capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
        geometry(80, 24, 8, 16)?,
    )?;

    assert_eq!(
        map_launch_dispatch(&start, resolved_facts("/home/worker")?, None),
        Err(HostLaunchAdapterError::OperationKindMismatch)
    );
    assert_eq!(
        map_start_dispatch(&launch),
        Err(HostLaunchAdapterError::OperationKindMismatch)
    );
    assert_eq!(
        map_launch_host_response(
            &start,
            host_creation_id()?,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH)?,
                TerminalHostResponse::TypedFailure(typed_failure(false)),
            ),
        ),
        Err(HostLaunchAdapterError::OperationKindMismatch)
    );
    assert_eq!(
        map_start_host_response(
            &launch,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH)?,
                TerminalHostResponse::TypedFailure(typed_failure(false)),
            ),
        ),
        Err(HostLaunchAdapterError::OperationKindMismatch)
    );
    Ok(())
}

#[test]
fn launch_success_preserves_exact_host_epoch_and_identity() -> TestResult {
    for mode in [
        TerminalLaunchMode::Deferred,
        TerminalLaunchMode::Immediate(geometry(80, 24, 8, 16)?),
    ] {
        let dispatch = launch_dispatch(None, mode)?;
        let identity = protocol_identity_value(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?;
        let completion = map_launch_host_response(
            &dispatch,
            host_creation_id()?,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH)?,
                TerminalHostResponse::TerminalCreated(identity),
            ),
        )?;

        assert_eq!(
            completion,
            LaunchHostCompletion::Accepted {
                identity: capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
            }
        );
    }
    Ok(())
}

#[test]
fn every_generic_typed_host_failure_is_a_reported_launch_failure() -> TestResult {
    for retryable in [false, true] {
        let dispatch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;
        assert_eq!(
            map_launch_host_response(
                &dispatch,
                host_creation_id()?,
                ObservedTerminalHostResponse::new(
                    protocol_epoch(HOST_EPOCH)?,
                    TerminalHostResponse::TypedFailure(typed_failure(retryable)),
                ),
            )?,
            LaunchHostCompletion::Failed {
                host_epoch: capability_epoch(HOST_EPOCH)?,
            }
        );
    }
    Ok(())
}

#[test]
fn start_success_preserves_exact_identity_geometry_and_transition() -> TestResult {
    let target = capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?;
    let geometry = geometry(120, 37, 8, 17)?;

    for (host_transition, capability_transition) in [
        (ProtocolStartTransition::Started, StartTransition::Started),
        (
            ProtocolStartTransition::AlreadyStarted,
            StartTransition::AlreadyStarted,
        ),
    ] {
        let dispatch = start_dispatch(target, geometry)?;
        let completion = map_start_host_response(
            &dispatch,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH)?,
                TerminalHostResponse::TerminalStartAccepted(TerminalStartAccepted {
                    geometry: protocol_geometry(geometry),
                    identity: protocol_identity_value(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
                    transition: host_transition,
                }),
            ),
        )?;

        assert_eq!(
            completion,
            StartHostCompletion::Accepted(HostStartAccepted::new(
                target,
                geometry,
                capability_transition,
            ))
        );
    }
    Ok(())
}

#[test]
fn every_generic_typed_host_failure_is_a_reported_start_failure() -> TestResult {
    let target = capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?;
    for retryable in [false, true] {
        let dispatch = start_dispatch(target, geometry(80, 24, 8, 16)?)?;
        assert_eq!(
            map_start_host_response(
                &dispatch,
                ObservedTerminalHostResponse::new(
                    protocol_epoch(HOST_EPOCH)?,
                    TerminalHostResponse::TypedFailure(typed_failure(retryable)),
                ),
            )?,
            StartHostCompletion::Failed {
                host_epoch: capability_epoch(HOST_EPOCH)?,
            }
        );
    }
    Ok(())
}

#[test]
fn response_mapper_rejects_wrong_response_kind() -> TestResult {
    let target = capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?;
    let geometry = geometry(80, 24, 8, 16)?;
    let launch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;
    let start = start_dispatch(target, geometry)?;
    let start_response = TerminalStartAccepted {
        geometry: protocol_geometry(geometry),
        identity: protocol_identity_value(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
        transition: ProtocolStartTransition::Started,
    };

    assert_eq!(
        map_launch_host_response(
            &launch,
            host_creation_id()?,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH)?,
                TerminalHostResponse::TerminalStartAccepted(start_response),
            ),
        ),
        Err(HostLaunchAdapterError::UnexpectedHostResponse)
    );
    assert_eq!(
        map_start_host_response(
            &start,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH)?,
                TerminalHostResponse::TerminalCreated(protocol_identity_value(
                    HOST_CREATION,
                    HOST_EPOCH,
                    HOST_TERMINAL,
                )?),
            ),
        ),
        Err(HostLaunchAdapterError::UnexpectedHostResponse)
    );
    Ok(())
}

#[test]
fn launch_success_rejects_mismatched_creation_identity_or_epoch() -> TestResult {
    let dispatch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;

    assert_eq!(
        map_launch_host_response(
            &dispatch,
            host_creation_id()?,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH)?,
                TerminalHostResponse::TerminalCreated(protocol_identity_value(
                    HOST_CREATION + 1,
                    HOST_EPOCH,
                    HOST_TERMINAL,
                )?),
            ),
        ),
        Err(HostLaunchAdapterError::CreationIdentityMismatch)
    );
    assert_eq!(
        map_launch_host_response(
            &dispatch,
            host_creation_id()?,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH + 1)?,
                TerminalHostResponse::TerminalCreated(protocol_identity_value(
                    HOST_CREATION,
                    HOST_EPOCH,
                    HOST_TERMINAL,
                )?),
            ),
        ),
        Err(HostLaunchAdapterError::HostEpochMismatch)
    );
    Ok(())
}

#[test]
fn start_success_rejects_mismatched_epoch_identity_or_geometry() -> TestResult {
    let target = capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?;
    let geometry = geometry(80, 24, 8, 16)?;
    let dispatch = start_dispatch(target, geometry)?;

    let cases = [
        (
            protocol_epoch(HOST_EPOCH + 1)?,
            TerminalStartAccepted {
                geometry: protocol_geometry(geometry),
                identity: protocol_identity_value(HOST_CREATION, HOST_EPOCH + 1, HOST_TERMINAL)?,
                transition: ProtocolStartTransition::Started,
            },
            HostLaunchAdapterError::HostEpochMismatch,
        ),
        (
            protocol_epoch(HOST_EPOCH)?,
            TerminalStartAccepted {
                geometry: protocol_geometry(geometry),
                identity: protocol_identity_value(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL + 1)?,
                transition: ProtocolStartTransition::Started,
            },
            HostLaunchAdapterError::TerminalIdentityMismatch,
        ),
        (
            protocol_epoch(HOST_EPOCH)?,
            TerminalStartAccepted {
                geometry: ProtocolTerminalPixelGeometry {
                    columns: geometry.columns() + 1,
                    ..protocol_geometry(geometry)
                },
                identity: protocol_identity_value(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
                transition: ProtocolStartTransition::Started,
            },
            HostLaunchAdapterError::GeometryMismatch,
        ),
    ];

    for (host_epoch, accepted, expected) in cases {
        assert_eq!(
            map_start_host_response(
                &dispatch,
                ObservedTerminalHostResponse::new(
                    host_epoch,
                    TerminalHostResponse::TerminalStartAccepted(accepted),
                ),
            ),
            Err(expected)
        );
    }
    Ok(())
}

#[test]
fn start_failure_rejects_a_host_epoch_other_than_the_authorized_target() -> TestResult {
    let dispatch = start_dispatch(
        capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
        geometry(80, 24, 8, 16)?,
    )?;

    assert_eq!(
        map_start_host_response(
            &dispatch,
            ObservedTerminalHostResponse::new(
                protocol_epoch(HOST_EPOCH + 1)?,
                TerminalHostResponse::TypedFailure(typed_failure(false)),
            ),
        ),
        Err(HostLaunchAdapterError::HostEpochMismatch)
    );
    Ok(())
}

#[test]
fn launch_request_reuses_terminal_protocol_argument_and_environment_bounds() -> TestResult {
    let dispatch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;

    let mut exact_arguments = resolved_facts("/home/worker")?;
    exact_arguments.arguments = vec![String::new(); MAXIMUM_LAUNCH_ARGUMENTS];
    map_launch_dispatch(&dispatch, exact_arguments.clone(), None)?;
    exact_arguments.arguments.push(String::new());
    assert_invalid_host_field(map_launch_dispatch(&dispatch, exact_arguments, None));

    let mut exact_environment = resolved_facts("/home/worker")?;
    exact_environment.environment = (0..MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES)
        .map(|index| LaunchEnvironmentEntry {
            name: format!("V{index}"),
            value: None,
        })
        .collect();
    map_launch_dispatch(&dispatch, exact_environment.clone(), None)?;
    exact_environment.environment.push(LaunchEnvironmentEntry {
        name: "OVER_BOUND".to_owned(),
        value: None,
    });
    assert_invalid_host_field(map_launch_dispatch(&dispatch, exact_environment, None));
    Ok(())
}

#[test]
fn launch_and_response_values_are_revalidated_at_the_host_protocol_boundary() -> TestResult {
    let launch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;
    let mut invalid_executable = resolved_facts("/home/worker")?;
    invalid_executable.executable.clear();
    assert_invalid_host_field(map_launch_dispatch(&launch, invalid_executable, None));

    let mut invalid_profile = resolved_facts("/home/worker")?;
    invalid_profile.terminal_profile.palette.pop();
    assert_invalid_host_field(map_launch_dispatch(&launch, invalid_profile, None));

    let mut invalid_failure = typed_failure(false);
    invalid_failure.code = "x".repeat(MAXIMUM_TYPED_FAILURE_CODE_BYTES + 1);
    assert_invalid_host_field(map_launch_host_response(
        &launch,
        host_creation_id()?,
        ObservedTerminalHostResponse::new(
            protocol_epoch(HOST_EPOCH)?,
            TerminalHostResponse::TypedFailure(invalid_failure),
        ),
    ));

    let start = start_dispatch(
        capability_identity(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
        geometry(80, 24, 8, 16)?,
    )?;
    assert_invalid_host_field(map_start_host_response(
        &start,
        ObservedTerminalHostResponse::new(
            protocol_epoch(HOST_EPOCH)?,
            TerminalHostResponse::TerminalStartAccepted(TerminalStartAccepted {
                geometry: ProtocolTerminalPixelGeometry {
                    cell_pixel_height: 16,
                    cell_pixel_width: 8,
                    columns: 0,
                    rows: 24,
                },
                identity: protocol_identity_value(HOST_CREATION, HOST_EPOCH, HOST_TERMINAL)?,
                transition: ProtocolStartTransition::Started,
            }),
        ),
    ));
    Ok(())
}

#[test]
fn admitted_site_observation_socket_sets_the_overlay_variable() -> TestResult {
    let directory = ObservationSiteDirectory::new()?;
    UnixListener::bind(directory.path().join(OBSERVATION_SOCKET_NAME))?;
    let dispatch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;

    let request = map_launch_dispatch(
        &dispatch,
        resolved_facts("/home/worker")?,
        Some(directory.path()),
    )?;

    let expected = directory
        .path()
        .join(OBSERVATION_SOCKET_NAME)
        .to_str()
        .expect("fixture path is UTF-8")
        .to_owned();
    assert_eq!(
        request.environment_overlay,
        vec![EnvironmentOverlayEntry {
            name: OBSERVATION_SOCKET_ENV.to_owned(),
            value: expected,
        }]
    );
    Ok(())
}

#[test]
fn unqualified_observation_candidates_produce_no_overlay() -> TestResult {
    let directory = ObservationSiteDirectory::new()?;
    let dispatch = launch_dispatch(None, TerminalLaunchMode::Deferred)?;
    let socket_name = directory.path().join(OBSERVATION_SOCKET_NAME);

    // Nothing at the socket name: no variable is published.
    let request = map_launch_dispatch(
        &dispatch,
        resolved_facts("/home/worker")?,
        Some(directory.path()),
    )?;
    assert!(request.environment_overlay.is_empty());

    // A regular file is not the bound socket.
    std::fs::write(&socket_name, b"not-a-socket")?;
    let request = map_launch_dispatch(
        &dispatch,
        resolved_facts("/home/worker")?,
        Some(directory.path()),
    )?;
    assert!(request.environment_overlay.is_empty());

    // A symlink to a real socket still fails the symlink_metadata type check.
    std::fs::remove_file(&socket_name)?;
    let target = directory.path().join("elsewhere.sock");
    UnixListener::bind(&target)?;
    std::os::unix::fs::symlink(&target, &socket_name)?;
    let request = map_launch_dispatch(
        &dispatch,
        resolved_facts("/home/worker")?,
        Some(directory.path()),
    )?;
    assert!(request.environment_overlay.is_empty());
    Ok(())
}

type TestResult = Result<(), Box<dyn std::error::Error>>;

static NEXT_OBSERVATION_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct ObservationSiteDirectory(PathBuf);

impl ObservationSiteDirectory {
    fn new() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "orchbee-worker-observation-overlay-{}-{}",
            std::process::id(),
            NEXT_OBSERVATION_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ObservationSiteDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn resolved_facts(
    default_cwd: &str,
) -> Result<ResolvedTerminalLaunchFacts, Box<dyn std::error::Error>> {
    Ok(ResolvedTerminalLaunchFacts::new(
        host_creation_id()?,
        "/bin/zsh".to_owned(),
        Vec::new(),
        default_environment(),
        AbsoluteCwd::new(default_cwd)?,
        terminal_profile(),
        Some(TerminalConfiguration {
            scrollback_bytes: 8 * 1024 * 1024,
        }),
    ))
}

fn default_environment() -> Vec<LaunchEnvironmentEntry> {
    [
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
        ("TERM_PROGRAM", "ghostty"),
    ]
    .into_iter()
    .map(|(name, value)| LaunchEnvironmentEntry {
        name: name.to_owned(),
        value: Some(value.to_owned()),
    })
    .collect()
}

fn terminal_profile() -> TerminalProfile {
    TerminalProfile {
        color_scheme: ColorScheme::Dark,
        default_background_rgb: 0x10_10_10,
        default_cursor_blinking: false,
        default_cursor_rgb: None,
        default_cursor_shape: CursorShape::Block,
        default_foreground_rgb: 0xee_ee_ee,
        device_attributes: DeviceAttributes {
            primary_conformance_level: 65,
            primary_features: vec![1, 4, 22],
            secondary_device_type: 1,
            secondary_firmware_version: 0,
            secondary_rom_cartridge: 0,
            tertiary_unit_id: 0,
        },
        enquiry_response: String::new(),
        palette: vec![0; TERMINAL_THEME_PALETTE_ENTRIES],
        version_response: String::new(),
    }
}

fn launch_dispatch(
    cwd: Option<AbsoluteCwd>,
    mode: TerminalLaunchMode,
) -> Result<AuthorizedHostDispatch, Box<dyn std::error::Error>> {
    let launch =
        AuthorizedTerminalLaunch::new(authorization(PRODUCT_TERMINAL, 21)?, cwd, mode, None);
    let mut kernel = kernel()?;
    expect_dispatch(kernel.admit_launch(launch, 100)?)
}

fn start_dispatch(
    identity: HostTerminalIdentity,
    geometry: TerminalPixelGeometry,
) -> Result<AuthorizedHostDispatch, Box<dyn std::error::Error>> {
    let start =
        AuthorizedTerminalStart::new(authorization(PRODUCT_TERMINAL, 22)?, identity, geometry);
    let mut kernel = kernel()?;
    expect_dispatch(kernel.admit_start(start, 100, identity.host_epoch())?)
}

fn expect_dispatch(
    admission: HostOperationAdmission,
) -> Result<AuthorizedHostDispatch, Box<dyn std::error::Error>> {
    match admission {
        HostOperationAdmission::Dispatch(dispatch) => Ok(dispatch),
        HostOperationAdmission::ResultReady | HostOperationAdmission::SlotOccupied => {
            Err("expected an authorized Host dispatch".into())
        }
    }
}

fn kernel() -> Result<AuthorizedHostOperationKernel, CapabilityProtocolError> {
    AuthorizedHostOperationKernel::new(CURRENT_LOCAL_WORKER_ID, nonzero_u64(7)?)
}

fn authorization(
    terminal: u8,
    operation: u8,
) -> Result<TerminalOperationAuthorization, CapabilityProtocolError> {
    Ok(TerminalOperationAuthorization::new(
        TerminalId::from_bytes([terminal; 16])?,
        OperationId::from_bytes([operation; 16])?,
        WorkerId::from_bytes([3; 16])?,
        WorkerGeneration::new(nonzero_u64(7)?),
        TerminalFence::new(nonzero_u64(1)?),
        TerminalOperationLease::new(
            LeaseId::from_bytes([operation; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(nonzero_u64(200)?),
        ),
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    ))
}

fn geometry(
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) -> Result<TerminalPixelGeometry, CapabilityProtocolError> {
    TerminalPixelGeometry::new(columns, rows, cell_pixel_width, cell_pixel_height)
}

fn capability_identity(
    creation: u8,
    epoch: u8,
    terminal: u8,
) -> Result<HostTerminalIdentity, CapabilityProtocolError> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([creation; 16])?,
        capability_epoch(epoch)?,
        HostTerminalId::from_bytes([terminal; 16])?,
    ))
}

fn capability_epoch(value: u8) -> Result<HostEpoch, CapabilityProtocolError> {
    HostEpoch::from_bytes([value; 16])
}

fn protocol_identity_value(
    creation: u8,
    epoch: u8,
    terminal: u8,
) -> Result<TerminalIdentity, IdentityError> {
    Ok(TerminalIdentity {
        creation_id: CreationId::from_bytes([creation; 16])?,
        host_epoch: protocol_epoch(epoch)?,
        terminal_id: ProtocolTerminalId::from_bytes([terminal; 16])?,
    })
}

fn protocol_epoch(value: u8) -> Result<ProtocolHostEpoch, IdentityError> {
    ProtocolHostEpoch::from_bytes([value; 16])
}

fn host_creation_id() -> Result<CreationId, IdentityError> {
    CreationId::from_bytes([HOST_CREATION; 16])
}

fn typed_failure(retryable: bool) -> TypedFailure {
    TypedFailure {
        code: if retryable {
            "resource-exhausted"
        } else {
            "internal-error"
        }
        .to_owned(),
        message: "terminal unavailable".to_owned(),
        retryable,
    }
}

fn nonzero_u64(value: u64) -> Result<NonZeroU64, CapabilityProtocolError> {
    NonZeroU64::new(value).ok_or(CapabilityProtocolError::InvalidPayload)
}

fn assert_invalid_host_field<T>(result: Result<T, HostLaunchAdapterError>) {
    let Err(HostLaunchAdapterError::HostProtocol(error)) = result else {
        panic!("expected a Terminal Protocol validation error");
    };
    assert_eq!(error.code(), ProtocolErrorCode::InvalidField);
}
