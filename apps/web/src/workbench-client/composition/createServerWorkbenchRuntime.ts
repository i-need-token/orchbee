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
	type WorkbenchShortcutPlatform,
	type WorkbenchWorkspaceLocationSource,
} from '../../workbench';
import type { TerminalPresentationId, WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import type { ApplicationLocaleSource } from '../../application-locale';
import { runApplicationCleanup } from '../../runApplicationCleanup';
import type { TerminalCompletionReminderObserver } from '../../terminal-feedback-settings';
import {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	type TerminalInputConfigurationSource,
} from '../../terminal-input-configuration';
import type { TerminalTrailingBlankRowsPreferenceSource } from '../../terminal-interaction-settings';
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
import { bindServerAgentBoard } from './bindServerAgentBoard';
import { bindServerConfiguredCommands } from './bindServerConfiguredCommands';
import { bindServerFocusControl } from './bindServerFocusControl';
import { bindServerWindowTitle, type ServerTitleWindow } from './bindServerWindowTitle';
import {
	createServerWorkbenchAuthority,
	createServerWorkspaceLocations,
	findServerPresentation,
	findServerTerminalBinding,
	nameServerTerminalTitle,
	readServerTerminalTask,
	presentServerTerminalColors,
	projectServerSnapshot,
	serverHostColors,
	serverTerminalBindingKey,
	settledServerTerminalLifecycle,
	ServerAgentNameSource,
	ServerFocusNewPanelSource,
	ServerLeaving,
	ServerTerminalAgentSource,
	ServerTerminalEndObserver,
	ServerTerminalProjectionClient,
	type ServerLaunchProfileSource,
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

export interface ServerWorkbenchRuntime {
	readonly actionRuntime: ActionRuntime;
	/** The names the Server reports for those Agents, read as the TUI's Agent rows read them. */
	readonly agentNameSource: WorkbenchAgentNameSource;
	/** The Agents the Server observes in this session's terminals. */
	readonly agentObservationSource: WorkbenchAgentSource;
	readonly authority: WorkbenchAuthority;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
	/** Each Workspace's branch or folder and divergence, read from the Server while shown. */
	readonly workspaceLocationSource: WorkbenchWorkspaceLocationSource;
	dispose(): void;
}

export interface CreateServerWorkbenchRuntimeOptions {
	readonly applicationLocaleSource: ApplicationLocaleSource;
	readonly connection: ServerConnection;
	/** Where New Workspace starts while the person works in no Workspace. */
	readonly defaultDirectory: string;
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
	readonly preferredLocales: readonly string[];
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
	readonly terminalTrailingBlankRowsPreferenceSource?:
		| TerminalTrailingBlankRowsPreferenceSource
		| undefined;
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
	applicationLocaleSource,
	connection,
	defaultDirectory,
	documentElement,
	focusRecoveryStorage,
	homeDirectory,
	launchProfiles,
	onPublicationFailure,
	preferredLocales,
	session,
	shortcutBindingSource,
	shortcutPlatform,
	terminalColorsSource,
	terminalCompletionReminderObserver,
	terminalHostName,
	terminalInputConfigurationSource = DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	terminalInteractionConfigurationSource = DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	terminalPlatformEffects = {},
	terminalPresentationConfigurationSource = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	terminalTrailingBlankRowsPreferenceSource,
	titleWindow,
}: CreateServerWorkbenchRuntimeOptions): ServerWorkbenchRuntime {
	let disposed = false;
	let locale = applicationLocaleSource.current();
	let intl = createOrchbeeIntl(locale);
	const initialPublication = projectServerSnapshot(session.snapshot, intl);
	const actionRuntime = new ActionRuntime();
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		initialClientView:
			focusRecoveryStorage === undefined
				? undefined
				: createFocusRecoveredWorkbenchClientView(
						initialPublication,
						readWorkbenchFocusRecovery(focusRecoveryStorage),
					),
		initialPublication,
	});
	const boundTerminal = (presentationId: TerminalPresentationId): string | null => {
		const binding = findServerTerminalBinding(session.snapshot, presentationId);
		return binding === null ? null : serverTerminalBindingKey(binding);
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
			onPublicationFailure(error);
		}
	};
	const stopPublication = session.subscribe(adoptPublication);
	// Registered after the publication's own adoption, so each view it reports holds that publication.
	const clientView = bindServerClientView({
		connection,
		runtime: projectWorkspaceRuntime,
		session,
	});
	const focusNewPanel = new ServerFocusNewPanelSource({
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
	const agentBoard = bindServerAgentBoard({
		connection,
		runtime: projectWorkspaceRuntime,
		agentObservationSource,
	});
	const authority = createServerWorkbenchAuthority({
		agentBoard: agentBoard.board,
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
		focusNewPanel,
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
	const applyTerminalMetadata = (
		presentationId: TerminalPresentationId,
		metadata: ServerTerminalMetadata,
	): void => {
		const observation = resolveTerminalMetadataObservation(metadata, terminalHostName);
		projectWorkspaceRuntime.applyTerminalMetadataObservation(presentationId, {
			...observation,
			naturalTask:
				observation.naturalTitle === null ? null : readServerTerminalTask(observation.naturalTitle),
			naturalTitle:
				observation.naturalTitle === null
					? null
					: nameServerTerminalTitle(observation.naturalTitle, intl),
		});
	};
	const reapplyTerminalMetadata = (): void => {
		for (const [presentationId, { metadata }] of metadataCursors) {
			applyTerminalMetadata(presentationId, metadata);
		}
	};
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
			applyTerminalMetadata(presentationId, metadata);
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
		readTerminalLaunchFailure: (presentationId) => {
			const failure = findServerPresentation(session.snapshot, presentationId)?.launchFailure;
			return failure === undefined ? null : { executable: failure.executable };
		},
		...(shortcutBindingSource === undefined ? {} : { shortcutBindingSource }),
		shortcutPlatform,
		...(terminalCompletionReminderObserver === undefined
			? {}
			: { terminalCompletionReminderObserver }),
		...(terminalTrailingBlankRowsPreferenceSource === undefined
			? {}
			: { trailingBlankRowsPreferenceSource: terminalTrailingBlankRowsPreferenceSource }),
	};

	return {
		actionRuntime,
		agentNameSource,
		agentObservationSource,
		authority,
		dispose: () => {
			if (disposed) return;
			disposed = true;
			metadataCursors.clear();
			presentationSnapshots.clear();
			runApplicationCleanup(
				[
					() => stopPublication(),
					() => agentBoard.dispose(),
					() => configuredCommands.dispose(),
					() => focusNewPanel.dispose(),
					() => clientView?.dispose(),
					() => terminalEnds.dispose(),
					() => stopLocaleObservation(),
					() => windowTitle?.dispose(),
					() => focusControl?.dispose(),
					() => stopFocusRecoveryTopologyObservation?.(),
					() => stopFocusRecoveryViewObservation?.(),
					() => actionRuntime.dispose(),
					() => agentNameSource.dispose(),
					() => agentObservationSource.dispose(),
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
