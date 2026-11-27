import { ActionRuntime } from '@orchbee/actions';
import { createOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	type ProjectedTerminalPresentationSnapshot,
	type TerminalInteractionConfigurationSource,
} from '@orchbee/terminal';
import {
	resolveActiveTerminalPresentationId,
	resolveTerminalMetadataObservation,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchAgentNameSource,
	type WorkbenchAgentSource,
	type WorkbenchAuthority,
	type WorkbenchClientView,
	type WorkbenchShortcutPlatform,
	type WorkbenchWorkspaceLocationSource,
} from '../../workbench';
import type { TerminalPresentationId, WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import type { ApplicationLocaleSource } from '../../application-locale';
import type { FileSessionPort } from '../../file-session';
import { runApplicationCleanup } from '../../runApplicationCleanup';
import type { TerminalCompletionReminderObserver } from '../../terminal-feedback-settings';
import {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	type TerminalInputConfigurationSource,
} from '../../terminal-input-configuration';
import { ContainedClientFault, reportContainedClientFault } from '../contained-client-fault';
import type { ProjectedTerminalShortcutBindingSource } from '../projected-terminal-actions';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../projected-terminal-renderer';
import {
	TerminalPresentationSnapshotCache,
	type ProjectedTerminalSurfaceCapability,
} from '../projected-terminal-surface';
import type { ServerConnection } from '../server-connection';
import { bindServerClientView } from './bindServerClientView';
import { bindServerConfiguredCommands } from './bindServerConfiguredCommands';
import { bindServerFocusControl } from './bindServerFocusControl';
import { bindServerWindowTitle, type ServerTitleWindow } from './bindServerWindowTitle';
import {
	createRelaunchPlaceWorkbenchClientView,
	type WorkbenchRelaunchPlace,
} from './createRelaunchPlaceWorkbenchClientView';
import {
	createServerWorkbenchAuthority,
	createServerWorkspaceLocations,
	findServerPresentation,
	findServerTerminalBinding,
	listServerTerminalBindings,
	mergeServerTerminalTitle,
	nameServerPublishedTitle,
	nameServerTerminalTitle,
	readServerPublishedTask,
	readServerTerminalTask,
	presentServerTerminalColors,
	projectServerSnapshot,
	serverHostColors,
	serverTerminalBindingKey,
	settledServerTerminalLifecycle,
	acknowledgeServerAgentCompletion,
	ServerAgentConfirmations,
	ServerAgentNameSource,
	ServerAgentCatalog,
	ServerFileSessionPort,
	ServerLeaving,
	ServerTerminalAgentSource,
	ServerTerminalEndObserver,
	ServerTerminalProjectionClient,
	ServerTerminalRuntime,
	ServerTerminalTitleSource,
	type ServerFileSessionOptions,
	type ServerLaunchProfileSource,
	type ServerTerminalTitle,
	type ServerTerminalTitleCursor,
	type ServerTerminalColorsSource,
	type ServerWorkbenchSession,
} from '../server-workbench';
import {
	createFocusRecoveredWorkbenchClientView,
	readWorkbenchFocusRecovery,
	writeWorkbenchFocusRecovery,
	type WorkbenchFocusRecoveryStorage,
} from './workbench-focus-recovery-storage';

type ServerTerminalMetadata = Parameters<
	ProjectedTerminalSurfaceCapability['observeTerminalMetadata']
>[1];
type TerminalCurrentDirectory = ReturnType<
	typeof resolveTerminalMetadataObservation
>['currentDirectory'];

export interface ServerWorkbenchRuntime {
	readonly actionRuntime: ActionRuntime;
	/** The names the Server reports for those Agents, read as the TUI's Agent rows read them. */
	readonly agentNameSource: WorkbenchAgentNameSource;
	/** The Agents the Server observes in this session's terminals. */
	readonly agentObservationSource: WorkbenchAgentSource;
	readonly authority: WorkbenchAuthority;
	/** The Session's file sessions, present where the window offers the File Bar and it serves them. */
	readonly fileSessionPort: FileSessionPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
	/** Each Workspace's branch or folder and divergence, read from the Server while shown. */
	readonly workspaceLocationSource: WorkbenchWorkspaceLocationSource;
	dispose(): void;
}

export interface CreateServerWorkbenchRuntimeOptions {
	readonly applicationLocaleSource: ApplicationLocaleSource;
	readonly connection: ServerConnection;
	/** Windows: Ctrl+C copies a terminal selection before it can interrupt the program. */
	readonly ctrlCCopiesSelection?: boolean | undefined;
	/** Where New Workspace starts while the person works in no Workspace. */
	readonly defaultDirectory: string;
	/**
	 * Opens file sessions over the connection for the File Bar; absent where the window does not
	 * offer it or the Session's handshake lacks `files`.
	 */
	readonly fileSessions?: Omit<ServerFileSessionOptions, 'connection'> | undefined;
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'> &
		Partial<Pick<HTMLElement, 'ownerDocument'>>;
	readonly focusRecoveryStorage?: WorkbenchFocusRecoveryStorage | undefined;
	/**
	 * The person's home folder where the Server runs, which a Workspace folder under it shows as `~`;
	 * absent where this Client cannot know it.
	 */
	readonly homeDirectory?: string | undefined;
	/** The Session's saved launch profiles, which the New Tab button offers. */
	readonly launchProfiles?: ServerLaunchProfileSource | undefined;
	/** A publication this Client cannot read leaves it behind the Server, so it must stop. */
	readonly onPublicationFailure: (error: unknown) => void;
	/** Opens the Quit All confirmation, which a terminal offers once its Server gave up on it. */
	readonly quitAll?: (() => void) | undefined;
	readonly preferredLocales: readonly string[];
	/**
	 * Where a window relaunched by Update opens, which takes the place of focus recovery; absent or
	 * null for every other start.
	 */
	readonly relaunchPlace?: WorkbenchRelaunchPlace | null | undefined;
	readonly session: ServerWorkbenchSession;
	readonly shortcutBindingSource?: ProjectedTerminalShortcutBindingSource | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	/** What the window paints default-coded cells with, and the colors it reports to a terminal. */
	readonly terminalColorsSource?: ServerTerminalColorsSource | undefined;
	readonly terminalCompletionReminderObserver?: TerminalCompletionReminderObserver | undefined;
	readonly terminalHostName: string;
	readonly terminalInputConfigurationSource?: TerminalInputConfigurationSource | undefined;
	readonly terminalInteractionConfigurationSource?:
		| TerminalInteractionConfigurationSource
		| undefined;
	readonly terminalPlatformEffects?: ProjectedTerminalPlatformEffects | undefined;
	readonly terminalPresentationConfigurationSource?:
		| TerminalPresentationConfigurationSource
		| undefined;
	/**
	 * Whether the Session's Server takes a completion acknowledgement besides typed input, as its
	 * handshake answer lists `agent-completion-acknowledge`.
	 */
	readonly agentCompletionAcknowledgement?: boolean | undefined;
	/**
	 * Whether the Session's Server answers Agent confirmations, as its handshake answer lists
	 * `agent-confirmation`. Without it, the window neither reads nor changes them.
	 */
	readonly agentConfirmations?: boolean | undefined;
	/**
	 * Whether the Session's Server publishes `workbench.terminal-runtime`, as its handshake answer
	 * lists `terminal-runtime`. Without it, every terminal keeps its own recovery.
	 */
	readonly terminalRuntime?: boolean | undefined;
	/**
	 * The window named after the Session's title policy, as the TUI names its outer terminal; absent
	 * where this Client names no window.
	 */
	readonly titleWindow?: ServerTitleWindow | undefined;
}

/**
 * A Workbench whose shared state is whatever the Server published. Nothing here decides a terminal's
 * lifetime, so no local Presentation identity is ever consumed and first entry creates nothing.
 */
export function createServerWorkbenchRuntime({
	agentCompletionAcknowledgement = false,
	agentConfirmations = false,
	applicationLocaleSource,
	connection,
	defaultDirectory,
	documentElement,
	fileSessions,
	focusRecoveryStorage,
	homeDirectory,
	launchProfiles,
	onPublicationFailure,
	preferredLocales,
	quitAll,
	relaunchPlace,
	session,
	shortcutBindingSource,
	shortcutPlatform,
	ctrlCCopiesSelection = false,
	terminalColorsSource,
	terminalCompletionReminderObserver,
	terminalHostName,
	terminalInputConfigurationSource = DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	terminalInteractionConfigurationSource = DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	terminalPlatformEffects = {},
	terminalPresentationConfigurationSource = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	terminalRuntime = false,
	titleWindow,
}: CreateServerWorkbenchRuntimeOptions): ServerWorkbenchRuntime {
	let disposed = false;
	let locale = applicationLocaleSource.current();
	let intl = createOrchbeeIntl(locale);
	const initialPublication = projectServerSnapshot(session.snapshot, intl);
	const actionRuntime = new ActionRuntime();
	const fileSessionPort =
		fileSessions === undefined
			? undefined
			: new ServerFileSessionPort({ ...fileSessions, connection });
	// A relaunch's place takes the place of focus recovery.
	const createInitialClientView = (): WorkbenchClientView | undefined => {
		if (relaunchPlace !== undefined && relaunchPlace !== null) {
			return createRelaunchPlaceWorkbenchClientView(initialPublication, relaunchPlace);
		}
		return focusRecoveryStorage === undefined
			? undefined
			: createFocusRecoveredWorkbenchClientView(
					initialPublication,
					readWorkbenchFocusRecovery(focusRecoveryStorage),
				);
	};
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		initialClientView: createInitialClientView(),
		initialPublication,
	});
	const boundTerminal = (presentationId: TerminalPresentationId): string | null => {
		const binding = findServerTerminalBinding(session.snapshot, presentationId);
		return binding === null ? null : serverTerminalBindingKey(binding);
	};
	// Each Presentation's title, from its mounted surface or from the titles the Server publishes for
	// every terminal, whichever arrived last for the terminal it shows.
	const titleCursors = new Map<TerminalPresentationId, ServerTerminalTitleCursor>();
	const applyTerminalTitle = (
		presentationId: TerminalPresentationId,
		currentDirectory: TerminalCurrentDirectory,
		title: ServerTerminalTitle | null,
	): void => {
		let naturalTask: string | null = null;
		let naturalTitle: string | null = null;
		// The Server already stripped what it published, so only a surface's own title is stripped.
		if (title?.published === true) {
			naturalTask = readServerPublishedTask(title.text);
			naturalTitle = nameServerPublishedTitle(title.text, intl);
		} else if (title !== null) {
			naturalTask = readServerTerminalTask(title.text);
			naturalTitle = nameServerTerminalTitle(title.text, intl);
		}
		projectWorkspaceRuntime.applyTerminalMetadataObservation(presentationId, {
			currentDirectory,
			naturalTask,
			naturalTitle,
		});
	};
	const mergeTerminalTitle = (
		presentationId: TerminalPresentationId,
		terminal: string | null,
		arrival: Parameters<typeof mergeServerTerminalTitle>[2],
	): void => {
		const { applied, cursor } = mergeServerTerminalTitle(
			titleCursors.get(presentationId),
			terminal,
			arrival,
		);
		if (cursor === undefined) return;
		titleCursors.set(presentationId, cursor);
		if (applied !== null) {
			applyTerminalTitle(presentationId, applied.currentDirectory, applied.title);
		}
	};
	// A publication can move a Presentation to another terminal, or remove it.
	const followTerminalTitles = (snapshot: WorkbenchSnapshot): void => {
		if (titleCursors.size === 0) return;
		const terminals = listServerTerminalBindings(snapshot);
		for (const presentationId of titleCursors.keys()) {
			const terminal = terminals.get(presentationId);
			if (terminal === undefined) {
				titleCursors.delete(presentationId);
			} else {
				mergeTerminalTitle(presentationId, terminal, { kind: 'terminal' });
			}
		}
	};
	const terminalEnds = new ServerTerminalEndObserver(
		() => session.snapshot,
		() => adoptPublication(session.snapshot),
	);
	const leaving = new ServerLeaving(
		() => session.snapshot,
		(returned) => {
			adoptPublication(session.snapshot);
			// A member that left took what its terminals reported with it, so it shows that again.
			if (returned) reapplyTerminalMetadata();
		},
	);
	const adoptPublication = (snapshot: WorkbenchSnapshot): void => {
		terminalEnds.prune(snapshot);
		leaving.prune(snapshot);
		try {
			projectWorkspaceRuntime.applyPublishedWorkbench(
				projectServerSnapshot(snapshot, intl, terminalEnds.ends, leaving),
			);
		} catch (error) {
			reportContainedClientFault(new ContainedClientFault('publication-apply', error));
			onPublicationFailure(error);
			return;
		}
		followTerminalTitles(snapshot);
	};
	const stopPublication = session.subscribe(adoptPublication);
	// Registered after the publication's own adoption, so each view it reports holds that publication.
	const clientView = bindServerClientView({
		connection,
		runtime: projectWorkspaceRuntime,
		session,
	});
	const agentCatalog = new ServerAgentCatalog({
		connection,
		sessionId: session.snapshot.sessionId,
	});
	const configuredCommands = bindServerConfiguredCommands({
		clientView,
		connection,
		runtime: projectWorkspaceRuntime,
		session,
	});
	const agentObservationSource = new ServerTerminalAgentSource(connection, session);
	const confirmations = agentConfirmations
		? new ServerAgentConfirmations(connection, session)
		: undefined;
	const authority = createServerWorkbenchAuthority({
		acknowledgeAgentCompletion: agentCompletionAcknowledgement
			? (presentationId) => acknowledgeServerAgentCompletion(session, presentationId)
			: undefined,
		agentCatalog,
		agentConfirmations: confirmations,
		configuredCommands: configuredCommands.commands,
		defaultDirectory,
		defaultWorkspaceLabel: () =>
			createOrchbeeIntl(applicationLocaleSource.current()).formatMessage(
				workbenchMessages.workspaceDefaultName,
			),
		focusedWorkspaceId: () => {
			const location = projectWorkspaceRuntime.getSnapshot().activeLocation;
			return location.kind === 'workspace' ? location.workspaceId : null;
		},
		...(launchProfiles === undefined ? {} : { launchProfiles }),
		leave: leaving,
		session,
	});

	// A publication can replace the terminal a Panel shows and this Client can move its focus, so both
	// are watched; only a different terminal is written.
	let persistedPresentationId: TerminalPresentationId | null | undefined;
	const persistFocusRecovery = (): void => {
		if (focusRecoveryStorage === undefined) return;
		const presentationId = resolveActiveTerminalPresentationId(projectWorkspaceRuntime);
		if (presentationId === persistedPresentationId) return;
		if (writeWorkbenchFocusRecovery(focusRecoveryStorage, presentationId)) {
			persistedPresentationId = presentationId;
		}
	};
	const stopFocusRecoveryTopologyObservation =
		focusRecoveryStorage === undefined
			? undefined
			: projectWorkspaceRuntime.subscribeTerminalTopology(persistFocusRecovery);
	const stopFocusRecoveryViewObservation =
		focusRecoveryStorage === undefined
			? undefined
			: projectWorkspaceRuntime.subscribeClientView(persistFocusRecovery);

	const projectionClient = new ServerTerminalProjectionClient(
		connection,
		() => session.snapshot,
		terminalColorsSource === undefined ? undefined : serverHostColors(terminalColorsSource),
		(presentationId, binding, settlement) => {
			terminalEnds.observe(presentationId, binding, settledServerTerminalLifecycle(settlement));
		},
	);
	const agentNameSource = new ServerAgentNameSource(connection, session, agentObservationSource);
	const workspaceLocationSource = createServerWorkspaceLocations({
		homeDirectory: homeDirectory ?? null,
		session,
	});
	const presentationSnapshots =
		new TerminalPresentationSnapshotCache<ProjectedTerminalPresentationSnapshot>();
	// A revision only orders metadata of one terminal, and a Presentation can be bound to another.
	// The latest metadata stays so that a language change can name its title again.
	const metadataCursors = new Map<
		TerminalPresentationId,
		{
			readonly metadata: ServerTerminalMetadata;
			readonly revision: bigint;
			readonly terminal: string | null;
		}
	>();
	const reapplyTerminalMetadata = (): void => {
		for (const [presentationId, { shown }] of titleCursors) {
			const cursor = metadataCursors.get(presentationId);
			applyTerminalTitle(
				presentationId,
				cursor === undefined
					? { kind: 'ignored' }
					: resolveTerminalMetadataObservation(cursor.metadata, terminalHostName).currentDirectory,
				shown,
			);
		}
	};
	const titleSource = new ServerTerminalTitleSource(connection, session, (rows) => {
		if (disposed) return;
		const terminals = listServerTerminalBindings(session.snapshot);
		for (const { binding, presentationId, title } of rows) {
			mergeTerminalTitle(presentationId, terminals.get(presentationId) ?? null, {
				binding,
				kind: 'stream',
				title,
			});
		}
	});
	const terminalRuntimeSource = terminalRuntime
		? new ServerTerminalRuntime({ connection, sessionId: session.snapshot.sessionId })
		: undefined;
	// The Server closes Agent streams while it replaces a lost Worker; serving again reopens them.
	let terminalRuntimeState = terminalRuntimeSource?.getSnapshot() ?? 'serving';
	const stopTerminalRuntimeObservation = terminalRuntimeSource?.subscribe(() => {
		const previous = terminalRuntimeState;
		terminalRuntimeState = terminalRuntimeSource.getSnapshot();
		if (disposed || previous === 'serving' || terminalRuntimeState !== 'serving') return;
		agentObservationSource.reopenClosed();
		confirmations?.reopenClosed();
		titleSource.reopenClosed();
	});
	const windowTitle =
		titleWindow === undefined
			? null
			: bindServerWindowTitle({
					connection,
					runtime: projectWorkspaceRuntime,
					session,
					// As the TUI reads only the live frame, a terminal the Presentation no longer shows names
					// nothing.
					terminalTitle: (presentationId) => {
						const cursor = metadataCursors.get(presentationId);
						const terminal = boundTerminal(presentationId);
						if (cursor === undefined || terminal === null || cursor.terminal !== terminal) {
							return null;
						}
						const { naturalTitle } = resolveTerminalMetadataObservation(
							cursor.metadata,
							terminalHostName,
						);
						return naturalTitle === null ? null : nameServerTerminalTitle(naturalTitle, intl);
					},
					titleWindow,
				});
	const focusControl =
		titleWindow === undefined
			? null
			: bindServerFocusControl({
					readTarget: () => resolveActiveTerminalPresentationId(projectWorkspaceRuntime),
					subscribeNavigation: projectWorkspaceRuntime.subscribeNavigation,
					projectionClient,
					titleWindow,
					ownerDocument: documentElement.ownerDocument,
				});

	const stopLocaleObservation = applicationLocaleSource.subscribe(() => {
		const next = applicationLocaleSource.current();
		if (disposed || next === locale) return;
		locale = next;
		intl = createOrchbeeIntl(next);
		adoptPublication(session.snapshot);
		reapplyTerminalMetadata();
		windowTitle?.refresh();
	});
	const terminalSurfaceCapability: ProjectedTerminalSurfaceCapability = {
		agentObservationSource,
		applicationLocaleSource,
		documentElement,
		inputConfigurationSource: terminalInputConfigurationSource,
		interactionConfigurationSource: terminalInteractionConfigurationSource,
		observeTerminalMetadata: (presentationId, metadata) => {
			if (disposed) return;
			const terminal = boundTerminal(presentationId);
			const previous = metadataCursors.get(presentationId);
			if (previous?.terminal === terminal && previous.revision > metadata.revision) return;
			metadataCursors.set(presentationId, { metadata, revision: metadata.revision, terminal });
			const { currentDirectory, naturalTitle } = resolveTerminalMetadataObservation(
				metadata,
				terminalHostName,
			);
			mergeTerminalTitle(presentationId, terminal, {
				currentDirectory,
				kind: 'surface',
				title: naturalTitle,
			});
			windowTitle?.terminalTitleChanged(presentationId);
		},
		platformEffects: terminalPlatformEffects,
		preferredLocales,
		// The Server offers no Host theme transaction. As in the TUI, the window paints default-coded
		// cells in the selected theme's colors, and a terminal following its controller takes them too.
		presentationConfigurationSource: presentServerTerminalColors(
			terminalPresentationConfigurationSource,
			terminalColorsSource,
		),
		presentationSnapshots,
		projectionClient,
		quitAll,
		readTerminalLaunchFailure: (presentationId) => {
			const failure = findServerPresentation(session.snapshot, presentationId)?.launchFailure;
			if (failure === undefined) return null;
			return failure.reason === 'directory-unavailable' && failure.directory !== undefined
				? {
						agentResume: failure.agentResume === true,
						directory: failure.directory,
						kind: 'directory-unavailable',
					}
				: { executable: failure.executable, kind: 'failed' };
		},
		...(shortcutBindingSource === undefined ? {} : { shortcutBindingSource }),
		shortcutPlatform,
		ctrlCCopiesSelection,
		...(terminalCompletionReminderObserver === undefined
			? {}
			: { terminalCompletionReminderObserver }),
		terminalRuntimeSource,
	};

	return {
		actionRuntime,
		agentNameSource,
		agentObservationSource,
		authority,
		fileSessionPort,
		dispose: () => {
			if (disposed) return;
			disposed = true;
			metadataCursors.clear();
			titleCursors.clear();
			presentationSnapshots.clear();
			runApplicationCleanup(
				[
					() => stopPublication(),
					() => fileSessionPort?.dispose(),
					() => agentCatalog.dispose(),
					() => configuredCommands.dispose(),
					() => clientView?.dispose(),
					() => terminalEnds.dispose(),
					() => stopLocaleObservation(),
					() => windowTitle?.dispose(),
					() => focusControl?.dispose(),
					() => stopFocusRecoveryTopologyObservation?.(),
					() => stopFocusRecoveryViewObservation?.(),
					() => actionRuntime.dispose(),
					() => agentNameSource.dispose(),
					() => stopTerminalRuntimeObservation?.(),
					() => terminalRuntimeSource?.dispose(),
					() => agentObservationSource.dispose(),
					() => confirmations?.dispose(),
					() => titleSource.dispose(),
					() => workspaceLocationSource.dispose(),
					() => projectionClient.dispose(),
				],
				'Server Workbench runtime cleanup failed',
			);
		},
		projectWorkspaceRuntime,
		terminalSurfaceCapability,
		workspaceLocationSource,
	};
}
