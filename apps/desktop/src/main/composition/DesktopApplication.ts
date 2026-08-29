import { randomBytes } from 'node:crypto';
import { constants, userInfo } from 'node:os';
import path from 'node:path';

import type { AgentChangeSource, AgentObservationEvent, AgentTransition } from '@orchbee/agent';
import { createOrchbeeIntl, resolveOrchbeeLocale, workbenchTabMessages } from '@orchbee/i18n';
import { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import {
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DesktopApplicationExitRequestIdSchema,
	DesktopRendererBootstrapSchema,
	type DesktopApplicationExitFailure,
	type DesktopApplicationExitReason,
	type DesktopApplicationExitRequestId,
	type DesktopAgentIntegrationManagementSnapshot,
	type DesktopBrowserAccessSnapshot,
	type DesktopTerminalDefaultSettingsSnapshot,
	type DesktopTerminalLaunchProfileSnapshot,
} from '@orchbee/desktop-client-protocol';
import {
	ClientIdSchema,
	decodeControlJson,
	MessageKind,
	TerminalDefaultsAcceptedSchema,
	TerminalExitedSchema,
	TerminalFailedSchema,
	TerminalApplicationSignalSchema,
	TerminalMetadataChangedSchema,
	type TerminalConfiguration,
	type CreationId,
	type DecodedProtocolEnvelope,
	type AttachmentCapabilities,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
	type TerminalLaunch,
	type TerminalApplicationSignal,
	type TerminalEnvironmentOverlay,
	type TerminalMetadataChanged,
} from '@orchbee/terminal-protocol';
import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
} from '@orchbee/semantic-command-protocol';
import { SemanticCommandLocalEndpointIdSchema } from '@orchbee/semantic-command-protocol/local-http';
import {
	type TerminalPresentationIdSchema,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	type TerminalPresentationId,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';
import {
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPanelNode,
	type WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import type {
	ProjectedAttachmentBootstrap,
	WorkbenchClientSessionBootstrap,
} from '@orchbee/workbench-client-protocol';
import {
	BrowserWindow,
	app,
	clipboard,
	MessageChannelMain,
	screen,
	type BrowserWindowConstructorOptions,
	type MessagePortMain,
} from 'electron';

import {
	DesktopApplicationLifetimeBroker,
	type DesktopApplicationLifetimeBrokerDependencies,
	type DesktopApplicationLifetimeDrainResult,
	type DesktopApplicationLifetimeExitResult,
} from '../application-lifetime';
import {
	createDesktopAgentIntegrationAdapters,
	DesktopAgentIntegrationManagementBroker,
	DesktopAgentIntegrationManagementController,
	NodeAgentIntegrationCommandRunner,
	type DesktopAgentIntegrationManagementBrokerController,
} from '../agent-integration-management';
import {
	AgentHookObservationIngress,
	DesktopAgentObservationBroker,
	DesktopAgentObservationRuntime,
	type DesktopAgentObservationBrokerSource,
} from '../agent-observation';
import {
	ProjectedTerminalBroker,
	type ProjectedTerminalBrokerEffectTarget,
	type ProjectedTerminalBrokerLifecycle,
	type ProjectedTerminalBrokerMetrics,
} from '../broker';
import {
	DesktopClientSessionBroker,
	DesktopClientSessionCoordinator,
	DesktopTerminalIntentReconciler,
	resolveDesktopTerminalPresentationEffectTarget,
	type DesktopClientSessionCoordinatorDependencies,
	type DesktopTerminalIntentReconcilerDependencies,
} from '../client-session';
import { type DesktopGatewayClientHandle } from '../gateway';
import {
	DesktopBrowserAccessBroker,
	DesktopBrowserAccessRuntime,
	type DesktopBrowserAccessBrokerRuntime,
	type DesktopBrowserAccessRuntimeOptions,
} from '../browser-access';
import { DesktopClipboardBroker } from '../desktop-clipboard';
import {
	DesktopEnvironmentRegistry,
	EnvironmentIdSchema,
	EnvironmentRecordSchema,
	FileEnvironmentRegistryStore,
	validateWorkbenchEnvironmentReferences,
	type EnvironmentId,
	type EnvironmentRecord,
	type EnvironmentRegistryCreateResult,
	type EnvironmentRegistryInitializeResult,
	type EnvironmentRegistryReadResult,
} from '../environment';
import { DevelopmentHostRuntime, type HostConnection } from '../host';
import {
	DesktopProjectFolderAcquisitionBroker,
	DesktopProjectFolderAcquisitionRuntime,
} from '../project-folder-acquisition';
import {
	DesktopProjectWorktreeBroker,
	DesktopProjectWorktreeRuntime,
	FileProjectWorktreeOperationStore,
} from '../project-worktree';
import {
	admitDesktopSshEnvironmentRemoteProject,
	attachDesktopSshEnvironmentRemoteProjectFolder,
	type DesktopSshEnvironmentRemoteProjectAdmissionResult,
	type DesktopSshEnvironmentRemoteProjectFolderAttachmentResult,
} from '../remote-project';
import {
	createDesktopSshEnvironmentRemoteTerminal,
	DesktopSshEnvironmentTerminalObservationOwner,
	DesktopSshEnvironmentTerminalRuntimeOwner,
	resolveDesktopSshEnvironmentTerminalRuntimeResources,
	resolveDesktopSshEnvironmentRemoteTerminalCreationTarget,
	type DesktopSshEnvironmentRemoteTerminalResult,
} from '../remote-terminal';
import {
	DesktopRemoteTerminalCreationBroker,
	type DesktopRemoteTerminalCreationBrokerRuntime,
} from '../remote-terminal-creation';
import {
	DesktopSshProjectEnrollmentBroker,
	DesktopSshProjectEnrollmentRuntime,
	discoverOpenSshHostAliases,
	type DesktopSshProjectEnrollmentBrokerRuntime,
} from '../ssh-project-enrollment';
import {
	createDesktopAgentReadOperations,
	createDesktopWorkbenchReadOperations,
	createDesktopWorkbenchTerminalCreateOperation,
	DesktopSemanticCommandCatalog,
	DesktopSemanticCommandLocalEndpoint,
	DesktopSemanticCommandService,
	type DesktopSemanticCommandLocalEndpointOptions,
} from '../semantic-command';
import {
	createSshEnvironmentConnectionState as createSshEnvironmentConnectionStateModel,
	observeSshEnvironmentConnection as observeSshEnvironmentConnectionModel,
	OpenSshTargetResolver,
	OpenSshTransportAdapter,
	PackagedSshEnvironmentReleaseSource,
	SshEnvironmentSupervisor,
	transitionSshEnvironmentConnection as transitionSshEnvironmentConnectionModel,
	UnavailableSshEnvironmentReleaseSource,
	type OpenSshTargetResolutionResult,
	type PackagedSshEnvironmentReleaseSourceOptions,
	type SshEnvironmentConnectionConfiguration,
	type SshEnvironmentConnectionEvent,
	type SshEnvironmentConnectionObservation,
	type SshEnvironmentConnectionState,
	type SshEnvironmentConnectionTransition,
	type SshProjectRootDirectoryListingOutcome,
	type SshProjectRootInspectionOutcome,
	type SshTerminalHostConnectionOutcome,
	type SshEnvironmentTargetReference,
} from '../ssh-environment';
import {
	TerminalExitPublicationLedger,
	TerminalExitSpillStore,
	type TerminalExitSpillReason,
} from '../recovery';
import {
	DesktopTerminalProjectionRouter,
	TerminalProjectionControlBroker,
	type DesktopTerminalProjectionControlService,
} from '../terminal-projection-control';
import {
	DesktopTerminalDefaultSettingsBroker,
	DesktopTerminalDefaultSettingsController,
	FileTerminalDefaultSettingsStore,
	type DesktopTerminalDefaultSettingsBrokerController,
} from '../terminal-default-settings';
import {
	DesktopTerminalLaunchProfileBroker,
	DesktopTerminalLaunchProfileController,
	FileTerminalLaunchProfileStore,
	type DesktopTerminalLaunchProfileBrokerController,
} from '../terminal-launch-profiles';
import type { DesktopTerminalExitRecordResult } from '../terminal-document-control';
import {
	DesktopWindowStateOwner,
	FileDesktopWindowStateStore,
	resolveDesktopWindowLaunchState,
	type DesktopWindowBounds,
	type DesktopWindowDisplayTopology,
	type DesktopWindowStateAttachment,
	type DesktopWindowStateStore,
} from '../window-state';
import {
	DesktopWindowLayoutBroker,
	DesktopWindowLayoutCoordinator,
	FileDesktopWindowLayoutStore,
	loadDesktopWindowLayoutOwner,
	type DesktopWindowLayoutHost,
	type DesktopWindowLayoutProductDocument,
	type DesktopWindowLayoutStore,
	type DesktopWindowLayoutWindow,
} from '../window-layout';
import { FileWorkbenchDocumentStore, type WorkbenchDocumentStore } from '../workbench-document';
import {
	composeDesktopTerminalRuntime,
	type DesktopTerminalRuntime,
} from './composeDesktopTerminalRuntime';
import { createDesktopWorkbenchInitializationDocument } from './createDesktopWorkbenchInitializationDocument';
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';
import { startsDesktopRendererDocument } from './startsDesktopRendererDocument';
import {
	DesktopSshEnvironmentConnectionOwner,
	type DesktopSshEnvironmentConnectionOwnerPort,
	type DesktopSshEnvironmentConnectionReadResult,
	type DesktopSshEnvironmentConnectResult,
	type DesktopSshEnvironmentDisconnectResult,
} from './DesktopSshEnvironmentConnectionOwner';

interface DesktopBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	claimResizeLease(): Promise<void>;
	dispose(): Promise<void>;
	setInputCapability(input: boolean): Promise<void>;
	start(): void;
}

interface DesktopClientSessionBrokerHandle {
	readonly bootstrap: WorkbenchClientSessionBootstrap;
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopProjectionControlBrokerHandle {
	dispose(): Promise<void>;
	start(): void;
}

interface DesktopApplicationLifetimeBrokerHandle {
	dispose(): void;
	requestExit(
		requestId: DesktopApplicationExitRequestId,
		reason: DesktopApplicationExitReason,
	): Promise<DesktopApplicationLifetimeExitResult>;
	start(): void;
}

interface DesktopAgentObservationBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopAgentIntegrationManagementBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopAgentIntegrationManagementControllerHandle extends DesktopAgentIntegrationManagementBrokerController {
	initialize(): Promise<DesktopAgentIntegrationManagementSnapshot>;
}

type DesktopAgentObservationRuntimeHandle = AgentChangeSource &
	DesktopAgentObservationBrokerSource & {
		dispatch(event: AgentObservationEvent): AgentTransition;
		dispose(): void;
		observeTerminalApplicationSignal?(
			event: TerminalApplicationSignal,
			presentationId: TerminalPresentationId,
		): void;
		retireTerminalSignalPresentation?(presentationId: TerminalPresentationId): void;
	};

interface DesktopAgentHookObservationIngressHandle {
	dispose(): Promise<void>;
	environmentOverlayFor(
		presentationId: TerminalPresentationId,
		creationId: CreationId,
	): TerminalEnvironmentOverlay;
	observeTerminalApplicationSignal(
		event: TerminalApplicationSignal,
		presentationId: TerminalPresentationId,
	): void;
	retirePresentation(presentationId: TerminalPresentationId): void;
	start(): Promise<void>;
}

interface DesktopTerminalEnvironmentOverlayPort {
	resolve(
		presentationId: TerminalPresentationId,
		creationId: CreationId,
	): TerminalEnvironmentOverlay;
	retire(presentationId: TerminalPresentationId): void;
}

interface DesktopBrowserAccessBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopClipboardBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopBrowserAccessRuntimeHandle extends DesktopBrowserAccessBrokerRuntime {
	dispose(): Promise<void>;
	initialize(): Promise<DesktopBrowserAccessSnapshot>;
}

interface DesktopProjectFolderAcquisitionBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopProjectWorktreeBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopRemoteTerminalCreationBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopSshProjectEnrollmentBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopSemanticCommandLocalEndpointHandle {
	dispose(): Promise<void>;
	start(): Promise<void>;
}

type DesktopProjectWorktreeRuntimeHandle = Pick<
	DesktopProjectWorktreeRuntime,
	'adopt' | 'create' | 'discover' | 'dispose' | 'start'
>;

interface DesktopTerminalLaunchProfileBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopTerminalDefaultSettingsBrokerHandle {
	dispose(): void;
	start(): void;
}

interface DesktopTerminalDefaultSettingsControllerHandle extends DesktopTerminalDefaultSettingsBrokerController {
	flush(): Promise<void>;
	initialize(): Promise<DesktopTerminalDefaultSettingsSnapshot>;
}

interface DesktopTerminalLaunchProfileControllerHandle extends DesktopTerminalLaunchProfileBrokerController {
	flush(): Promise<void>;
	initialize(): Promise<DesktopTerminalLaunchProfileSnapshot>;
	resolveDefaultLaunch(): TerminalLaunch;
}

interface GatewayClientResourceOwner {
	dispose(): Promise<void>;
}

interface DesktopRendererDocumentHandle {
	readonly cleanup: () => Promise<void>;
	readonly lifetimeBroker: DesktopApplicationLifetimeBrokerHandle;
}

interface DesktopNativeWindowRecord {
	geometryCaptureScheduled: boolean;
	layoutCommitted: boolean;
	loading: Promise<void> | undefined;
	pendingRendererGeneration: number | undefined;
	retiring: boolean;
	readonly window: BrowserWindow;
	readonly windowId: string;
}

type TerminalExitPublicationResult = 'already-committed' | 'committed' | 'pending';

const MAX_PENDING_TERMINAL_EXIT_RETRIES = 64;
const MAX_PENDING_TERMINAL_METADATA = 1_024;
const RENDERER_LIFETIME_RECOVERY_POLL_MILLISECONDS = 25;
const RENDERER_LIFETIME_RECOVERY_TIMEOUT_MILLISECONDS = 5_000;
const TERMINAL_EXIT_RETRY_DELAY_MILLISECONDS = 25;
const TERMINAL_EXIT_ASSOCIATION_RETRY_LIMIT = 3;
const MACOS_TRAFFIC_LIGHT_POSITION = { x: 8, y: 10 } as const;

function terminalRgbCss(value: number): string {
	return `#${value.toString(16).padStart(6, '0')}`;
}

export interface DesktopSshEnvironmentResources {
	readonly askPassHelperEntryPath: string;
	readonly askPassRuntimeExecutablePath: string;
	readonly release: PackagedSshEnvironmentReleaseSourceOptions | null;
}

export interface DesktopApplicationOptions {
	readonly prepareDevelopmentHostRestart?:
		| ((documentRevision: WorkbenchSharedDocumentRevision) => Promise<boolean>)
		| undefined;
	readonly platform: NodeJS.Platform;
	readonly preloadPath: string;
	readonly rendererUrl: string;
	readonly runtime: DesktopRuntimeConfiguration;
	readonly semanticCommandEndpointWorkerPath: string;
	readonly sshEnvironment: DesktopSshEnvironmentResources;
	readonly singleInstanceLockOwned: boolean;
	readonly traceCopyPath: boolean;
}

export interface DesktopApplicationDependencies {
	readonly connectHost: (configuration: DesktopRuntimeConfiguration) => Promise<HostConnection>;
	readonly createAgentObservationBroker?:
		| ((
				port: MessagePortMain,
				source: DesktopAgentObservationBrokerSource,
		  ) => DesktopAgentObservationBrokerHandle)
		| undefined;
	readonly createAgentObservationRuntime?:
		| ((options: {
				readonly allocateEventId: () => string;
		  }) => DesktopAgentObservationRuntimeHandle)
		| undefined;
	readonly createAgentHookObservationIngress?:
		| ((
				runtime: DesktopAgentObservationRuntimeHandle,
				options: {
					readonly allocateIdentifier: () => string;
					readonly socketPath: string | null;
				},
		  ) => DesktopAgentHookObservationIngressHandle)
		| undefined;
	readonly createAgentIntegrationManagementBroker: (
		port: MessagePortMain,
		controller: DesktopAgentIntegrationManagementBrokerController,
	) => DesktopAgentIntegrationManagementBrokerHandle;
	readonly createAgentIntegrationManagementController: (options: {
		readonly homeDirectory: string;
		readonly platform: NodeJS.Platform;
		readonly resourceRoot: string;
		readonly shellExecutable: string;
	}) => DesktopAgentIntegrationManagementControllerHandle;
	readonly createBroker: (
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
		effectTarget: ProjectedTerminalBrokerEffectTarget,
		lifecycle?: ProjectedTerminalBrokerLifecycle,
		initialCapabilities?: AttachmentCapabilities,
	) => DesktopBrokerHandle;
	readonly createApplicationLifetimeBroker: (
		port: MessagePortMain,
		dependencies: DesktopApplicationLifetimeBrokerDependencies,
	) => DesktopApplicationLifetimeBrokerHandle;
	readonly createBrowserAccessBroker: (
		port: MessagePortMain,
		runtime: DesktopBrowserAccessBrokerRuntime,
	) => DesktopBrowserAccessBrokerHandle;
	readonly createBrowserAccessRuntime: (
		options: DesktopBrowserAccessRuntimeOptions,
		openClient: () => Promise<DesktopGatewayClientHandle>,
	) => DesktopBrowserAccessRuntimeHandle;
	readonly createClipboardBroker: (
		port: MessagePortMain,
		readClipboardText: () => string,
	) => DesktopClipboardBrokerHandle;
	readonly createEnvironmentRegistry: (filePath: string) => DesktopEnvironmentRegistry;
	readonly createSshEnvironmentConnectionOwner: (options: {
		readonly askPassHelperEntryPath: string;
		readonly askPassRuntimeExecutablePath: string;
		readonly defaultConfigurationFilePath: null | string;
		readonly platform: NodeJS.Platform;
		readonly registry: Pick<DesktopEnvironmentRegistry, 'get'>;
		readonly release: PackagedSshEnvironmentReleaseSourceOptions | null;
	}) => DesktopSshEnvironmentConnectionOwnerPort;
	readonly createClientSessionBroker: (
		port: MessagePortMain,
		coordinator: DesktopClientSessionCoordinator,
		disposeAttachments: () => Promise<void>,
	) => DesktopClientSessionBrokerHandle;
	readonly createClientSessionCoordinator: (
		dependencies: DesktopClientSessionCoordinatorDependencies,
	) => DesktopClientSessionCoordinator;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly createProjectionControlBroker: (
		port: MessagePortMain,
		service: DesktopTerminalProjectionControlService,
	) => DesktopProjectionControlBrokerHandle;
	readonly createProjectFolderAcquisitionBroker: (
		port: MessagePortMain,
		window: BrowserWindow,
	) => DesktopProjectFolderAcquisitionBrokerHandle;
	readonly createProjectWorktreeBroker: (
		port: MessagePortMain,
		runtime: DesktopProjectWorktreeRuntimeHandle,
	) => DesktopProjectWorktreeBrokerHandle;
	readonly createRemoteTerminalCreationBroker: (
		port: MessagePortMain,
		runtime: DesktopRemoteTerminalCreationBrokerRuntime,
	) => DesktopRemoteTerminalCreationBrokerHandle;
	readonly createSshProjectEnrollmentBroker: (
		port: MessagePortMain,
		runtime: DesktopSshProjectEnrollmentBrokerRuntime,
	) => DesktopSshProjectEnrollmentBrokerHandle;
	readonly createProjectWorktreeRuntime: (options: {
		readonly allocateIdentifier: () => string;
		readonly coordinator: DesktopClientSessionCoordinator;
		readonly defaultCwd: string;
		readonly managedRoot: string;
		readonly operationStorePath: string;
		readonly terminalTitle: string;
	}) => DesktopProjectWorktreeRuntimeHandle;
	readonly createSemanticCommandLocalEndpoint: (
		options: DesktopSemanticCommandLocalEndpointOptions,
	) => DesktopSemanticCommandLocalEndpointHandle;
	readonly createTerminalLaunchProfileBroker: (
		port: MessagePortMain,
		controller: DesktopTerminalLaunchProfileBrokerController,
	) => DesktopTerminalLaunchProfileBrokerHandle;
	readonly createTerminalDefaultSettingsBroker: (
		port: MessagePortMain,
		controller: DesktopTerminalDefaultSettingsBrokerController,
	) => DesktopTerminalDefaultSettingsBrokerHandle;
	readonly createTerminalDefaultSettingsController: (options: {
		readonly runtimeConfiguration: TerminalConfiguration;
		readonly setHostDefaults: (
			configuration: TerminalConfiguration,
		) => Promise<TerminalConfiguration>;
		readonly storePath: string;
	}) => DesktopTerminalDefaultSettingsControllerHandle;
	readonly createTerminalLaunchProfileController: (options: {
		readonly allocateProfileId: () => string;
		readonly automaticShellIntegration: DesktopRuntimeConfiguration['automaticShellIntegration'];
		readonly baseLaunch: TerminalLaunch;
		readonly platform: NodeJS.Platform;
		readonly storePath: string;
	}) => DesktopTerminalLaunchProfileControllerHandle;
	readonly createTerminalRuntime: (
		host: HostConnection,
		configuration: DesktopRuntimeConfiguration,
		randomIdentifier: () => string,
		createMessageChannel: DesktopApplicationDependencies['createMessageChannel'],
		createBroker: DesktopApplicationDependencies['createBroker'],
		resolveTerminalLaunch: () => TerminalLaunch,
		environmentOverlayPort?: DesktopTerminalEnvironmentOverlayPort,
	) => DesktopTerminalRuntime;
	readonly createTerminalExitSpillStore?: (filePath: string) => TerminalExitSpillStore;
	readonly createTerminalExitPublicationLedger?: (
		filePath: string,
	) => TerminalExitPublicationLedger;
	readonly createTerminalIntentReconciler: (
		dependencies: DesktopTerminalIntentReconcilerDependencies,
	) => DesktopTerminalIntentReconciler;
	readonly createWorkbenchDocumentStore: (
		filePath: string,
		legacyFilePath: string,
	) => WorkbenchDocumentStore;
	readonly createWindow: (options: BrowserWindowConstructorOptions) => BrowserWindow;
	readonly createWindowLayoutStore?: (filePath: string) => DesktopWindowLayoutStore;
	readonly createWindowStateStore: (filePath: string) => DesktopWindowStateStore;
	readonly getWindowDisplayTopology: () => DesktopWindowDisplayTopology;
	readonly getPreferredLocales: () => readonly string[];
	readonly randomIdentifier: () => string;
	readonly quitApplication: () => void;
	readonly sshEnvironmentTargetResolver?: Pick<OpenSshTargetResolver, 'resolve'> | undefined;
	readonly writeTrace: (trace: unknown) => void;
}

const defaultDependencies: DesktopApplicationDependencies = {
	connectHost: (configuration) =>
		new DevelopmentHostRuntime({
			binaryPath: configuration.binaryPath,
			endpointId: configuration.endpointId,
			launchIfUnavailable: configuration.terminalHostLaunchMode === 'connect-or-launch',
			...(configuration.posixHostEndpointAddonPath === null
				? {}
				: { posixEndpointAddonPath: configuration.posixHostEndpointAddonPath }),
			runtimeDirectory: configuration.runtimeDirectory,
			terminalDefaults: configuration.terminalConfiguration,
		}).connect(),
	createAgentObservationBroker: (port, source) => new DesktopAgentObservationBroker(port, source),
	createAgentObservationRuntime: (options) => new DesktopAgentObservationRuntime(options),
	createAgentHookObservationIngress: (runtime, options) =>
		new AgentHookObservationIngress(runtime, options),
	createAgentIntegrationManagementBroker: (port, controller) =>
		new DesktopAgentIntegrationManagementBroker(port, controller),
	createAgentIntegrationManagementController: (options) => {
		const commandRunner = new NodeAgentIntegrationCommandRunner({
			environment: process.env,
			homeDirectory: options.homeDirectory,
			shellExecutable: options.shellExecutable,
		});
		return new DesktopAgentIntegrationManagementController(
			createDesktopAgentIntegrationAdapters({
				commandRunner,
				homeDirectory: options.homeDirectory,
				platform: options.platform,
				resourceRoot: options.resourceRoot,
			}),
		);
	},
	createBroker: (host, port, bootstrap, effectTarget, lifecycle, initialCapabilities) =>
		new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			undefined,
			effectTarget,
			lifecycle,
			initialCapabilities,
		),
	createApplicationLifetimeBroker: (port, dependencies) =>
		new DesktopApplicationLifetimeBroker(port, dependencies),
	createBrowserAccessBroker: (port, runtime) => new DesktopBrowserAccessBroker(port, runtime),
	createBrowserAccessRuntime: (options, openClient) =>
		new DesktopBrowserAccessRuntime(options, { openClient }),
	createClipboardBroker: (port, readClipboardText) =>
		new DesktopClipboardBroker(port, { readText: readClipboardText }),
	createEnvironmentRegistry: (filePath) =>
		new DesktopEnvironmentRegistry(new FileEnvironmentRegistryStore(filePath)),
	createSshEnvironmentConnectionOwner: (options) => {
		const executablePath = OpenSshTargetResolver.resolveSystemExecutablePath(
			options.platform,
			process.env['SystemRoot'],
		);
		const releaseSource =
			options.release === null
				? new UnavailableSshEnvironmentReleaseSource()
				: new PackagedSshEnvironmentReleaseSource(options.release);
		const targetResolver = new OpenSshTargetResolver(
			executablePath,
			undefined,
			options.defaultConfigurationFilePath,
		);
		return new DesktopSshEnvironmentConnectionOwner({
			createSupervisor: (configuration, interaction) =>
				new SshEnvironmentSupervisor(
					{
						...configuration,
						askPassHelperEntryPath: options.askPassHelperEntryPath,
						askPassRuntimeExecutablePath: options.askPassRuntimeExecutablePath,
					},
					{
						interaction,
						releaseSource,
						targetResolver,
						transportAdapter: new OpenSshTransportAdapter(
							executablePath,
							undefined,
							options.defaultConfigurationFilePath,
						),
					},
				),
			registry: options.registry,
			supported: options.platform !== 'win32',
		});
	},
	createClientSessionBroker: (port, coordinator, disposeAttachments) =>
		new DesktopClientSessionBroker(port, coordinator, { disposeAttachments }),
	createClientSessionCoordinator: (dependencies) =>
		new DesktopClientSessionCoordinator(dependencies),
	createMessageChannel: () => new MessageChannelMain(),
	createProjectionControlBroker: (port, service) =>
		new TerminalProjectionControlBroker(port, service),
	createProjectFolderAcquisitionBroker: (port, window) =>
		new DesktopProjectFolderAcquisitionBroker(
			port,
			new DesktopProjectFolderAcquisitionRuntime(window),
		),
	createProjectWorktreeBroker: (port, runtime) => new DesktopProjectWorktreeBroker(port, runtime),
	createRemoteTerminalCreationBroker: (port, runtime) =>
		new DesktopRemoteTerminalCreationBroker(port, runtime),
	createSshProjectEnrollmentBroker: (port, runtime) =>
		new DesktopSshProjectEnrollmentBroker(port, runtime),
	createProjectWorktreeRuntime: (options) =>
		new DesktopProjectWorktreeRuntime({
			allocateIdentifier: options.allocateIdentifier,
			coordinator: options.coordinator,
			defaultCwd: options.defaultCwd,
			managedRoot: options.managedRoot,
			operationStore: new FileProjectWorktreeOperationStore(options.operationStorePath),
			terminalTitle: options.terminalTitle,
		}),
	createSemanticCommandLocalEndpoint: (options) => new DesktopSemanticCommandLocalEndpoint(options),
	createTerminalDefaultSettingsBroker: (port, controller) =>
		new DesktopTerminalDefaultSettingsBroker(port, controller),
	createTerminalDefaultSettingsController: (options) =>
		new DesktopTerminalDefaultSettingsController({
			runtimeConfiguration: options.runtimeConfiguration,
			setHostDefaults: options.setHostDefaults,
			store: new FileTerminalDefaultSettingsStore(options.storePath),
		}),
	createTerminalLaunchProfileBroker: (port, controller) =>
		new DesktopTerminalLaunchProfileBroker(port, controller),
	createTerminalLaunchProfileController: (options) =>
		new DesktopTerminalLaunchProfileController({
			allocateProfileId: options.allocateProfileId,
			automaticShellIntegration: options.automaticShellIntegration,
			baseLaunch: options.baseLaunch,
			platform: options.platform,
			store: new FileTerminalLaunchProfileStore(options.storePath),
		}),
	createTerminalRuntime: (
		host,
		configuration,
		randomIdentifier,
		createMessageChannel,
		createBroker,
		resolveTerminalLaunch,
		environmentOverlayPort,
	) =>
		composeDesktopTerminalRuntime({
			configuration,
			createBroker,
			createMessageChannel,
			host,
			randomIdentifier,
			...(environmentOverlayPort === undefined
				? {}
				: {
						resolveEnvironmentOverlay: environmentOverlayPort.resolve,
						retireEnvironmentOverlay: environmentOverlayPort.retire,
					}),
			resolveTerminalLaunch,
			writeClipboardText: async (text) => clipboard.writeText(text),
		}),
	createTerminalIntentReconciler: (dependencies) =>
		new DesktopTerminalIntentReconciler(dependencies),
	createTerminalExitPublicationLedger: (filePath) => new TerminalExitPublicationLedger(filePath),
	createWorkbenchDocumentStore: (filePath, legacyFilePath) =>
		new FileWorkbenchDocumentStore(filePath, { legacyFilePath }),
	createWindow: (options) => new BrowserWindow(options),
	createWindowLayoutStore: (filePath) => new FileDesktopWindowLayoutStore(filePath),
	createWindowStateStore: (filePath) => new FileDesktopWindowStateStore(filePath),
	getPreferredLocales: () => app.getPreferredSystemLanguages(),
	getWindowDisplayTopology: () => ({
		primaryWorkArea: screen.getPrimaryDisplay().workArea,
		workAreas: screen.getAllDisplays().map((display) => display.workArea),
	}),
	randomIdentifier: () => randomBytes(16).toString('hex'),
	quitApplication: () => app.quit(),
	writeTrace: (trace) => console.info(JSON.stringify(trace)),
};

const unavailableBrowserAccessRuntime: DesktopBrowserAccessBrokerRuntime = {
	copyAccessLink: async () => unavailableBrowserAccess(),
	getSnapshot: () => ({
		access: { phase: 'stopped' },
		personalAccessToken: 'unavailable',
	}),
	issuePersonalAccessToken: async () => unavailableBrowserAccess(),
	openLocalBrowser: async () => unavailableBrowserAccess(),
	start: async () => unavailableBrowserAccess(),
	stop: async () => unavailableBrowserAccess(),
};

function applicationHomeSshConfigurationFilePath(
	applicationHomeDirectory: string,
	nativeHomeDirectory: string,
): null | string {
	return path.resolve(applicationHomeDirectory) === path.resolve(nativeHomeDirectory)
		? null
		: path.join(applicationHomeDirectory, '.ssh', 'config');
}

function nativeAccountHomeDirectory(fallback: string): string {
	try {
		const directory = userInfo().homedir;
		return path.isAbsolute(directory) ? directory : fallback;
	} catch {
		return fallback;
	}
}

function unavailableBrowserAccess(): never {
	throw Object.assign(new Error('Desktop Browser access resources are unavailable.'), {
		code: 'server-unavailable' as const,
	});
}

export class DesktopApplication {
	readonly #agentIntegrationManagementController: DesktopAgentIntegrationManagementControllerHandle;
	readonly #agentObservationRuntime: DesktopAgentObservationRuntimeHandle;
	#agentHookObservationIngress: DesktopAgentHookObservationIngressHandle | undefined;
	#applicationLifetimeBroker: DesktopApplicationLifetimeBrokerHandle | undefined;
	#applicationLifetimeRecovery: Promise<boolean> | undefined;
	#browserAccessRuntime: DesktopBrowserAccessRuntimeHandle | undefined;
	#clientSessionCoordinator: DesktopClientSessionCoordinator | undefined;
	readonly #dependencies: DesktopApplicationDependencies;
	readonly #options: DesktopApplicationOptions;
	#connection: HostConnection | undefined;
	#documentFailure: Error | undefined;
	#documentGeneration = 0;
	readonly #documentGenerations = new Map<BrowserWindow, number>();
	#documentTransition = Promise.resolve();
	#developmentHostRestartPresentationIds: readonly TerminalPresentationId[] | undefined;
	readonly #environmentRegistry: DesktopEnvironmentRegistry;
	readonly #registeredEnvironmentIds = new Set<EnvironmentId>();
	readonly #sshEnvironmentConnections: DesktopSshEnvironmentConnectionOwnerPort;
	readonly #sshEnvironmentTargetResolver: Pick<OpenSshTargetResolver, 'resolve'>;
	#sshEnvironmentTerminalRuntimeOwner: DesktopSshEnvironmentTerminalRuntimeOwner | undefined;
	#exitRequest: Promise<DesktopApplicationLifetimeExitResult> | undefined;
	readonly #pendingGatewayClientCleanups = new Set<GatewayClientResourceOwner>();
	#projectWorktreeRuntime: DesktopProjectWorktreeRuntimeHandle | undefined;
	#sshProjectEnrollmentRuntime: DesktopSshProjectEnrollmentRuntime | undefined;
	#quitPrepared = false;
	#removeHostListener: (() => void) | undefined;
	#semanticCommandLocalEndpoint: DesktopSemanticCommandLocalEndpointHandle | undefined;
	#semanticCommandService: DesktopSemanticCommandService | undefined;
	#shuttingDown = false;
	#shutdownPromise: Promise<void> | undefined;
	#terminalRuntime: DesktopTerminalRuntime | undefined;
	#terminalIntentReconciler: DesktopTerminalIntentReconciler | undefined;
	#terminalDefaultSettingsController: DesktopTerminalDefaultSettingsControllerHandle | undefined;
	readonly #terminalLaunchProfileController: DesktopTerminalLaunchProfileControllerHandle;
	readonly #pendingTerminalExits = new Map<string, TerminalExited>();
	readonly #pendingTerminalMetadata = new Map<string, TerminalMetadataChanged>();
	readonly #terminalExitAttempts = new Map<string, Promise<void>>();
	readonly #terminalExitPublicationReservations = new Map<
		string,
		{
			readonly event: TerminalExited;
			readonly presentationId: ReturnType<typeof terminalPresentationId>;
		}
	>();
	readonly #publishedTerminalExitRevisions = new Map<string, TerminalExited['finalRevision']>();
	readonly #associatedTerminalExitKeys = new Set<string>();
	readonly #unknownIdentityAttempts = new Set<string>();
	#terminalExitRetryOwnerDisposed = false;
	#terminalExitRetryOwnerClosing = false;
	#terminalExitRetryTimer: ReturnType<typeof setTimeout> | undefined;
	readonly #terminalExitAttemptCounts = new Map<string, number>();
	readonly #terminalExitSpillStore: TerminalExitSpillStore;
	readonly #terminalExitPublicationLedger: TerminalExitPublicationLedger;
	#terminalExitSpillReady = false;
	readonly #rendererDocuments = new Map<BrowserWindow, DesktopRendererDocumentHandle>();
	readonly #rendererDocumentRetirements = new Map<BrowserWindow, DesktopRendererDocumentHandle>();
	#window: BrowserWindow | undefined;
	readonly #windows = new Map<string, DesktopNativeWindowRecord>();
	readonly #windowIds = new Map<BrowserWindow, string>();
	#windowLayoutCoordinator: DesktopWindowLayoutCoordinator | undefined;
	#windowLayoutOwner: Awaited<ReturnType<typeof loadDesktopWindowLayoutOwner>> | undefined;
	#windowStateAttachment: DesktopWindowStateAttachment | undefined;
	#windowStateAttachmentWindow: BrowserWindow | undefined;
	readonly #windowStateOwner: DesktopWindowStateOwner;
	readonly #workbenchDocumentStore: WorkbenchDocumentStore;

	constructor(
		options: DesktopApplicationOptions,
		dependencies: DesktopApplicationDependencies = defaultDependencies,
	) {
		if (!options.singleInstanceLockOwned) {
			throw new Error('Desktop recovery requires the Electron single-instance lock.');
		}
		this.#options = options;
		this.#dependencies = dependencies;
		const defaultSshConfigurationFilePath = applicationHomeSshConfigurationFilePath(
			options.runtime.homeDirectory,
			nativeAccountHomeDirectory(options.runtime.homeDirectory),
		);
		this.#sshEnvironmentTargetResolver =
			dependencies.sshEnvironmentTargetResolver ??
			new OpenSshTargetResolver(
				OpenSshTargetResolver.resolveSystemExecutablePath(
					options.platform,
					process.env['SystemRoot'],
				),
				undefined,
				defaultSshConfigurationFilePath,
			);
		this.#agentIntegrationManagementController =
			dependencies.createAgentIntegrationManagementController({
				homeDirectory: options.runtime.homeDirectory,
				platform: options.platform,
				resourceRoot: options.runtime.agentObservationAdapterDirectory,
				shellExecutable: options.runtime.terminalLaunch.executable,
			});
		this.#agentObservationRuntime =
			dependencies.createAgentObservationRuntime?.({
				allocateEventId: () => `desktop:agent:event:${dependencies.randomIdentifier()}`,
			}) ??
			new DesktopAgentObservationRuntime({
				allocateEventId: () => `desktop:agent:event:${dependencies.randomIdentifier()}`,
			});
		try {
			this.#agentHookObservationIngress =
				dependencies.createAgentHookObservationIngress?.(this.#agentObservationRuntime, {
					allocateIdentifier: dependencies.randomIdentifier,
					socketPath:
						options.platform === 'win32'
							? null
							: path.join(options.runtime.runtimeDirectory, 'agent-observation-v2.sock'),
				}) ??
				new AgentHookObservationIngress(this.#agentObservationRuntime, {
					allocateIdentifier: dependencies.randomIdentifier,
					socketPath:
						options.platform === 'win32'
							? null
							: path.join(options.runtime.runtimeDirectory, 'agent-observation-v2.sock'),
				});
		} catch {
			this.#agentHookObservationIngress = undefined;
		}
		this.#environmentRegistry = dependencies.createEnvironmentRegistry(
			options.runtime.environmentRegistryStorePath,
		);
		this.#sshEnvironmentConnections = dependencies.createSshEnvironmentConnectionOwner({
			askPassHelperEntryPath: options.sshEnvironment.askPassHelperEntryPath,
			askPassRuntimeExecutablePath: options.sshEnvironment.askPassRuntimeExecutablePath,
			defaultConfigurationFilePath: defaultSshConfigurationFilePath,
			platform: options.platform,
			registry: this.#environmentRegistry,
			release: options.sshEnvironment.release,
		});
		this.#terminalExitSpillStore =
			dependencies.createTerminalExitSpillStore?.(options.runtime.terminalExitSpillPath) ??
			new TerminalExitSpillStore(options.runtime.terminalExitSpillPath);
		this.#terminalExitPublicationLedger =
			dependencies.createTerminalExitPublicationLedger?.(
				options.runtime.terminalExitPublicationLedgerPath,
			) ?? new TerminalExitPublicationLedger(options.runtime.terminalExitPublicationLedgerPath);
		this.#windowStateOwner = new DesktopWindowStateOwner(
			dependencies.createWindowStateStore(options.runtime.windowStateStorePath),
			options.platform,
		);
		this.#workbenchDocumentStore = dependencies.createWorkbenchDocumentStore(
			options.runtime.workbenchDocumentStorePath,
			options.runtime.workbenchDocumentLegacyStorePath,
		);
		this.#terminalLaunchProfileController = dependencies.createTerminalLaunchProfileController({
			allocateProfileId: () => `profile-${dependencies.randomIdentifier()}`,
			automaticShellIntegration: options.runtime.automaticShellIntegration,
			baseLaunch: options.runtime.terminalLaunch,
			platform: options.platform,
			storePath: options.runtime.terminalLaunchProfileStorePath,
		});
	}

	get hasWindow(): boolean {
		return [...this.#windows.values()].some((record) => !record.window.isDestroyed());
	}

	get quitPrepared(): boolean {
		return this.#quitPrepared;
	}

	canCreateWindow(): boolean {
		return (
			!this.#quitPrepared &&
			!this.#shuttingDown &&
			this.#connection !== undefined &&
			this.#clientSessionCoordinator !== undefined &&
			this.#windowLayoutCoordinator !== undefined &&
			this.#options.runtime.terminalLaunch.currentDirectory !== null
		);
	}

	async createWindow(): Promise<boolean> {
		const clientSessionCoordinator = this.#clientSessionCoordinator;
		const windowLayoutCoordinator = this.#windowLayoutCoordinator;
		const defaultCwd = this.#options.runtime.terminalLaunch.currentDirectory;
		if (
			!this.canCreateWindow() ||
			clientSessionCoordinator === undefined ||
			windowLayoutCoordinator === undefined ||
			defaultCwd === null
		) {
			return false;
		}
		const terminalTitle = createOrchbeeIntl(
			resolveOrchbeeLocale(this.#dependencies.getPreferredLocales()),
		).formatMessage(workbenchTabMessages.terminalTitle);
		let created: { readonly tabId: string; readonly terminalContextId: string } | undefined;
		const mutation = await clientSessionCoordinator
			.mutateCanonicalDocument((canonical) => {
				const runtime = new WorkbenchProjectWorkspaceRuntime({
					initialDocument: canonical,
					standaloneCwd: defaultCwd,
					standaloneTerminalTitle: terminalTitle,
					terminalPresentation: {
						createOperationId: () =>
							terminalPresentationOperationId(
								`desktop:new-window:operation:${this.#dependencies.randomIdentifier()}`,
							),
						createPresentationId: () =>
							terminalPresentationId(
								`desktop:new-window:presentation:${this.#dependencies.randomIdentifier()}`,
							),
						createRemovalId: () =>
							terminalPresentationRemovalId(
								`desktop:new-window:removal:${this.#dependencies.randomIdentifier()}`,
							),
						dispatchCommands: () => undefined,
						initialPresentationId: terminalPresentationId(
							`desktop:new-window:initial:${this.#dependencies.randomIdentifier()}`,
						),
						kind: 'managed',
					},
				});
				const creation = runtime.createTerminalAtCwd({
					cwd: defaultCwd,
					groupId: null,
					source: 'programmatic',
					title: terminalTitle,
				});
				if (creation.kind !== 'created') {
					return { kind: 'rejected', reason: creation.reason };
				}
				const document = runtime.getSharedDocument(
					workbenchSharedDocumentRevision((BigInt(canonical.revision) + 1n).toString()),
				);
				const terminalContext = document.terminalContexts.find((candidate) =>
					candidate.tabCollection.tabs.some((tab) => tab.id === creation.tabId),
				);
				if (terminalContext === undefined) {
					return { kind: 'rejected', reason: 'terminal-context-unavailable' };
				}
				created = {
					tabId: creation.tabId,
					terminalContextId: terminalContext.terminalContextId,
				};
				return { document, kind: 'commit' };
			})
			.catch(() => ({ kind: 'rejected' as const, reason: 'document-mutation-failed' }));
		if (mutation.kind !== 'committed' || created === undefined) return false;
		const sourceWindowId = windowLayoutCoordinator.layout.activationOrder.at(-1);
		if (sourceWindowId === undefined) return false;
		const result = await windowLayoutCoordinator.moveTabsToNewWindow(
			sourceWindowId,
			created.terminalContextId,
			[created.tabId],
		);
		return result.kind === 'completed';
	}

	async createEnvironment(
		environment: EnvironmentRecord,
	): Promise<EnvironmentRegistryCreateResult> {
		const result = await this.#environmentRegistry.create(
			EnvironmentRecordSchema.parse(environment),
		);
		if ('environment' in result) {
			this.#registeredEnvironmentIds.add(result.environment.environmentId);
		}
		return result;
	}

	getEnvironment(
		environmentId: EnvironmentId,
	): Promise<EnvironmentRegistryReadResult<EnvironmentRecord | null>> {
		return this.#environmentRegistry.get(environmentId);
	}

	listEnvironments(): Promise<EnvironmentRegistryReadResult<readonly EnvironmentRecord[]>> {
		return this.#environmentRegistry.list();
	}

	async reloadEnvironmentRegistry(): Promise<EnvironmentRegistryInitializeResult> {
		const result = await this.#environmentRegistry.initialize();
		if (result.kind === 'ready')
			this.#replaceRegisteredEnvironmentIds(result.document.environments);
		return result;
	}

	#replaceRegisteredEnvironmentIds(environments: readonly EnvironmentRecord[]): void {
		this.#registeredEnvironmentIds.clear();
		for (const environment of environments) {
			this.#registeredEnvironmentIds.add(environment.environmentId);
		}
	}

	connectSshEnvironment(environmentId: EnvironmentId): Promise<DesktopSshEnvironmentConnectResult> {
		return this.#sshEnvironmentConnections.connect(environmentId);
	}

	async disconnectSshEnvironment(
		environmentId: EnvironmentId,
	): Promise<DesktopSshEnvironmentDisconnectResult> {
		let retirementFailure: unknown;
		try {
			await this.#sshEnvironmentTerminalRuntimeOwner?.retire(environmentId);
		} catch (error: unknown) {
			retirementFailure = error;
		}
		const result = await this.#sshEnvironmentConnections.disconnect(environmentId);
		if (retirementFailure !== undefined) throw retirementFailure;
		return result;
	}

	inspectSshEnvironmentProjectRoot(
		environmentId: EnvironmentId,
		requestedPath: string,
		signal?: AbortSignal,
	): Promise<SshProjectRootInspectionOutcome> {
		return this.#sshEnvironmentConnections.inspectProjectRoot(environmentId, requestedPath, signal);
	}

	listSshEnvironmentProjectDirectory(
		environmentId: EnvironmentId,
		requestedPath: string | null,
		signal?: AbortSignal,
	): Promise<SshProjectRootDirectoryListingOutcome> {
		return this.#sshEnvironmentConnections.listProjectDirectory(
			environmentId,
			requestedPath,
			signal,
		);
	}

	openSshEnvironmentTerminalHost(
		environmentId: EnvironmentId,
	): Promise<SshTerminalHostConnectionOutcome> {
		return this.#sshEnvironmentConnections.openTerminalHost(environmentId);
	}

	admitSshEnvironmentRemoteProject(
		environmentId: EnvironmentId,
		requestedPath: string,
		signal?: AbortSignal,
	): Promise<DesktopSshEnvironmentRemoteProjectAdmissionResult> {
		const coordinator = this.#clientSessionCoordinator;
		if (coordinator === undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'workbench-unavailable' });
		}
		return admitDesktopSshEnvironmentRemoteProject({
			allocateDirectoryToken: this.#dependencies.randomIdentifier,
			connections: this.#sshEnvironmentConnections,
			coordinator,
			environmentId,
			registry: this.#environmentRegistry,
			requestedPath,
			...(signal === undefined ? {} : { signal }),
		});
	}

	attachSshEnvironmentRemoteProjectFolder(
		environmentId: EnvironmentId,
		projectId: string,
		requestedPath: string,
		signal?: AbortSignal,
	): Promise<DesktopSshEnvironmentRemoteProjectFolderAttachmentResult> {
		const coordinator = this.#clientSessionCoordinator;
		if (coordinator === undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'workbench-unavailable' });
		}
		return attachDesktopSshEnvironmentRemoteProjectFolder({
			allocateDirectoryToken: this.#dependencies.randomIdentifier,
			connections: this.#sshEnvironmentConnections,
			coordinator,
			environmentId,
			projectId,
			registry: this.#environmentRegistry,
			requestedPath,
			...(signal === undefined ? {} : { signal }),
		});
	}

	createSshEnvironmentRemoteTerminal(
		environmentId: EnvironmentId,
		projectId: string,
		workspaceId: string,
		signal?: AbortSignal,
	): Promise<DesktopSshEnvironmentRemoteTerminalResult> {
		const coordinator = this.#clientSessionCoordinator;
		const runtimeOwner = this.#sshEnvironmentTerminalRuntimeOwner;
		if (coordinator === undefined || runtimeOwner === undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'workbench-unavailable' });
		}
		const intl = createOrchbeeIntl(resolveOrchbeeLocale(this.#dependencies.getPreferredLocales()));
		return createDesktopSshEnvironmentRemoteTerminal({
			allocateIdentifier: this.#dependencies.randomIdentifier,
			connections: this.#sshEnvironmentConnections,
			coordinator,
			environmentId,
			projectId,
			registry: this.#environmentRegistry,
			runtimeOwner,
			...(signal === undefined ? {} : { signal }),
			terminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
			workspaceId,
		});
	}

	createSshEnvironmentRemoteTerminalForWorkspace(
		projectId: string,
		workspaceId: string,
		signal?: AbortSignal,
	): Promise<DesktopSshEnvironmentRemoteTerminalResult> {
		const coordinator = this.#clientSessionCoordinator;
		if (coordinator === undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'workbench-unavailable' });
		}
		const document = coordinator.canonicalDocument;
		if (document === null) {
			return Promise.resolve({ kind: 'failed', reason: 'workbench-missing' });
		}
		const target = resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
			document,
			projectId,
			workspaceId,
		);
		if (target.kind === 'rejected') {
			return Promise.resolve({ kind: 'failed', reason: target.reason });
		}
		return this.createSshEnvironmentRemoteTerminal(
			target.environmentId,
			projectId,
			workspaceId,
			signal,
		);
	}

	#isCurrentSshEnvironmentTerminalTarget(
		environmentId: EnvironmentId,
		presentationId: TerminalPresentationId,
	): boolean {
		const document = this.#clientSessionCoordinator?.canonicalDocument;
		if (document === null || document === undefined) return false;
		try {
			const target = resolveDesktopTerminalPresentationEffectTarget(document, presentationId);
			return (
				target.kind === 'ssh-environment' &&
				EnvironmentIdSchema.parse(target.environmentId) === EnvironmentIdSchema.parse(environmentId)
			);
		} catch {
			return false;
		}
	}

	getSshEnvironmentConnection(
		environmentId: EnvironmentId,
	): DesktopSshEnvironmentConnectionReadResult {
		return this.#sshEnvironmentConnections.observation(environmentId);
	}

	subscribeSshEnvironmentConnection(
		environmentId: EnvironmentId,
		listener: (observation: SshEnvironmentConnectionObservation) => void,
	): (() => void) | null {
		return this.#sshEnvironmentConnections.subscribe(environmentId, listener);
	}

	createSshEnvironmentConnectionState(
		configuration: SshEnvironmentConnectionConfiguration,
	): SshEnvironmentConnectionState {
		return createSshEnvironmentConnectionStateModel(configuration);
	}

	observeSshEnvironmentConnection(
		state: SshEnvironmentConnectionState,
	): SshEnvironmentConnectionObservation {
		return observeSshEnvironmentConnectionModel(state);
	}

	resolveSshEnvironmentTarget(
		target: SshEnvironmentTargetReference,
	): Promise<OpenSshTargetResolutionResult> {
		return this.#sshEnvironmentTargetResolver.resolve(target);
	}

	transitionSshEnvironmentConnection(
		state: SshEnvironmentConnectionState,
		event: SshEnvironmentConnectionEvent,
	): SshEnvironmentConnectionTransition {
		return transitionSshEnvironmentConnectionModel(state, event);
	}

	focusWindow(): void {
		const window =
			this.#window === undefined || this.#window.isDestroyed()
				? this.#mostRecentNativeWindow()
				: this.#window;
		if (window === undefined || window.isDestroyed()) return;
		if (window.isMinimized()) window.restore();
		window.show();
		window.focus();
	}

	getWindowState(): 'full-screen' | 'maximized' | 'normal' | 'unavailable' {
		const window = this.#window;
		if (window === undefined || window.isDestroyed()) return 'unavailable';
		if (window.isFullScreen()) return 'full-screen';
		return window.isMaximized() ? 'maximized' : 'normal';
	}

	toggleWindowFullScreen(): boolean {
		const window = this.#window;
		if (window === undefined || window.isDestroyed()) return false;
		window.setFullScreen(!window.isFullScreen());
		return true;
	}

	toggleWindowMaximized(): boolean {
		const window = this.#window;
		if (window === undefined || window.isDestroyed() || window.isFullScreen()) return false;
		if (window.isMaximized()) window.unmaximize();
		else window.maximize();
		return true;
	}

	requestExit(reason: DesktopApplicationExitReason): Promise<DesktopApplicationLifetimeExitResult> {
		if (this.#exitRequest !== undefined) return this.#exitRequest;
		const broker = this.#applicationLifetimeBroker;
		if (broker === undefined) return Promise.resolve({ kind: 'unavailable' });
		const requestId = DesktopApplicationExitRequestIdSchema.parse(
			`desktop:exit:${this.#dependencies.randomIdentifier()}`,
		);
		const request = broker.requestExit(requestId, reason);
		this.#exitRequest = request;
		void request.then((result) => {
			if (this.#exitRequest === request && result.kind !== 'ready') {
				this.#exitRequest = undefined;
			}
		});
		return request;
	}

	async requestExitAndQuit(reason: DesktopApplicationExitReason): Promise<void> {
		if (reason === 'development-host-restart' && this.#exitRequest === undefined) {
			this.#developmentHostRestartPresentationIds = undefined;
		}
		if (this.#applicationLifetimeBroker === undefined && !this.#shuttingDown) {
			if (!this.hasWindow) await this.openWindow();
			await this.#documentTransition;
		}
		const requestedBroker = this.#applicationLifetimeBroker;
		let result = await this.requestExit(reason);
		if (
			result.kind === 'unavailable' &&
			reason === 'development-host-restart' &&
			!this.#shuttingDown &&
			(await this.#recoverApplicationLifetimeBroker(requestedBroker))
		) {
			result = await this.requestExit(reason);
		}
		if (result.kind !== 'ready' || this.#quitPrepared) return;
		this.#quitPrepared = true;
		await this.shutdown();
		this.#dependencies.quitApplication();
	}

	async start(): Promise<void> {
		if (this.#connection !== undefined) throw new Error('Desktop application is already started.');
		const environmentRegistryInitialization = await this.#environmentRegistry.initialize();
		if (environmentRegistryInitialization.kind === 'unavailable') {
			throw new Error(
				`Desktop Environment registry is unavailable: ${environmentRegistryInitialization.reason}.`,
			);
		}
		this.#replaceRegisteredEnvironmentIds(environmentRegistryInitialization.document.environments);
		const connection = await this.#dependencies.connectHost(this.#options.runtime);
		this.#connection = connection;
		try {
			await connection.negotiate(ClientIdSchema.parse(this.#dependencies.randomIdentifier()));
			const agentIntegrationInitialization =
				this.#agentIntegrationManagementController.initialize();
			const observationIngress = this.#agentHookObservationIngress;
			if (observationIngress !== undefined) {
				try {
					await observationIngress.start();
				} catch {
					await observationIngress.dispose().catch(() => undefined);
					if (this.#agentHookObservationIngress === observationIngress) {
						this.#agentHookObservationIngress = undefined;
					}
				}
			}
			const terminalDefaultSettingsController =
				this.#dependencies.createTerminalDefaultSettingsController({
					runtimeConfiguration: this.#options.runtime.terminalConfiguration,
					setHostDefaults: async (configuration) => {
						const accepted = await connection.requestControl(
							MessageKind.setTerminalDefaults,
							{ configuration },
							TerminalDefaultsAcceptedSchema,
						);
						return accepted.configuration;
					},
					storePath: this.#options.runtime.terminalDefaultSettingsStorePath,
				});
			await terminalDefaultSettingsController.initialize();
			this.#terminalDefaultSettingsController = terminalDefaultSettingsController;
			const publicationLedgerLoad = await this.#terminalExitPublicationLedger.load();
			if (publicationLedgerLoad.kind !== 'ready') {
				throw new Error(
					`Terminal exit publication ledger is unavailable: ${publicationLedgerLoad.reason}.`,
				);
			}
			const spillLoad = await this.#terminalExitSpillStore.load(
				this.#terminalExitPublicationLedger,
			);
			if (spillLoad.kind !== 'ready') {
				throw new Error(`Terminal exit spill store is unavailable: ${spillLoad.reason}.`);
			}
			this.#terminalExitSpillReady = true;
			await this.#terminalLaunchProfileController.initialize();
			await agentIntegrationInitialization;
			const terminalRuntime = this.#dependencies.createTerminalRuntime(
				connection,
				this.#options.runtime,
				this.#dependencies.randomIdentifier,
				this.#dependencies.createMessageChannel,
				this.#dependencies.createBroker,
				() => this.#terminalLaunchProfileController.resolveDefaultLaunch(),
				{
					resolve: (presentationId, creationId) =>
						this.#agentHookObservationIngress?.environmentOverlayFor(presentationId, creationId) ??
						[],
					retire: (presentationId) => {
						this.#agentHookObservationIngress?.retirePresentation(presentationId);
						this.#agentObservationRuntime.retireTerminalSignalPresentation?.(presentationId);
					},
				},
			);
			this.#terminalRuntime = terminalRuntime;
			terminalRuntime.setIdentityObservedListener?.(this.#handleTerminalIdentityObserved);
			this.#removeHostListener = connection.onEnvelope(this.#handleHostEnvelope);
			const initialization = await terminalRuntime.initialize();
			if (initialization.kind !== 'ready') {
				throw new Error(
					`Desktop terminal runtime initialization stopped with ${initialization.kind}.`,
				);
			}
			const sshEnvironmentTerminalRuntimeOwner = new DesktopSshEnvironmentTerminalRuntimeOwner({
				connections: this.#sshEnvironmentConnections,
				createObservation: (environmentId) => {
					const resources = resolveDesktopSshEnvironmentTerminalRuntimeResources(
						this.#options.runtime,
						environmentId,
					);
					const spillStore =
						this.#dependencies.createTerminalExitSpillStore?.(
							resources.configuration.terminalExitSpillPath,
						) ?? new TerminalExitSpillStore(resources.configuration.terminalExitSpillPath);
					const publicationLedger =
						this.#dependencies.createTerminalExitPublicationLedger?.(
							resources.configuration.terminalExitPublicationLedgerPath,
						) ??
						new TerminalExitPublicationLedger(
							resources.configuration.terminalExitPublicationLedgerPath,
						);
					return new DesktopSshEnvironmentTerminalObservationOwner({
						createExitEvent: toDocumentExit,
						createFailureEvent: toDocumentFailure,
						environmentId,
						publication: {
							commitExternalEvent: (event) =>
								this.#terminalIntentReconciler?.commitExternalEvent(event) ?? false,
							hasExternalEvent: (event) =>
								this.#terminalIntentReconciler?.hasExternalEvent(event) ?? false,
							isCurrentPresentationTarget: (targetEnvironmentId, presentationId) =>
								this.#isCurrentSshEnvironmentTerminalTarget(targetEnvironmentId, presentationId),
							publishTerminalEvent: (event) =>
								this.#clientSessionCoordinator?.publishTerminalEvent(event) ?? false,
							publishTerminalMetadata: (metadata) =>
								this.#clientSessionCoordinator?.publishTerminalMetadata(metadata) ?? false,
							recordExternalEvent: (event) =>
								this.#terminalIntentReconciler?.recordExternalEvent(event) ?? false,
							reserveExternalEvent: (event) =>
								this.#terminalIntentReconciler?.reserveExternalEvent(event) ?? false,
						},
						publicationLedger,
						spillStore,
						trace: (reason) => this.#writeCopyPathTrace(reason),
					});
				},
				createRuntime: (environmentId, remoteHost) => {
					const resources = resolveDesktopSshEnvironmentTerminalRuntimeResources(
						this.#options.runtime,
						environmentId,
					);
					return this.#dependencies.createTerminalRuntime(
						remoteHost,
						resources.configuration,
						this.#dependencies.randomIdentifier,
						this.#dependencies.createMessageChannel,
						this.#dependencies.createBroker,
						() => resources.launch,
					);
				},
			});
			this.#sshEnvironmentTerminalRuntimeOwner = sshEnvironmentTerminalRuntimeOwner;
			const defaultCwd = this.#options.runtime.terminalLaunch.currentDirectory;
			if (defaultCwd === null) {
				throw new Error('Desktop terminal runtime requires a default CWD.');
			}
			const initialPresentationId = terminalPresentationId(
				`desktop:presentation:${this.#dependencies.randomIdentifier()}`,
			);
			const intl = createOrchbeeIntl(
				resolveOrchbeeLocale(this.#dependencies.getPreferredLocales()),
			);
			const createOperationId = () =>
				terminalPresentationOperationId(
					`desktop:operation:${this.#dependencies.randomIdentifier()}`,
				);
			const createPresentationId = () =>
				terminalPresentationId(`desktop:presentation:${this.#dependencies.randomIdentifier()}`);
			const createRemovalId = () =>
				terminalPresentationRemovalId(`desktop:removal:${this.#dependencies.randomIdentifier()}`);
			const initializationDocument = createDesktopWorkbenchInitializationDocument({
				createOperationId,
				createPresentationId,
				createRemovalId,
				defaultCwd,
				initialPresentationId,
				recoveredPresentations: terminalRuntime.getRecoveredPresentations(),
				terminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
			});
			const terminalIntentReconciler = this.#dependencies.createTerminalIntentReconciler({
				createOperationId,
				execute: async (command, target) => {
					if (target.kind === 'local') {
						return { event: await terminalRuntime.execute(command), kind: 'settled' };
					}
					return sshEnvironmentTerminalRuntimeOwner.execute(
						EnvironmentIdSchema.parse(target.environmentId),
						command,
					);
				},
				getTerminalOutcome: (presentationId, target) => {
					const terminalFailure =
						target.kind === 'local'
							? terminalRuntime.getTerminalFailure(presentationId)
							: sshEnvironmentTerminalRuntimeOwner.getTerminalFailure(
									EnvironmentIdSchema.parse(target.environmentId),
									presentationId,
								);
					if (terminalFailure !== null) {
						return toDocumentFailure(presentationId, terminalFailure);
					}
					const terminalExit =
						target.kind === 'local'
							? terminalRuntime.getDurableTerminalExit(presentationId)
							: sshEnvironmentTerminalRuntimeOwner.getDurableTerminalExit(
									EnvironmentIdSchema.parse(target.environmentId),
									presentationId,
								);
					return terminalExit === null ? null : toDocumentExit(presentationId, terminalExit);
				},
			});
			const clientSessionCoordinator = this.#dependencies.createClientSessionCoordinator({
				createClientSessionId: () => `desktop:client:${this.#dependencies.randomIdentifier()}`,
				defaultCwd,
				getTerminalEvents: () => terminalIntentReconciler.getTerminalEvents(),
				initialPresentationId,
				initializationDocument,
				onCanonicalDocumentChanged: async (document) => {
					const coordinator = this.#windowLayoutCoordinator;
					if (coordinator === undefined) return;
					await coordinator.reconcileDocument(
						toDesktopWindowLayoutDocument(document),
						this.#fallbackWindowBounds(),
					);
				},
				reconcileDocument: async (document, publish) => {
					await terminalIntentReconciler.reconcileDocument(document, (event) => {
						if (event.type === 'terminal-exited') {
							let target;
							try {
								target = resolveDesktopTerminalPresentationEffectTarget(
									document,
									event.presentationId,
								);
							} catch {
								return false;
							}
							return target.kind === 'local'
								? this.#commitReconciledTerminalExit(terminalRuntime, event, publish)
								: sshEnvironmentTerminalRuntimeOwner.commitReconciledExit(
										EnvironmentIdSchema.parse(target.environmentId),
										event,
										publish,
									);
						}
						publish(event);
						return true;
					});
					await Promise.all(
						[...this.#registeredEnvironmentIds].map(async (environmentId) =>
							sshEnvironmentTerminalRuntimeOwner.retryPending(environmentId),
						),
					);
				},
				reconcileTerminalArchives: (document) =>
					terminalRuntime.reconcileArchives(
						document === null
							? new Set<TerminalPresentationId>()
							: collectDocumentArchivePresentationIds(document),
					),
				store: this.#workbenchDocumentStore,
				terminalHostName: this.#options.runtime.terminalHostName,
				validateDocument: (document) =>
					terminalIntentReconciler.validateDocument(document) &&
					validateWorkbenchEnvironmentReferences(document, this.#registeredEnvironmentIds),
			});
			this.#terminalIntentReconciler = terminalIntentReconciler;
			this.#clientSessionCoordinator = clientSessionCoordinator;
			const coordinatorStart = await clientSessionCoordinator.start();
			if (coordinatorStart.kind !== 'ready') {
				throw new Error(
					`Desktop client session coordinator stopped with ${coordinatorStart.reason}.`,
				);
			}
			const displayTopology = this.#dependencies.getWindowDisplayTopology();
			const legacyLaunchState = await this.#windowStateOwner.loadLaunchState(displayTopology);
			const canonicalDocument =
				clientSessionCoordinator.canonicalDocument ?? initializationDocument;
			const windowLayoutStore =
				this.#dependencies.createWindowLayoutStore?.(this.#options.runtime.windowLayoutStorePath) ??
				new FileDesktopWindowLayoutStore(this.#options.runtime.windowLayoutStorePath);
			const windowLayoutOwner = await loadDesktopWindowLayoutOwner(
				windowLayoutStore,
				toDesktopWindowLayoutDocument(canonicalDocument),
				completeDesktopWindowBounds(legacyLaunchState.bounds, displayTopology),
				legacyLaunchState.maximized,
			);
			this.#windowLayoutOwner = windowLayoutOwner;
			const windowLayoutHost: DesktopWindowLayoutHost = {
				activateWindow: this.#activateNativeWindow,
				discardPreparedWindow: this.#discardPreparedNativeWindow,
				getNewWindowBounds: this.#getNewWindowBounds,
				getWindowIdAtPoint: this.#getWindowIdAtPoint,
				prepareWindow: this.#prepareNativeWindow,
				retireWindow: this.#retireNativeWindow,
			};
			this.#windowLayoutCoordinator = new DesktopWindowLayoutCoordinator(
				windowLayoutOwner,
				toDesktopWindowLayoutDocument(canonicalDocument),
				windowLayoutHost,
			);
			this.#sshProjectEnrollmentRuntime = new DesktopSshProjectEnrollmentRuntime({
				admitProject: (environmentId, requestedPath, signal) =>
					this.admitSshEnvironmentRemoteProject(environmentId, requestedPath, signal),
				attachProjectFolder: (environmentId, projectId, requestedPath, signal) =>
					this.attachSshEnvironmentRemoteProjectFolder(
						environmentId,
						projectId,
						requestedPath,
						signal,
					),
				allocateIdentifier: this.#dependencies.randomIdentifier,
				connections: {
					claimInteractionLease: (environmentId, interaction) =>
						this.#sshEnvironmentConnections.claimInteractionLease(environmentId, interaction),
					connect: (environmentId) => this.#sshEnvironmentConnections.connect(environmentId),
					disconnect: (environmentId) => this.disconnectSshEnvironment(environmentId),
					inspectProjectRoot: (environmentId, requestedPath, signal) =>
						this.#sshEnvironmentConnections.inspectProjectRoot(
							environmentId,
							requestedPath,
							signal,
						),
					listProjectDirectory: (environmentId, requestedPath, signal) =>
						this.#sshEnvironmentConnections.listProjectDirectory(
							environmentId,
							requestedPath,
							signal,
						),
					observation: (environmentId) =>
						this.#sshEnvironmentConnections.observation(environmentId),
					subscribe: (environmentId, listener) =>
						this.#sshEnvironmentConnections.subscribe(environmentId, listener),
				},
				discoverHostAliases: () =>
					discoverOpenSshHostAliases(
						path.join(this.#options.runtime.homeDirectory, '.ssh', 'config'),
						this.#options.runtime.homeDirectory,
					),
				getDocument: () => clientSessionCoordinator.canonicalDocument,
				registry: {
					create: (environment) => this.createEnvironment(environment),
					list: () => this.listEnvironments(),
				},
				resolveTarget: (target) => this.#sshEnvironmentTargetResolver.resolve(target),
			});
			const semanticCommandCoordinatorGeneration = SemanticCommandCoordinatorGenerationSchema.parse(
				`desktop:semantic-generation:${this.#dependencies.randomIdentifier()}`,
			);
			const semanticCommandService = new DesktopSemanticCommandService(
				new DesktopSemanticCommandCatalog([
					...createDesktopAgentReadOperations({
						coordinatorGeneration: semanticCommandCoordinatorGeneration,
						cursorSecret: randomBytes(32),
						source: this.#agentObservationRuntime,
					}),
					...createDesktopWorkbenchReadOperations({
						coordinatorGeneration: semanticCommandCoordinatorGeneration,
						cursorSecret: randomBytes(32),
						getDocument: () => clientSessionCoordinator.canonicalDocument,
					}),
					createDesktopWorkbenchTerminalCreateOperation({
						createOperationId,
						createPresentationId,
						createRemovalId,
						defaultCwd,
						initialPresentationId,
						mutateCanonicalDocument: (mutation, afterCommit) =>
							clientSessionCoordinator.mutateCanonicalDocument(mutation, afterCommit),
						terminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
					}),
				]),
				{
					coordinatorGeneration: semanticCommandCoordinatorGeneration,
					createExecutionIdentity: () =>
						SemanticCommandExecutionIdentitySchema.parse(
							`desktop:semantic-execution:${this.#dependencies.randomIdentifier()}`,
						),
				},
			);
			this.#semanticCommandService = semanticCommandService;
			const semanticCommandLocalEndpoint = this.#dependencies.createSemanticCommandLocalEndpoint({
				addonPath: this.#options.runtime.semanticCommandEndpointAddonPath,
				coordinatorGeneration: semanticCommandCoordinatorGeneration,
				endpointId: SemanticCommandLocalEndpointIdSchema.parse(
					this.#dependencies.randomIdentifier(),
				),
				runtimeDirectory: this.#options.runtime.runtimeDirectory,
				service: semanticCommandService,
				workerEntryPath: this.#options.semanticCommandEndpointWorkerPath,
			});
			this.#semanticCommandLocalEndpoint = semanticCommandLocalEndpoint;
			await semanticCommandLocalEndpoint.start();
			const projectWorktreeRuntime = this.#dependencies.createProjectWorktreeRuntime({
				allocateIdentifier: this.#dependencies.randomIdentifier,
				coordinator: clientSessionCoordinator,
				defaultCwd,
				managedRoot: this.#options.runtime.managedWorktreeRoot,
				operationStorePath: this.#options.runtime.projectWorktreeOperationStorePath,
				terminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
			});
			this.#projectWorktreeRuntime = projectWorktreeRuntime;
			if ((await projectWorktreeRuntime.start()) !== 'ready') {
				throw new Error('Desktop Project Worktree control is unavailable.');
			}
			this.#publishAllPendingTerminalMetadata();
			this.#promoteSpilledTerminalExits();
			this.#scheduleTerminalExitRetry();
			if (this.#options.runtime.browserAccess.kind === 'enabled') {
				this.#browserAccessRuntime = this.#dependencies.createBrowserAccessRuntime(
					{
						gatewayEntryPath: this.#options.runtime.browserAccess.gatewayEntryPath,
						personalAccessTokenStorePath:
							this.#options.runtime.browserAccess.personalAccessTokenStorePath,
						rendererRoot: this.#options.runtime.browserAccess.rendererRoot,
						serverEntryPath: this.#options.runtime.browserAccess.serverEntryPath,
					},
					() => this.#openGatewayClient(),
				);
				await this.#browserAccessRuntime.initialize();
			}
			await this.openWindow();
		} catch (error) {
			this.#removeHostListener?.();
			this.#removeHostListener = undefined;
			let cleanupFailure: unknown;
			try {
				await this.#disposeTerminalExitRetryOwner();
			} catch (cleanupError: unknown) {
				cleanupFailure = cleanupError;
			}
			try {
				await this.#disposeBrowserAccessResources();
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				await this.#disposeSemanticCommandResources();
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				const projectWorktreeRuntime = this.#projectWorktreeRuntime;
				await projectWorktreeRuntime?.dispose();
				if (this.#projectWorktreeRuntime === projectWorktreeRuntime) {
					this.#projectWorktreeRuntime = undefined;
				}
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			this.#sshProjectEnrollmentRuntime?.dispose();
			this.#sshProjectEnrollmentRuntime = undefined;
			try {
				this.#windowLayoutCoordinator?.dispose();
				this.#windowLayoutCoordinator = undefined;
				await this.#windowLayoutOwner?.flush();
				this.#windowLayoutOwner?.dispose();
				this.#windowLayoutOwner = undefined;
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				const coordinator = this.#clientSessionCoordinator;
				await coordinator?.dispose();
				if (this.#clientSessionCoordinator === coordinator) {
					this.#clientSessionCoordinator = undefined;
					this.#terminalIntentReconciler = undefined;
				}
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				const remoteTerminalRuntimeOwner = this.#sshEnvironmentTerminalRuntimeOwner;
				await remoteTerminalRuntimeOwner?.dispose();
				if (this.#sshEnvironmentTerminalRuntimeOwner === remoteTerminalRuntimeOwner) {
					this.#sshEnvironmentTerminalRuntimeOwner = undefined;
				}
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				const terminalRuntime = this.#terminalRuntime;
				await terminalRuntime?.dispose();
				if (this.#terminalRuntime === terminalRuntime) this.#terminalRuntime = undefined;
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			try {
				const observationIngress = this.#agentHookObservationIngress;
				await observationIngress?.dispose();
				if (this.#agentHookObservationIngress === observationIngress) {
					this.#agentHookObservationIngress = undefined;
				}
			} catch (cleanupError: unknown) {
				cleanupFailure ??= cleanupError;
			}
			connection.close();
			this.#connection = undefined;
			throw cleanupFailure ?? error;
		}
	}

	async openWindow(): Promise<void> {
		if (this.hasWindow) {
			this.focusWindow();
			return;
		}
		const coordinator = this.#windowLayoutCoordinator;
		if (
			this.#connection === undefined ||
			this.#terminalRuntime === undefined ||
			coordinator === undefined
		) {
			throw new Error('Desktop application must start before opening a window.');
		}
		this.#documentFailure = undefined;
		for (const layoutWindow of coordinator.layout.windows) {
			if (!(await this.#createNativeWindow(layoutWindow, true))) {
				throw new Error(`Desktop native window ${layoutWindow.windowId} could not be created.`);
			}
		}
		const loading = [...this.#windows.values()].flatMap((record) => {
			const operation = this.#loadNativeWindow(record);
			return operation === undefined ? [] : [operation];
		});
		await Promise.all(loading);
		const activeWindowId = coordinator.layout.activationOrder.at(-1);
		if (activeWindowId !== undefined) this.#activateNativeWindow(activeWindowId);
		await this.#documentTransition;
		if (this.#documentFailure !== undefined) throw this.#documentFailure;
	}

	readonly #prepareNativeWindow = async (
		layoutWindow: DesktopWindowLayoutWindow,
	): Promise<boolean> => {
		if (!(await this.#createNativeWindow(layoutWindow, false))) return false;
		const record = this.#windows.get(layoutWindow.windowId);
		if (record === undefined) return false;
		try {
			await this.#loadNativeWindow(record);
			return true;
		} catch {
			this.#retireNativeWindow(layoutWindow.windowId);
			await this.#documentTransition;
			return false;
		}
	};

	readonly #createNativeWindow = async (
		layoutWindow: DesktopWindowLayoutWindow,
		layoutCommitted: boolean,
	): Promise<boolean> => {
		if (this.#windows.has(layoutWindow.windowId) || this.#shuttingDown) return false;
		const rendererUrl = new URL(this.#options.rendererUrl);
		if (!isPermittedDesktopRendererUrl(rendererUrl)) return false;
		const launchState = resolveDesktopWindowLaunchState(
			{ bounds: layoutWindow.bounds, maximized: layoutWindow.maximized, version: 1 },
			this.#dependencies.getWindowDisplayTopology(),
		);
		let window: BrowserWindow;
		try {
			window = this.#dependencies.createWindow({
				backgroundColor: terminalRgbCss(
					this.#options.runtime.terminalLaunch.terminalProfile.defaultBackgroundRgb,
				),
				...launchState.bounds,
				show: false,
				...(this.#options.platform === 'darwin'
					? { trafficLightPosition: MACOS_TRAFFIC_LIGHT_POSITION }
					: { titleBarOverlay: true }),
				titleBarStyle: 'hidden',
				webPreferences: {
					allowRunningInsecureContent: false,
					contextIsolation: true,
					nodeIntegration: false,
					preload: this.#options.preloadPath,
					sandbox: true,
					spellcheck: false,
					webSecurity: true,
				},
			});
		} catch {
			return false;
		}
		const record: DesktopNativeWindowRecord = {
			geometryCaptureScheduled: false,
			layoutCommitted,
			loading: undefined,
			pendingRendererGeneration: undefined,
			retiring: false,
			window,
			windowId: layoutWindow.windowId,
		};
		this.#windows.set(record.windowId, record);
		this.#windowIds.set(window, record.windowId);
		this.#window ??= window;
		if (this.#windowStateAttachment === undefined) {
			this.#windowStateAttachment = this.#windowStateOwner.attach(window);
			this.#windowStateAttachmentWindow = window;
		}
		if (launchState.maximized) window.maximize();
		let loadingGeneration = 0;
		window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
		window.webContents.on('will-navigate', (event, navigationUrl) => {
			if (!isSameDesktopRendererOrigin(navigationUrl, rendererUrl)) event.preventDefault();
		});
		window.webContents.on('did-start-navigation', (navigation) => {
			if (!startsDesktopRendererDocument(navigation)) return;
			loadingGeneration = this.#beginRendererDocument(window);
		});
		window.webContents.on('did-finish-load', () => {
			const generation =
				loadingGeneration === 0 ? this.#beginRendererDocument(window) : loadingGeneration;
			loadingGeneration = 0;
			if (record.layoutCommitted) this.#queueRendererDocument(window, generation);
			else record.pendingRendererGeneration = generation;
		});
		window.webContents.on('render-process-gone', () => {
			this.#beginRendererDocument(window);
			void this.#documentTransition.then(() => {
				if (this.#windowIds.has(window) && !window.isDestroyed() && !this.#shuttingDown) {
					window.reload();
				}
			});
		});
		window.on('close', (event) => {
			if (record.retiring || this.#quitPrepared || this.#shuttingDown) return;
			event.preventDefault();
			void this.#requestNativeWindowClose(record.windowId);
		});
		window.on('focus', () => {
			this.#window = window;
			this.#selectApplicationLifetimeBroker(window);
			void this.#windowLayoutCoordinator?.focusWindow(record.windowId);
		});
		const scheduleGeometryCapture = () => this.#scheduleWindowGeometryCapture(record);
		window.on('maximize', scheduleGeometryCapture);
		window.on('move', scheduleGeometryCapture);
		window.on('resize', scheduleGeometryCapture);
		window.on('unmaximize', scheduleGeometryCapture);
		window.once('ready-to-show', () => window.show());
		window.once('closed', () => {
			if (
				this.#windowStateAttachmentWindow === window &&
				this.#windowStateAttachment !== undefined
			) {
				this.#windowStateAttachment.dispose();
				this.#windowStateAttachment = undefined;
				this.#windowStateAttachmentWindow = undefined;
			}
			this.#writeCopyPathTrace('window-closed');
			this.#beginRendererDocument(window);
			this.#windows.delete(record.windowId);
			this.#windowIds.delete(window);
			if (this.#window === window) {
				this.#window = this.#mostRecentNativeWindow();
				this.#selectApplicationLifetimeBroker(this.#window);
			}
		});
		return true;
	};

	#loadNativeWindow(record: DesktopNativeWindowRecord): Promise<void> | undefined {
		if (record.window.isDestroyed()) return undefined;
		if (record.loading !== undefined) return record.loading;
		const operation = record.window.loadURL(this.#options.rendererUrl).then(async () => {
			await this.#documentTransition;
			if (this.#documentFailure !== undefined) throw this.#documentFailure;
		});
		record.loading = operation.catch((error: unknown) => {
			record.loading = undefined;
			throw error;
		});
		return record.loading;
	}

	readonly #activateNativeWindow = (windowId: string): void => {
		const record = this.#windows.get(windowId);
		if (record === undefined || record.window.isDestroyed()) return;
		record.layoutCommitted = true;
		const pendingRendererGeneration = record.pendingRendererGeneration;
		record.pendingRendererGeneration = undefined;
		if (pendingRendererGeneration !== undefined) {
			this.#queueRendererDocument(record.window, pendingRendererGeneration);
		}
		this.#window = record.window;
		this.#selectApplicationLifetimeBroker(record.window);
		const loading = this.#loadNativeWindow(record);
		void loading
			?.then(async () => {
				await this.#documentTransition;
				if (this.#documentFailure !== undefined) throw this.#documentFailure;
				if (record.window.isDestroyed()) return;
				if (record.window.isMinimized()) record.window.restore();
				record.window.show();
				record.window.focus();
			})
			.catch(() => undefined);
	};

	readonly #discardPreparedNativeWindow = async (windowId: string): Promise<void> => {
		this.#retireNativeWindow(windowId);
		await this.#documentTransition;
	};

	readonly #retireNativeWindow = (windowId: string): void => {
		const record = this.#windows.get(windowId);
		if (record === undefined || record.window.isDestroyed()) return;
		record.retiring = true;
		record.window.destroy();
	};

	async #requestNativeWindowClose(windowId: string): Promise<void> {
		const coordinator = this.#windowLayoutCoordinator;
		if (coordinator === undefined) return;
		if (coordinator.layout.windows.length === 1) {
			await this.requestExitAndQuit('user-quit');
			return;
		}
		const result = await coordinator.requestWindowClose(windowId);
		if (result.kind === 'last-window') await this.requestExitAndQuit('user-quit');
	}

	#scheduleWindowGeometryCapture(record: DesktopNativeWindowRecord): void {
		if (record.geometryCaptureScheduled || record.retiring || record.window.isDestroyed()) return;
		record.geometryCaptureScheduled = true;
		queueMicrotask(() => {
			record.geometryCaptureScheduled = false;
			if (record.retiring || record.window.isDestroyed()) return;
			void this.#windowLayoutCoordinator?.captureGeometry(
				record.windowId,
				record.window.getNormalBounds(),
				record.window.isMaximized(),
			);
		});
	}

	readonly #getWindowIdAtPoint = (point: Readonly<{ x: number; y: number }>): string | null => {
		for (const [windowId, record] of [...this.#windows].reverse()) {
			if (record.retiring || record.window.isDestroyed()) continue;
			const bounds = record.window.getNormalBounds();
			if (
				point.x >= bounds.x &&
				point.x < bounds.x + bounds.width &&
				point.y >= bounds.y &&
				point.y < bounds.y + bounds.height
			) {
				return windowId;
			}
		}
		return null;
	};

	readonly #getNewWindowBounds = (
		sourceWindowId: string,
		point: Readonly<{ x: number; y: number }> | undefined,
	): ReturnType<DesktopNativeWindowRecord['window']['getNormalBounds']> => {
		const source =
			this.#windows.get(sourceWindowId)?.window.getNormalBounds() ?? this.#fallbackWindowBounds();
		const candidate = {
			height: source.height,
			width: source.width,
			x: point === undefined ? source.x + 32 : Math.round(point.x - source.width / 2),
			y: point === undefined ? source.y + 32 : Math.round(point.y - 24),
		};
		const launch = resolveDesktopWindowLaunchState(
			{ bounds: candidate, maximized: false, version: 1 },
			this.#dependencies.getWindowDisplayTopology(),
		);
		return completeDesktopWindowBounds(
			launch.bounds,
			this.#dependencies.getWindowDisplayTopology(),
		);
	};

	#fallbackWindowBounds(): ReturnType<BrowserWindow['getNormalBounds']> {
		const topology = this.#dependencies.getWindowDisplayTopology();
		return completeDesktopWindowBounds({ height: 720, width: 1_024 }, topology);
	}

	#mostRecentNativeWindow(): BrowserWindow | undefined {
		const activationOrder = this.#windowLayoutCoordinator?.layout.activationOrder ?? [];
		for (const windowId of [...activationOrder].reverse()) {
			const window = this.#windows.get(windowId)?.window;
			if (window !== undefined && !window.isDestroyed()) return window;
		}
		return undefined;
	}

	#selectApplicationLifetimeBroker(window: BrowserWindow | undefined): void {
		this.#applicationLifetimeBroker =
			window === undefined ? undefined : this.#rendererDocuments.get(window)?.lifetimeBroker;
	}

	async shutdown(): Promise<void> {
		if (this.#shutdownPromise !== undefined) return this.#shutdownPromise;
		this.#shuttingDown = true;
		const operation = this.#shutdownInternal();
		this.#shutdownPromise = operation.catch((error: unknown) => {
			this.#shutdownPromise = undefined;
			throw error;
		});
		return this.#shutdownPromise;
	}

	async #shutdownInternal(): Promise<void> {
		for (const record of this.#windows.values()) {
			if (!record.window.isDestroyed()) {
				await this.#windowLayoutCoordinator?.captureGeometry(
					record.windowId,
					record.window.getNormalBounds(),
					record.window.isMaximized(),
				);
			}
		}
		await this.#windowLayoutOwner?.flush().catch(() => {
			this.#writeCopyPathTrace('window-layout-flush-failed-during-shutdown');
		});
		this.#windowStateAttachment?.capture();
		await this.#windowStateOwner.flush().catch(() => {
			this.#writeCopyPathTrace('window-state-flush-failed-during-shutdown');
		});
		this.#removeHostListener?.();
		this.#removeHostListener = undefined;
		let firstFailure: unknown;
		this.#sshProjectEnrollmentRuntime?.dispose();
		this.#sshProjectEnrollmentRuntime = undefined;
		try {
			const remoteTerminalRuntimeOwner = this.#sshEnvironmentTerminalRuntimeOwner;
			await remoteTerminalRuntimeOwner?.dispose();
			if (this.#sshEnvironmentTerminalRuntimeOwner === remoteTerminalRuntimeOwner) {
				this.#sshEnvironmentTerminalRuntimeOwner = undefined;
			}
		} catch (error: unknown) {
			firstFailure = error;
			this.#writeCopyPathTrace('ssh-environment-terminal-runtime-failed-during-shutdown');
		}
		try {
			await this.#sshEnvironmentConnections.dispose();
		} catch (error: unknown) {
			firstFailure ??= error;
			this.#writeCopyPathTrace('ssh-environment-connections-failed-during-shutdown');
		}
		try {
			await this.#disposeTerminalExitRetryOwner();
		} catch (error: unknown) {
			firstFailure = error;
			this.#writeCopyPathTrace('terminal-exit-retry-owner-failed-during-shutdown');
		}
		try {
			this.#writeCopyPathTrace('main-shutdown');
			for (const record of this.#windows.values()) {
				this.#beginRendererDocument(record.window);
			}
			await this.#documentTransition;
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			await this.#terminalLaunchProfileController.flush();
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			await this.#terminalDefaultSettingsController?.flush();
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			await this.#disposeBrowserAccessResources();
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			await this.#disposeSemanticCommandResources();
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			const projectWorktreeRuntime = this.#projectWorktreeRuntime;
			await projectWorktreeRuntime?.dispose();
			if (this.#projectWorktreeRuntime === projectWorktreeRuntime) {
				this.#projectWorktreeRuntime = undefined;
			}
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			const coordinator = this.#clientSessionCoordinator;
			await coordinator?.dispose();
			if (this.#clientSessionCoordinator === coordinator) {
				this.#clientSessionCoordinator = undefined;
				this.#terminalIntentReconciler = undefined;
			}
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		this.#windowLayoutCoordinator?.dispose();
		this.#windowLayoutCoordinator = undefined;
		this.#windowLayoutOwner?.dispose();
		this.#windowLayoutOwner = undefined;
		try {
			const terminalRuntime = this.#terminalRuntime;
			await terminalRuntime?.dispose();
			if (this.#terminalRuntime === terminalRuntime) this.#terminalRuntime = undefined;
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		try {
			const observationIngress = this.#agentHookObservationIngress;
			await observationIngress?.dispose();
			if (this.#agentHookObservationIngress === observationIngress) {
				this.#agentHookObservationIngress = undefined;
			}
		} catch (error: unknown) {
			firstFailure ??= error;
		}
		this.#agentObservationRuntime.dispose();
		this.#connection?.close();
		this.#connection = undefined;
		if (firstFailure !== undefined) throw firstFailure;
	}

	async #disposeSemanticCommandResources(): Promise<void> {
		const endpoint = this.#semanticCommandLocalEndpoint;
		try {
			await endpoint?.dispose();
		} finally {
			if (this.#semanticCommandLocalEndpoint === endpoint) {
				this.#semanticCommandLocalEndpoint = undefined;
			}
			this.#semanticCommandService?.dispose();
			this.#semanticCommandService = undefined;
		}
	}

	async #disposePendingGatewayClientCleanups(): Promise<void> {
		if (this.#pendingGatewayClientCleanups.size === 0) return;
		const results = await Promise.allSettled(
			[...this.#pendingGatewayClientCleanups].map((owner) => owner.dispose()),
		);
		const failures = results.flatMap((result) =>
			result.status === 'rejected' ? [result.reason] : [],
		);
		if (failures.length > 0) {
			throw new AggregateError(failures, 'Desktop Gateway client cleanup did not converge.');
		}
	}

	async #disposeBrowserAccessResources(): Promise<void> {
		const browserAccessRuntime = this.#browserAccessRuntime;
		const failures: unknown[] = [];
		try {
			await browserAccessRuntime?.dispose();
		} catch (error: unknown) {
			failures.push(error);
		}
		try {
			await this.#disposePendingGatewayClientCleanups();
		} catch (error: unknown) {
			failures.push(error);
		}
		if (failures.length > 0) {
			throw new AggregateError(failures, 'Desktop Browser access disposal did not converge.');
		}
		if (this.#browserAccessRuntime === browserAccessRuntime) {
			this.#browserAccessRuntime = undefined;
		}
	}

	#beginRendererDocument(window: BrowserWindow): number {
		const generation = ++this.#documentGeneration;
		this.#documentGenerations.set(window, generation);
		const handle = this.#rendererDocuments.get(window);
		this.#rendererDocuments.delete(window);
		if (handle !== undefined) this.#rendererDocumentRetirements.set(window, handle);
		if (this.#applicationLifetimeBroker === handle?.lifetimeBroker) {
			this.#applicationLifetimeBroker = undefined;
		}
		const transition = this.#documentTransition.then(async () => {
			const retirement = this.#rendererDocumentRetirements.get(window);
			await retirement?.cleanup();
			if (this.#rendererDocumentRetirements.get(window) === retirement) {
				this.#rendererDocumentRetirements.delete(window);
			}
			this.#documentFailure = undefined;
		});
		this.#documentTransition = transition.catch((error: unknown) => {
			this.#documentFailure =
				error instanceof Error ? error : new Error('Renderer document cleanup failed.');
		});
		return generation;
	}

	async #recoverApplicationLifetimeBroker(
		retiredBroker: DesktopApplicationLifetimeBrokerHandle | undefined,
	): Promise<boolean> {
		if (this.#applicationLifetimeRecovery !== undefined) {
			return this.#applicationLifetimeRecovery;
		}
		const operation = this.#recoverApplicationLifetimeBrokerInternal(retiredBroker);
		this.#applicationLifetimeRecovery = operation;
		try {
			return await operation;
		} finally {
			if (this.#applicationLifetimeRecovery === operation) {
				this.#applicationLifetimeRecovery = undefined;
			}
		}
	}

	async #recoverApplicationLifetimeBrokerInternal(
		retiredBroker: DesktopApplicationLifetimeBrokerHandle | undefined,
	): Promise<boolean> {
		await this.#documentTransition;
		if (this.#shuttingDown) return false;
		const currentBroker = this.#applicationLifetimeBroker;
		if (currentBroker !== undefined && currentBroker !== retiredBroker) return true;
		if (!this.hasWindow) {
			await this.openWindow();
			return this.#applicationLifetimeBroker !== undefined;
		}
		const window = this.#window;
		if (window === undefined || window.isDestroyed()) return false;
		window.reload();
		const deadline = Date.now() + RENDERER_LIFETIME_RECOVERY_TIMEOUT_MILLISECONDS;
		do {
			await this.#documentTransition;
			const recoveredBroker = this.#applicationLifetimeBroker;
			if (recoveredBroker !== undefined && recoveredBroker !== retiredBroker) return true;
			await new Promise((resolve) =>
				setTimeout(resolve, RENDERER_LIFETIME_RECOVERY_POLL_MILLISECONDS),
			);
		} while (Date.now() < deadline && !this.#shuttingDown);
		return false;
	}

	#queueRendererDocument(window: BrowserWindow, generation: number): void {
		this.#documentTransition = this.#documentTransition.then(async () => {
			try {
				await this.#attachRendererDocument(window, generation);
			} catch (error) {
				this.#documentFailure =
					error instanceof Error ? error : new Error('Renderer document control failed.');
			}
		});
	}

	async #attachRendererDocument(window: BrowserWindow, generation: number): Promise<void> {
		const terminalRuntime = this.#terminalRuntime;
		const sshEnvironmentTerminalRuntimeOwner = this.#sshEnvironmentTerminalRuntimeOwner;
		const sshProjectEnrollmentRuntime = this.#sshProjectEnrollmentRuntime;
		const clientSessionCoordinator = this.#clientSessionCoordinator;
		const agentIntegrationManagementController = this.#agentIntegrationManagementController;
		const terminalDefaultSettingsController = this.#terminalDefaultSettingsController;
		const windowLayoutCoordinator = this.#windowLayoutCoordinator;
		const windowId = this.#windowIds.get(window);
		const windowRecord = windowId === undefined ? undefined : this.#windows.get(windowId);
		if (
			terminalRuntime === undefined ||
			sshEnvironmentTerminalRuntimeOwner === undefined ||
			sshProjectEnrollmentRuntime === undefined ||
			clientSessionCoordinator === undefined ||
			terminalDefaultSettingsController === undefined ||
			windowLayoutCoordinator === undefined ||
			windowId === undefined ||
			windowRecord?.layoutCommitted !== true ||
			this.#rendererDocumentRetirements.has(window) ||
			window.isDestroyed() ||
			generation !== this.#documentGenerations.get(window) ||
			this.#shuttingDown
		) {
			return;
		}
		let lifetimeChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let agentObservationChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let agentIntegrationManagementChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let browserAccessChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let clipboardChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let clientSessionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectFolderAcquisitionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectWorktreeChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let remoteTerminalCreationChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let sshProjectEnrollmentChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let terminalLaunchProfileChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let terminalDefaultSettingsChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let windowLayoutChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionBroker: DesktopProjectionControlBrokerHandle | undefined;
		let agentObservationBroker: DesktopAgentObservationBrokerHandle | undefined;
		let agentIntegrationManagementBroker: DesktopAgentIntegrationManagementBrokerHandle | undefined;
		let clientSessionBroker: DesktopClientSessionBrokerHandle | undefined;
		let lifetimeBroker: DesktopApplicationLifetimeBrokerHandle | undefined;
		let browserAccessBroker: DesktopBrowserAccessBrokerHandle | undefined;
		let clipboardBroker: DesktopClipboardBrokerHandle | undefined;
		let projectFolderAcquisitionBroker: DesktopProjectFolderAcquisitionBrokerHandle | undefined;
		let projectWorktreeBroker: DesktopProjectWorktreeBrokerHandle | undefined;
		let remoteTerminalCreationBroker: DesktopRemoteTerminalCreationBrokerHandle | undefined;
		let sshProjectEnrollmentBroker: DesktopSshProjectEnrollmentBrokerHandle | undefined;
		let terminalLaunchProfileBroker: DesktopTerminalLaunchProfileBrokerHandle | undefined;
		let terminalDefaultSettingsBroker: DesktopTerminalDefaultSettingsBrokerHandle | undefined;
		let windowLayoutBroker: DesktopWindowLayoutBroker | undefined;
		let cleanupComplete = false;
		let cleanupPromise: Promise<void> | undefined;
		const cleanup = async (): Promise<void> => {
			if (cleanupComplete) return;
			if (cleanupPromise !== undefined) return cleanupPromise;
			const operation = (async () => {
				let firstFailure: unknown;
				for (const dispose of [
					() => agentIntegrationManagementBroker?.dispose(),
					() => agentObservationBroker?.dispose(),
					() => lifetimeBroker?.dispose(),
					() => browserAccessBroker?.dispose(),
					() => clipboardBroker?.dispose(),
					() => projectFolderAcquisitionBroker?.dispose(),
					() => projectWorktreeBroker?.dispose(),
					() => remoteTerminalCreationBroker?.dispose(),
					() => sshProjectEnrollmentBroker?.dispose(),
					() => terminalDefaultSettingsBroker?.dispose(),
					() => terminalLaunchProfileBroker?.dispose(),
					() => windowLayoutBroker?.dispose(),
				]) {
					try {
						dispose();
					} catch (error: unknown) {
						firstFailure ??= error;
					}
				}
				let clientDisposeFailed = false;
				try {
					await clientSessionBroker?.dispose();
				} catch (error: unknown) {
					clientDisposeFailed = true;
					firstFailure ??= error;
				}
				if (clientSessionBroker === undefined || clientDisposeFailed) {
					try {
						await projectionBroker?.dispose();
					} catch (error: unknown) {
						firstFailure ??= error;
					}
				}
				closeRendererChannel(lifetimeChannel);
				closeRendererChannel(agentObservationChannel);
				closeRendererChannel(agentIntegrationManagementChannel);
				closeRendererChannel(browserAccessChannel);
				closeRendererChannel(clipboardChannel);
				closeRendererChannel(clientSessionChannel);
				closeRendererChannel(projectionChannel);
				closeRendererChannel(projectFolderAcquisitionChannel);
				closeRendererChannel(projectWorktreeChannel);
				closeRendererChannel(remoteTerminalCreationChannel);
				closeRendererChannel(sshProjectEnrollmentChannel);
				closeRendererChannel(terminalLaunchProfileChannel);
				closeRendererChannel(terminalDefaultSettingsChannel);
				closeRendererChannel(windowLayoutChannel);
				if (firstFailure !== undefined) throw firstFailure;
				cleanupComplete = true;
			})();
			cleanupPromise = operation.catch((error: unknown) => {
				cleanupPromise = undefined;
				throw error;
			});
			return cleanupPromise;
		};
		try {
			const terminalProjectionRouter = new DesktopTerminalProjectionRouter({
				getDocument: () => clientSessionCoordinator.canonicalDocument,
				localRuntime: terminalRuntime,
				remoteRuntimes: sshEnvironmentTerminalRuntimeOwner,
			});
			lifetimeChannel = this.#dependencies.createMessageChannel();
			agentObservationChannel = this.#dependencies.createMessageChannel();
			agentIntegrationManagementChannel = this.#dependencies.createMessageChannel();
			browserAccessChannel = this.#dependencies.createMessageChannel();
			clientSessionChannel = this.#dependencies.createMessageChannel();
			projectionChannel = this.#dependencies.createMessageChannel();
			projectFolderAcquisitionChannel = this.#dependencies.createMessageChannel();
			projectWorktreeChannel = this.#dependencies.createMessageChannel();
			remoteTerminalCreationChannel = this.#dependencies.createMessageChannel();
			sshProjectEnrollmentChannel = this.#dependencies.createMessageChannel();
			terminalLaunchProfileChannel = this.#dependencies.createMessageChannel();
			terminalDefaultSettingsChannel = this.#dependencies.createMessageChannel();
			clipboardChannel = this.#dependencies.createMessageChannel();
			windowLayoutChannel = this.#dependencies.createMessageChannel();
			projectionBroker = this.#dependencies.createProjectionControlBroker(
				projectionChannel.port2,
				terminalProjectionRouter,
			);
			agentObservationBroker =
				this.#dependencies.createAgentObservationBroker?.(
					agentObservationChannel.port2,
					this.#agentObservationRuntime,
				) ??
				new DesktopAgentObservationBroker(
					agentObservationChannel.port2,
					this.#agentObservationRuntime,
				);
			agentIntegrationManagementBroker = this.#dependencies.createAgentIntegrationManagementBroker(
				agentIntegrationManagementChannel.port2,
				agentIntegrationManagementController,
			);
			clientSessionBroker = this.#dependencies.createClientSessionBroker(
				clientSessionChannel.port2,
				clientSessionCoordinator,
				async () => {
					await projectionBroker?.dispose();
				},
			);
			lifetimeBroker = this.#dependencies.createApplicationLifetimeBroker(lifetimeChannel.port2, {
				drainForStop: this.#drainForApplicationStop,
				getDevelopmentHostRestartPresentationIds: () => this.#developmentHostRestartPresentationIds,
				prepareDevelopmentHostRestart: this.#options.prepareDevelopmentHostRestart,
				prepareMode: (mode, restartPresentationIds) => {
					if (mode === 'development-host-restart') {
						if (
							restartPresentationIds === undefined ||
							new Set(restartPresentationIds).size !== restartPresentationIds.length ||
							(this.#developmentHostRestartPresentationIds !== undefined &&
								!sameTerminalPresentationIds(
									this.#developmentHostRestartPresentationIds,
									restartPresentationIds,
								))
						) {
							return false;
						}
					}
					const prepared = clientSessionCoordinator.beginQuiescing(
						clientSessionBroker?.bootstrap.clientSessionId ?? '',
						mode,
					);
					if (prepared && mode === 'development-host-restart') {
						this.#developmentHostRestartPresentationIds = [...(restartPresentationIds ?? [])];
					}
					return prepared;
				},
			});
			browserAccessBroker = this.#dependencies.createBrowserAccessBroker(
				browserAccessChannel.port2,
				this.#browserAccessRuntime ?? unavailableBrowserAccessRuntime,
			);
			clipboardBroker = this.#dependencies.createClipboardBroker(clipboardChannel.port2, () =>
				clipboard.readText(),
			);
			projectFolderAcquisitionBroker = this.#dependencies.createProjectFolderAcquisitionBroker(
				projectFolderAcquisitionChannel.port2,
				window,
			);
			const projectWorktreeRuntime = this.#projectWorktreeRuntime;
			if (projectWorktreeRuntime === undefined) {
				throw new Error('Desktop Project Worktree runtime is unavailable.');
			}
			projectWorktreeBroker = this.#dependencies.createProjectWorktreeBroker(
				projectWorktreeChannel.port2,
				projectWorktreeRuntime,
			);
			remoteTerminalCreationBroker = this.#dependencies.createRemoteTerminalCreationBroker(
				remoteTerminalCreationChannel.port2,
				{
					create: (projectId, workspaceId, signal) =>
						this.createSshEnvironmentRemoteTerminalForWorkspace(projectId, workspaceId, signal),
				},
			);
			sshProjectEnrollmentBroker = this.#dependencies.createSshProjectEnrollmentBroker(
				sshProjectEnrollmentChannel.port2,
				sshProjectEnrollmentRuntime,
			);
			terminalLaunchProfileBroker = this.#dependencies.createTerminalLaunchProfileBroker(
				terminalLaunchProfileChannel.port2,
				this.#terminalLaunchProfileController,
			);
			terminalDefaultSettingsBroker = this.#dependencies.createTerminalDefaultSettingsBroker(
				terminalDefaultSettingsChannel.port2,
				terminalDefaultSettingsController,
			);
			windowLayoutBroker = new DesktopWindowLayoutBroker(
				windowLayoutChannel.port2,
				windowLayoutCoordinator.createBrokerController(windowId),
			);
			clientSessionBroker.start();
			agentObservationBroker.start();
			agentIntegrationManagementBroker.start();
			browserAccessBroker.start();
			clipboardBroker.start();
			lifetimeBroker.start();
			projectionBroker.start();
			projectFolderAcquisitionBroker.start();
			projectWorktreeBroker.start();
			remoteTerminalCreationBroker.start();
			sshProjectEnrollmentBroker.start();
			terminalLaunchProfileBroker.start();
			terminalDefaultSettingsBroker.start();
			windowLayoutBroker.start();
			window.webContents.postMessage(
				DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
				DesktopRendererBootstrapSchema.parse({
					clientSession: clientSessionBroker.bootstrap,
					type: DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
				}),
				[
					clientSessionChannel.port1,
					projectionChannel.port1,
					agentObservationChannel.port1,
					agentIntegrationManagementChannel.port1,
					lifetimeChannel.port1,
					browserAccessChannel.port1,
					projectFolderAcquisitionChannel.port1,
					projectWorktreeChannel.port1,
					terminalLaunchProfileChannel.port1,
					terminalDefaultSettingsChannel.port1,
					clipboardChannel.port1,
					windowLayoutChannel.port1,
					remoteTerminalCreationChannel.port1,
					sshProjectEnrollmentChannel.port1,
				],
			);
			this.#rendererDocuments.set(window, { cleanup, lifetimeBroker });
			if (this.#window === window) this.#applicationLifetimeBroker = lifetimeBroker;
		} catch (error: unknown) {
			await cleanup();
			throw error;
		}
	}

	async #openGatewayClient(): Promise<DesktopGatewayClientHandle> {
		const terminalRuntime = this.#terminalRuntime;
		const sshEnvironmentTerminalRuntimeOwner = this.#sshEnvironmentTerminalRuntimeOwner;
		const clientSessionCoordinator = this.#clientSessionCoordinator;
		if (
			terminalRuntime === undefined ||
			sshEnvironmentTerminalRuntimeOwner === undefined ||
			clientSessionCoordinator === undefined ||
			this.#shuttingDown
		) {
			throw new Error('Desktop Gateway client control is unavailable.');
		}
		let clientSessionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionChannel:
			| { readonly port1: MessagePortMain; readonly port2: MessagePortMain }
			| undefined;
		let projectionBroker: DesktopProjectionControlBrokerHandle | undefined;
		let clientSessionBroker: DesktopClientSessionBrokerHandle | undefined;
		let disposalPromise: Promise<void> | undefined;
		let disposed = false;
		const cleanup = async (): Promise<void> => {
			if (disposed) return;
			if (disposalPromise !== undefined) return disposalPromise;
			const operation = (async () => {
				let firstFailure: unknown;
				let clientDisposeFailed = false;
				try {
					await clientSessionBroker?.dispose();
				} catch (error: unknown) {
					clientDisposeFailed = true;
					firstFailure = error;
				}
				if (clientSessionBroker === undefined || clientDisposeFailed) {
					try {
						await projectionBroker?.dispose();
					} catch (error: unknown) {
						firstFailure ??= error;
					}
				}
				closeRendererChannel(clientSessionChannel);
				closeRendererChannel(projectionChannel);
				if (firstFailure !== undefined) throw firstFailure;
				disposed = true;
				this.#pendingGatewayClientCleanups.delete(owner);
			})();
			disposalPromise = operation.catch((error: unknown) => {
				disposalPromise = undefined;
				throw error;
			});
			return disposalPromise;
		};
		const owner: GatewayClientResourceOwner = { dispose: cleanup };
		this.#pendingGatewayClientCleanups.add(owner);
		try {
			const terminalProjectionRouter = new DesktopTerminalProjectionRouter({
				getDocument: () => clientSessionCoordinator.canonicalDocument,
				localRuntime: terminalRuntime,
				remoteRuntimes: sshEnvironmentTerminalRuntimeOwner,
			});
			clientSessionChannel = this.#dependencies.createMessageChannel();
			projectionChannel = this.#dependencies.createMessageChannel();
			projectionBroker = this.#dependencies.createProjectionControlBroker(projectionChannel.port2, {
				isPublicationTargetCurrent: (presentationId, lease) =>
					terminalProjectionRouter.isPublicationTargetCurrent(presentationId, lease),
				open: async (presentationId, geometry) =>
					terminalProjectionRouter.open(presentationId, geometry, {
						brokerEffectTarget: 'client',
					}),
			});
			clientSessionBroker = this.#dependencies.createClientSessionBroker(
				clientSessionChannel.port2,
				clientSessionCoordinator,
				async () => projectionBroker?.dispose(),
			);
			clientSessionBroker.start();
			projectionBroker.start();
			return {
				bootstrap: clientSessionBroker.bootstrap,
				clientSessionPort: clientSessionChannel.port1,
				dispose: cleanup,
				projectionPort: projectionChannel.port1,
			};
		} catch (error: unknown) {
			try {
				await cleanup();
			} catch (cleanupError: unknown) {
				throw new AggregateError(
					[error, cleanupError],
					'Desktop Gateway client bootstrap compensation failed.',
				);
			}
			throw error;
		}
	}

	readonly #drainForApplicationStop = async (
		documentRevision: WorkbenchSharedDocumentRevision,
	): Promise<DesktopApplicationLifetimeDrainResult> => {
		const terminalRuntime = this.#terminalRuntime;
		if (terminalRuntime === undefined) {
			return {
				failures: [typedExitFailure('unreferenced-binding-close-failed', 1, true)],
				kind: 'failed',
			};
		}
		const loaded = await this.#workbenchDocumentStore.load().catch(() => ({
			kind: 'unavailable' as const,
			reason: 'read-failed' as const,
		}));
		if (loaded.kind !== 'available' || loaded.document.revision !== documentRevision) {
			return {
				failures: [typedExitFailure('shared-document-control-unavailable', 1, true)],
				kind: 'failed',
			};
		}
		const referencedPresentationIds = collectDocumentPresentationIds(loaded.document);
		const drained = await terminalRuntime.closeBindingsNotReferencedBy(referencedPresentationIds);
		if (drained.failures.length > 0) {
			return {
				failures: [
					typedExitFailure(
						'unreferenced-binding-close-failed',
						drained.failures.length,
						drained.failures.every((item) => item.failure.retryable),
					),
				],
				kind: 'failed',
			};
		}
		const remainingOwnedBindings = terminalRuntime.getOwnedBindingCount();
		if (remainingOwnedBindings > 0) {
			return {
				failures: [typedExitFailure('terminal-stop-failed', remainingOwnedBindings, true)],
				kind: 'failed',
			};
		}
		let unknownHostTerminalCount: number;
		try {
			unknownHostTerminalCount = await terminalRuntime.inspectUnknownHostTerminalCount();
		} catch {
			return {
				failures: [typedExitFailure('host-inventory-unavailable', 1, true)],
				kind: 'failed',
			};
		}
		if (unknownHostTerminalCount > 0) {
			return {
				failures: [typedExitFailure('host-orphan-detected', unknownHostTerminalCount, false)],
				kind: 'failed',
			};
		}
		return { closedUnreferencedCount: drained.closedCount, kind: 'completed' };
	};

	readonly #handleHostEnvelope = (envelope: DecodedProtocolEnvelope): void => {
		if (envelope.kind === MessageKind.terminalExited) {
			const exited = decodeControlJson(envelope.payload, TerminalExitedSchema);
			if (exited.ok) this.#acceptTerminalExit(exited.value);
			return;
		}
		if (envelope.kind === MessageKind.terminalMetadataChanged) {
			const metadata = decodeControlJson(envelope.payload, TerminalMetadataChangedSchema);
			if (metadata.ok) this.#acceptTerminalMetadata(metadata.value);
			return;
		}
		if (envelope.kind === MessageKind.terminalApplicationSignal) {
			const signal = decodeControlJson(envelope.payload, TerminalApplicationSignalSchema);
			if (signal.ok) this.#acceptTerminalApplicationSignal(signal.value);
			return;
		}
		if (envelope.kind !== MessageKind.terminalFailed) return;
		const failed = decodeControlJson(envelope.payload, TerminalFailedSchema);
		if (!failed.ok) return;
		this.#acceptTerminalFailure(failed.value);
	};

	#acceptTerminalApplicationSignal(event: TerminalApplicationSignal): void {
		const presentationId = this.#terminalRuntime?.findPresentationByIdentity(event.identity);
		if (presentationId === undefined || presentationId === null) return;
		this.#agentHookObservationIngress?.observeTerminalApplicationSignal(event, presentationId);
		this.#agentObservationRuntime.observeTerminalApplicationSignal?.(event, presentationId);
	}

	#acceptTerminalMetadata(event: TerminalMetadataChanged): void {
		const key = terminalExitIdentityKey(event.identity);
		const existing = this.#pendingTerminalMetadata.get(key);
		if (existing !== undefined && BigInt(existing.revision) >= BigInt(event.revision)) return;
		if (
			existing === undefined &&
			this.#pendingTerminalMetadata.size >= MAX_PENDING_TERMINAL_METADATA
		) {
			const oldestKey = this.#pendingTerminalMetadata.keys().next().value;
			if (oldestKey !== undefined) this.#pendingTerminalMetadata.delete(oldestKey);
		}
		this.#pendingTerminalMetadata.delete(key);
		this.#pendingTerminalMetadata.set(key, event);
		this.#publishPendingTerminalMetadata(key);
	}

	#publishPendingTerminalMetadata(key: string): void {
		const event = this.#pendingTerminalMetadata.get(key);
		const runtime = this.#terminalRuntime;
		const coordinator = this.#clientSessionCoordinator;
		if (event === undefined || runtime === undefined || coordinator === undefined) return;
		const presentationId = runtime.findPresentationByIdentity(event.identity);
		if (presentationId === null) return;
		coordinator.publishTerminalMetadata({
			currentDirectory: event.currentDirectory,
			identity: event.identity,
			presentationId,
			revision: event.revision,
			title: event.title,
		});
		this.#pendingTerminalMetadata.delete(key);
	}

	#publishAllPendingTerminalMetadata(): void {
		for (const key of this.#pendingTerminalMetadata.keys()) {
			this.#publishPendingTerminalMetadata(key);
		}
	}

	#acceptTerminalFailure(event: TerminalFailed): void {
		const runtime = this.#terminalRuntime;
		if (runtime === undefined) return;
		void runtime
			.recordFailure(event)
			.then((presentationId) => {
				if (presentationId === null) return;
				const reconciler = this.#terminalIntentReconciler;
				const coordinator = this.#clientSessionCoordinator;
				if (reconciler === undefined || coordinator === undefined) return;
				const documentEvent = toDocumentFailure(presentationId, event);
				if (!reconciler.recordExternalEvent(documentEvent)) return;
				coordinator.publishTerminalEvent(documentEvent);
				this.#writeCopyPathTrace('terminal-failed');
			})
			.catch(() => this.#writeCopyPathTrace('terminal-failure-association-failed'));
	}

	#acceptTerminalExit(event: TerminalExited): void {
		const key = terminalExitIdentityKey(event);
		if (
			this.#isTerminalExitPublished(event) &&
			!this.#terminalExitPublicationReservations.has(key)
		) {
			return;
		}
		if (this.#terminalExitSpillReady && this.#terminalExitSpillStore.isRejected(event)) return;
		if (!this.#admitPendingTerminalExit(event, 'host-admission')) return;
		if (this.#terminalExitRetryOwnerDisposed || this.#terminalExitRetryOwnerClosing) {
			if (!this.#spillTerminalExit(event, 'shutdown')) {
				throw new Error('Desktop terminal exit owner is closed and its spill is full.');
			}
			this.#writeCopyPathTrace('terminal-exit-spilled-during-shutdown');
			return;
		}
		const existing = this.#pendingTerminalExits.get(key);
		if (
			existing === undefined &&
			this.#pendingTerminalExits.size >= MAX_PENDING_TERMINAL_EXIT_RETRIES
		) {
			// The Host event has no ACK/redelivery. Persist before returning from
			// this listener so a full in-memory owner cannot lose the envelope.
			const spill = this.#spillTerminalExit(event, 'queue-full');
			if (!spill) {
				this.#writeCopyPathTrace('terminal-exit-spill-full');
				throw new Error('Desktop terminal exit spill admission is full.');
			}
			this.#writeCopyPathTrace('terminal-exit-spilled');
			this.#scheduleTerminalExitRetry();
			return;
		}
		if (existing === undefined) {
			this.#pendingTerminalExits.set(key, event);
			this.#terminalExitAttemptCounts.set(key, 0);
		} else if (BigInt(event.finalRevision) > BigInt(existing.finalRevision)) {
			this.#pendingTerminalExits.set(key, event);
			this.#terminalExitAttemptCounts.set(key, 0);
		}
		void this.#attemptTerminalExit(key);
	}

	#admitPendingTerminalExit(event: TerminalExited, reason: TerminalExitSpillReason): boolean {
		if (!this.#terminalExitSpillReady) {
			throw new Error('Desktop terminal exit pending journal is not ready.');
		}
		try {
			const admission = this.#terminalExitSpillStore.admit(event, reason);
			if (admission.kind === 'admitted') return true;
			const rejection = this.#terminalExitSpillStore.reject(event, 'pending-journal-full');
			if (rejection.kind === 'recorded') {
				this.#writeCopyPathTrace('terminal-exit-rejected-pending-journal-full');
				return false;
			}
			throw new Error('Desktop terminal exit rejection journal is full.');
		} catch (error: unknown) {
			try {
				const rejection = this.#terminalExitSpillStore.reject(event, 'write-failed');
				if (rejection.kind === 'recorded') {
					this.#writeCopyPathTrace('terminal-exit-rejected-write-failed');
					return false;
				}
			} catch (rejectionError: unknown) {
				throw new AggregateError(
					[error, rejectionError],
					'Desktop terminal exit pending journal failed closed.',
				);
			}
			throw error;
		}
	}

	#attemptTerminalExit(key: string): Promise<void> {
		const existingAttempt = this.#terminalExitAttempts.get(key);
		if (existingAttempt !== undefined) return existingAttempt;
		const operation = this.#recordPendingTerminalExit(key).catch(() => {
			// A failed attempt keeps the event in the Main-owned bounded queue and
			// schedules a later retry. This branch is deliberately contained so a
			// Host envelope can never create an unhandled rejection.
			this.#writeCopyPathTrace('terminal-exit-persistence-failed');
		});
		this.#terminalExitAttempts.set(key, operation);
		void operation.then(() => {
			if (this.#terminalExitAttempts.get(key) === operation) {
				this.#terminalExitAttempts.delete(key);
			}
			if (
				!this.#terminalExitRetryOwnerDisposed &&
				!this.#terminalExitRetryOwnerClosing &&
				this.#pendingTerminalExits.has(key)
			) {
				this.#scheduleTerminalExitRetry();
			}
		});
		return operation;
	}

	#publishTerminalExit(
		runtime: DesktopTerminalRuntime,
		event: TerminalExited,
		presentationId: ReturnType<typeof terminalPresentationId>,
		publish: (
			event: Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }>,
		) => unknown,
	): TerminalExitPublicationResult {
		const key = terminalExitIdentityKey(event);
		const reconciler = this.#terminalIntentReconciler;
		if (reconciler === undefined) return 'pending';
		let currentReservation = this.#terminalExitPublicationReservations.get(key);
		if (
			currentReservation === undefined ||
			BigInt(currentReservation.event.finalRevision) !== BigInt(event.finalRevision)
		) {
			const documentEvent = toDocumentExit(presentationId, event);
			if (this.#isTerminalExitPublished(event) && reconciler.hasExternalEvent(documentEvent)) {
				return 'already-committed';
			}
			if (!reconciler.reserveExternalEvent(documentEvent)) return 'pending';
			currentReservation = {
				event,
				presentationId,
			};
			this.#terminalExitPublicationReservations.set(key, currentReservation);
		}
		if (currentReservation === undefined) return 'pending';
		const reservationEvent = currentReservation.event;
		const documentEvent = toDocumentExit(currentReservation.presentationId, reservationEvent);
		if (
			!reconciler.hasExternalEvent(documentEvent) &&
			!reconciler.reserveExternalEvent(documentEvent)
		) {
			return 'pending';
		}
		const ledgerCommitted = this.#isTerminalExitPublished(reservationEvent);
		if (!ledgerCommitted) {
			let committed: boolean;
			try {
				committed = runtime.commitExitPublication?.(reservationEvent) ?? true;
			} catch {
				this.#writeCopyPathTrace('terminal-exit-adapter-commit-failed');
				return 'pending';
			}
			if (!committed) return 'pending';
			const marked = this.#markTerminalExitPublished(reservationEvent);
			if (!marked) return 'pending';
			this.#publishedTerminalExitRevisions.set(key, reservationEvent.finalRevision);
		}
		const externalEventCommitted = reconciler.commitExternalEvent(documentEvent);
		if (!externalEventCommitted) return 'pending';
		try {
			if (publish(documentEvent) === false) return 'pending';
		} catch {
			this.#writeCopyPathTrace('terminal-exit-publication-failed');
			return 'pending';
		}
		this.#terminalExitPublicationReservations.delete(key);
		this.#removePublishedTerminalExitFromPending(reservationEvent);
		return ledgerCommitted ? 'already-committed' : 'committed';
	}

	#commitReconciledTerminalExit(
		runtime: DesktopTerminalRuntime,
		event: TerminalPresentationEvent,
		publish: (event: TerminalPresentationEvent) => unknown,
	): boolean {
		if (event.type !== 'terminal-exited') return true;
		const terminalExit = runtime.getDurableTerminalExit(event.presentationId);
		if (terminalExit === null) return false;
		const publication = this.#publishTerminalExit(
			runtime,
			terminalExit,
			event.presentationId,
			publish,
		);
		if (publication === 'pending') {
			this.#acceptTerminalExit(terminalExit);
			return false;
		}
		return true;
	}

	#isTerminalExitPublished(event: TerminalExited): boolean {
		const publishedRevision = this.#publishedTerminalExitRevisions.get(
			terminalExitIdentityKey(event),
		);
		if (
			publishedRevision !== undefined &&
			BigInt(publishedRevision) >= BigInt(event.finalRevision)
		) {
			return true;
		}
		if (!this.#terminalExitSpillReady) return false;
		try {
			return this.#terminalExitPublicationLedger.has(event);
		} catch {
			return false;
		}
	}

	#markTerminalExitPublished(event: TerminalExited): boolean {
		try {
			return this.#terminalExitPublicationLedger.markPublished(event);
		} catch {
			this.#writeCopyPathTrace('terminal-exit-publication-ledger-mark-failed');
			return false;
		}
	}

	async #recordPendingTerminalExit(key: string): Promise<void> {
		const event = this.#pendingTerminalExits.get(key);
		const runtime = this.#terminalRuntime;
		if (event === undefined || runtime === undefined) return;
		this.#terminalExitAttemptCounts.set(key, (this.#terminalExitAttemptCounts.get(key) ?? 0) + 1);
		const outcome = await this.#recordTerminalExit(runtime, event);
		if (outcome.kind === 'retryable') {
			this.#writeCopyPathTrace(`terminal-exit-${outcome.reason}`);
			if (
				(outcome.reason === 'association' || outcome.reason === 'buffer-full') &&
				(this.#terminalExitAttemptCounts.get(key) ?? 0) >= TERMINAL_EXIT_ASSOCIATION_RETRY_LIMIT
			) {
				if (this.#spillTerminalExit(event, 'association-pending')) {
					runtime.releasePendingExit?.(event);
					this.#deleteTerminalExitIfCurrent(key, event, false);
					this.#writeCopyPathTrace('terminal-exit-association-spilled');
				} else {
					this.#writeCopyPathTrace('terminal-exit-spill-full');
				}
			}
			return;
		}
		if (outcome.kind === 'unattributed') {
			this.#unknownIdentityAttempts.add(key);
			if (this.#spillTerminalExit(event, 'unknown-identity')) {
				this.#deleteTerminalExitIfCurrent(key, event, false);
				this.#writeCopyPathTrace('terminal-exit-unknown-identity');
			} else {
				this.#writeCopyPathTrace('terminal-exit-spill-full');
			}
			return;
		}
		if (outcome.kind === 'committed') {
			const publication = this.#publishTerminalExit(
				runtime,
				event,
				outcome.presentationId,
				(documentEvent) => {
					const coordinator = this.#clientSessionCoordinator;
					if (coordinator === undefined)
						throw new Error('Desktop client session coordinator is unavailable.');
					return coordinator.publishTerminalEvent(documentEvent);
				},
			);
			if (publication === 'pending') return;
			if (publication === 'committed') this.#writeCopyPathTrace('terminal-exited');
			this.#deleteTerminalExitIfCurrent(key, event);
			return;
		}
		if (outcome.kind === 'ignored') {
			// A close lifecycle can consume the exit while retiring its binding;
			// there is no publication to commit or spill in that case.
			if (outcome.reason === 'closed' || this.#isTerminalExitPublished(event)) {
				this.#deleteTerminalExitIfCurrent(key, event);
			}
			return;
		}
		if (outcome.kind === 'durable' || outcome.kind === 'published') {
			const publication = this.#publishTerminalExit(
				runtime,
				event,
				outcome.presentationId,
				(documentEvent) => {
					const coordinator = this.#clientSessionCoordinator;
					if (coordinator === undefined)
						throw new Error('Desktop client session coordinator is unavailable.');
					return coordinator.publishTerminalEvent(documentEvent);
				},
			);
			if (publication === 'pending') return;
			if (publication === 'committed') this.#writeCopyPathTrace('terminal-exited');
		}
		this.#deleteTerminalExitIfCurrent(key, event);
	}

	async #recordTerminalExit(
		runtime: DesktopTerminalRuntime,
		event: TerminalExited,
	): Promise<DesktopTerminalExitRecordResult> {
		if (typeof runtime.recordExitOutcome === 'function') {
			return runtime.recordExitOutcome(event);
		}
		const presentationId = await runtime.recordExit(event);
		return presentationId === null
			? { kind: 'ignored' }
			: { finalRevision: event.finalRevision, kind: 'durable', presentationId };
	}

	#removePublishedTerminalExitFromPending(event: TerminalExited): void {
		if (!this.#terminalExitSpillReady) return;
		try {
			this.#terminalExitSpillStore.remove(event);
		} catch {
			// The publication ledger and reconciler are already authoritative. Keep
			// the pending journal entry as a durable compaction retry if this cleanup
			// write fails.
			this.#writeCopyPathTrace('terminal-exit-pending-journal-compaction-failed');
		}
	}

	#deleteTerminalExitIfCurrent(key: string, event: TerminalExited, removeSpill = true): void {
		const current = this.#pendingTerminalExits.get(key);
		if (current !== undefined && BigInt(current.finalRevision) <= BigInt(event.finalRevision)) {
			this.#pendingTerminalExits.delete(key);
			this.#terminalExitAttemptCounts.delete(key);
			if (removeSpill) this.#removePublishedTerminalExitFromPending(event);
		}
	}

	#scheduleTerminalExitRetry(): void {
		if (
			this.#terminalExitRetryOwnerDisposed ||
			this.#terminalExitRetryOwnerClosing ||
			(this.#pendingTerminalExits.size === 0 && !this.#hasRetryableSpilledTerminalExits()) ||
			this.#terminalExitRetryTimer !== undefined
		) {
			return;
		}
		const timer = setTimeout(() => {
			this.#terminalExitRetryTimer = undefined;
			this.#promoteSpilledTerminalExits();
			for (const key of this.#pendingTerminalExits.keys()) void this.#attemptTerminalExit(key);
			this.#scheduleTerminalExitRetry();
		}, TERMINAL_EXIT_RETRY_DELAY_MILLISECONDS);
		timer.unref?.();
		this.#terminalExitRetryTimer = timer;
	}

	async #disposeTerminalExitRetryOwner(): Promise<void> {
		if (this.#terminalExitRetryOwnerDisposed && this.#pendingTerminalExits.size === 0) return;
		this.#terminalExitRetryOwnerClosing = true;
		if (this.#terminalExitRetryTimer !== undefined) {
			clearTimeout(this.#terminalExitRetryTimer);
			this.#terminalExitRetryTimer = undefined;
		}
		await Promise.all(this.#terminalExitAttempts.values());
		for (const key of this.#pendingTerminalExits.keys()) {
			await this.#attemptTerminalExit(key);
		}
		await Promise.all(this.#terminalExitAttempts.values());
		if (this.#pendingTerminalExits.size > 0) {
			let spillFailure: Error | undefined;
			for (const [key, event] of this.#pendingTerminalExits) {
				try {
					if (!this.#spillTerminalExit(event, 'shutdown')) {
						spillFailure = new Error('Desktop terminal exit spill admission is full.');
						break;
					}
					this.#deleteTerminalExitIfCurrent(key, event, false);
				} catch (error: unknown) {
					spillFailure = error instanceof Error ? error : new Error('Terminal exit spill failed.');
					break;
				}
			}
			if (spillFailure !== undefined) {
				this.#terminalExitRetryOwnerClosing = false;
				this.#writeCopyPathTrace('terminal-exit-retry-pending-on-dispose');
				throw spillFailure;
			}
		}
		this.#terminalExitRetryOwnerDisposed = true;
		this.#terminalExitRetryOwnerClosing = false;
	}

	#spillTerminalExit(event: TerminalExited, reason: TerminalExitSpillReason): boolean {
		if (!this.#terminalExitSpillReady) return false;
		return this.#terminalExitSpillStore.admit(event, reason).kind === 'admitted';
	}

	#hasRetryableSpilledTerminalExits(): boolean {
		if (!this.#terminalExitSpillReady) return false;
		return this.#terminalExitSpillStore.list().some((entry) => {
			if (this.#isTerminalExitPublished(entry.event)) return false;
			if (
				entry.reason === 'host-admission' ||
				entry.reason === 'queue-full' ||
				entry.reason === 'shutdown'
			) {
				return true;
			}
			const key = terminalExitIdentityKey(entry.event);
			return this.#associatedTerminalExitKeys.has(key) && !this.#unknownIdentityAttempts.has(key);
		});
	}

	#promoteSpilledTerminalExits(): void {
		if (!this.#terminalExitSpillReady) return;
		const available = MAX_PENDING_TERMINAL_EXIT_RETRIES - this.#pendingTerminalExits.size;
		if (available <= 0) return;
		const candidates = this.#terminalExitSpillStore
			.list()
			.filter((entry) => {
				if (this.#isTerminalExitPublished(entry.event)) {
					this.#removePublishedTerminalExitFromPending(entry.event);
					return false;
				}
				if (
					entry.reason === 'host-admission' ||
					entry.reason === 'queue-full' ||
					entry.reason === 'shutdown'
				) {
					return true;
				}
				const key = terminalExitIdentityKey(entry.event);
				return this.#associatedTerminalExitKeys.has(key) && !this.#unknownIdentityAttempts.has(key);
			})
			.slice(0, available);
		for (const entry of candidates) {
			const key = terminalExitIdentityKey(entry.event);
			if (this.#pendingTerminalExits.has(key)) continue;
			this.#pendingTerminalExits.set(key, entry.event);
			this.#terminalExitAttemptCounts.set(key, 0);
			void this.#attemptTerminalExit(key);
		}
	}

	readonly #handleTerminalIdentityObserved = (identity: TerminalIdentity): void => {
		const key = terminalExitIdentityKey(identity);
		this.#publishPendingTerminalMetadata(key);
		this.#associatedTerminalExitKeys.add(key);
		this.#unknownIdentityAttempts.delete(key);
		if (this.#pendingTerminalExits.has(key)) {
			void this.#attemptTerminalExit(key);
		} else {
			this.#promoteSpilledTerminalExits();
		}
		this.#scheduleTerminalExitRetry();
	};

	#writeCopyPathTrace(reason: string): void {
		if (!this.#options.traceCopyPath || this.#connection === undefined) return;
		try {
			this.#dependencies.writeTrace({
				brokers: this.#terminalRuntime?.getProjectionMetrics() ?? [],
				hostTransport: this.#connection.metrics,
				reason,
				type: 'orchbee.desktop.copy-path',
			});
		} catch {
			// Diagnostics are subordinate to the durable owner and cannot change its
			// retry or rejection state.
		}
	}
}

function isPermittedDesktopRendererUrl(url: URL): boolean {
	if (url.username !== '' || url.password !== '') return false;
	return (
		(url.protocol === 'http:' && url.hostname === '127.0.0.1') ||
		(url.protocol === 'orchbee:' && url.hostname === 'app' && url.port === '')
	);
}

function toDesktopWindowLayoutDocument(
	document: WorkbenchSharedDocument,
): DesktopWindowLayoutProductDocument {
	return {
		terminalContexts: document.terminalContexts.map((context) => ({
			tabIds: context.tabCollection.tabs.map((tab) => tab.id),
			tabs: context.tabCollection.tabs.map((tab) => ({ group: tab.group, id: tab.id })),
			terminalContextId: context.terminalContextId,
		})),
	};
}

function completeDesktopWindowBounds(
	bounds: DesktopWindowBounds | Readonly<{ height: number; width: number }>,
	topology: DesktopWindowDisplayTopology,
): DesktopWindowBounds {
	if ('x' in bounds && 'y' in bounds) return bounds;
	const workArea = topology.primaryWorkArea;
	return {
		height: bounds.height,
		width: bounds.width,
		x: Math.round(workArea.x + (workArea.width - bounds.width) / 2),
		y: Math.round(workArea.y + (workArea.height - bounds.height) / 2),
	};
}

function isSameDesktopRendererOrigin(value: string, rendererUrl: URL): boolean {
	let navigationUrl: URL;
	try {
		navigationUrl = new URL(value);
	} catch {
		return false;
	}
	return (
		isPermittedDesktopRendererUrl(navigationUrl) &&
		navigationUrl.protocol === rendererUrl.protocol &&
		navigationUrl.hostname === rendererUrl.hostname &&
		navigationUrl.port === rendererUrl.port
	);
}

function collectDocumentPresentationIds(
	document: WorkbenchSharedDocument,
): ReadonlySet<ReturnType<typeof TerminalPresentationIdSchema.parse>> {
	const presentationIds = new Set<ReturnType<typeof TerminalPresentationIdSchema.parse>>();
	const visit = (node: WorkbenchSharedPanelNode): void => {
		if (node.kind === 'panel') {
			presentationIds.add(node.primaryPresentation.presentationId);
			for (const presentation of node.childPresentations) {
				presentationIds.add(presentation.presentationId);
			}
			return;
		}
		for (const child of node.children) visit(child.node);
	};
	for (const context of document.terminalContexts) {
		for (const tab of context.panelWorkspace.tabs) visit(tab.root);
	}
	return presentationIds;
}

function collectDocumentArchivePresentationIds(
	document: WorkbenchSharedDocument,
): ReadonlySet<TerminalPresentationId> {
	const presentationIds = new Set<TerminalPresentationId>();
	const visit = (node: WorkbenchSharedPanelNode): void => {
		if (node.kind === 'panel') {
			for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
				if (presentation.intent.kind === 'stop' || presentation.intent.kind === 'stopped') {
					presentationIds.add(presentation.presentationId);
				}
			}
			return;
		}
		for (const child of node.children) visit(child.node);
	};
	for (const context of document.terminalContexts) {
		for (const tab of context.panelWorkspace.tabs) visit(tab.root);
	}
	return presentationIds;
}

function sameTerminalPresentationIds(
	left: readonly TerminalPresentationId[],
	right: readonly TerminalPresentationId[],
): boolean {
	return left.length === right.length && left.every((value, index) => value === right[index]);
}

function typedExitFailure(
	code: DesktopApplicationExitFailure['code'],
	count: number,
	retryable: boolean,
): DesktopApplicationExitFailure {
	return { code, count, retryable };
}

function closeRendererChannel(
	channel: { readonly port1: MessagePortMain; readonly port2: MessagePortMain } | undefined,
): void {
	if (channel === undefined) return;
	try {
		channel.port1.close();
	} catch {
		// Renderer bootstrap rollback is best effort after ownership is retained.
	}
	try {
		channel.port2.close();
	} catch {
		// Renderer bootstrap rollback is best effort after ownership is retained.
	}
}

function toDocumentExit(
	presentationId: Parameters<DesktopTerminalRuntime['getProjectionTarget']>[0],
	exited: TerminalExited,
): Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }> {
	if (exited.exit.kind === 'exit-code') {
		return { exitCode: exited.exit.code, presentationId, signal: null, type: 'terminal-exited' };
	}
	if (exited.exit.kind === 'signal') {
		return {
			exitCode: null,
			presentationId,
			signal: signalName(exited.exit.signal),
			type: 'terminal-exited',
		};
	}
	return { exitCode: null, presentationId, signal: null, type: 'terminal-exited' };
}

function toDocumentFailure(
	presentationId: Parameters<DesktopTerminalRuntime['getProjectionTarget']>[0],
	failed: TerminalFailed,
): Extract<TerminalPresentationEvent, { readonly type: 'terminal-failed' }> {
	return {
		failure: {
			code: `terminal-runtime-${failed.failure.kind}-${failed.failure.stage}`,
			retryable: false,
		},
		presentationId,
		type: 'terminal-failed',
	};
}

function terminalExitIdentityKey(source: TerminalExited | TerminalIdentity): string {
	const identity = 'identity' in source ? source.identity : source;
	return `${identity.hostEpoch}:${identity.creationId}:${identity.terminalId}`;
}

function signalName(signal: number): string {
	for (const [name, value] of Object.entries(constants.signals)) {
		if (value === signal) return name;
	}
	return `SIGNAL_${signal}`;
}
