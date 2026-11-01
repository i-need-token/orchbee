import { ActionRuntime } from '@orchbee/actions';
import { createOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	type ProjectedTerminalPresentationSnapshot,
	type TerminalInteractionConfigurationSource,
} from '@orchbee/terminal';
import {
	resolveTerminalMetadataObservation,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	workbenchSharedDocumentRevision,
	type TerminalPresentationId,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import type { ApplicationLocaleSource } from '../../application-locale';
import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
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
import {
	findServerTerminalBinding,
	projectServerSnapshot,
	ServerTerminalProjectionClient,
	type ServerWorkbenchSession,
} from '../server-workbench';
import {
	createFocusRecoveredWorkbenchClientView,
	readWorkbenchFocusRecovery,
	resolveFocusedWorkbenchPresentationId,
	writeWorkbenchFocusRecovery,
	type WorkbenchFocusRecoveryStorage,
} from './workbench-focus-recovery-storage';

export interface ServerWorkbenchRuntime {
	readonly actionRuntime: ActionRuntime;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
	dispose(): void;
}

export interface CreateServerWorkbenchRuntimeOptions {
	readonly applicationLocaleSource: ApplicationLocaleSource;
	readonly connection: ServerConnection;
	/** Names the directory of a Workspace the Server holds without one. */
	readonly defaultDirectory: string;
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly focusRecoveryStorage?: WorkbenchFocusRecoveryStorage | undefined;
	/** A publication this Client cannot read leaves it behind the Server, so it must stop. */
	readonly onPublicationFailure: (error: unknown) => void;
	readonly preferredLocales: readonly string[];
	readonly session: ServerWorkbenchSession;
	readonly shortcutBindingSource?: ProjectedTerminalShortcutBindingSource | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
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
	onPublicationFailure,
	preferredLocales,
	session,
	shortcutBindingSource,
	shortcutPlatform,
	terminalCompletionReminderObserver,
	terminalHostName,
	terminalInputConfigurationSource = DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	terminalInteractionConfigurationSource = DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	terminalPlatformEffects = {},
	terminalPresentationConfigurationSource = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	terminalTrailingBlankRowsPreferenceSource,
}: CreateServerWorkbenchRuntimeOptions): ServerWorkbenchRuntime {
	let disposed = false;
	const initialPublication = projectServerSnapshot(session.snapshot, defaultDirectory);
	const actionRuntime = new ActionRuntime();
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		initialClientView:
			focusRecoveryStorage === undefined
				? undefined
				: createFocusRecoveredWorkbenchClientView(
						initialPublication.document,
						readWorkbenchFocusRecovery(focusRecoveryStorage),
					),
		initialPublication,
		standaloneCwd: defaultDirectory,
		standaloneTerminalTitle: createOrchbeeIntl(applicationLocaleSource.current()).formatMessage(
			workbenchTabMessages.terminalTitle,
		),
		terminalPresentation: {
			createOperationId: () =>
				terminalPresentationOperationId(`workbench:operation:${createBrowserRandomIdentifier()}`),
			createPresentationId: () =>
				terminalPresentationId(`workbench:presentation:${createBrowserRandomIdentifier()}`),
			createRemovalId: () =>
				terminalPresentationRemovalId(`workbench:removal:${createBrowserRandomIdentifier()}`),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId(
				`workbench:presentation:${createBrowserRandomIdentifier()}`,
			),
			kind: 'managed',
		},
	});
	const adoptPublication = (snapshot: WorkbenchSnapshot): void => {
		try {
			projectWorkspaceRuntime.applyPublishedWorkbench(
				projectServerSnapshot(snapshot, defaultDirectory),
			);
		} catch (error) {
			onPublicationFailure(error);
		}
	};
	const stopPublication = session.subscribe(adoptPublication);

	const persistFocusRecovery = (): void => {
		if (focusRecoveryStorage === undefined) return;
		writeWorkbenchFocusRecovery(
			focusRecoveryStorage,
			resolveFocusedWorkbenchPresentationId(
				projectWorkspaceRuntime.getSharedDocument(workbenchSharedDocumentRevision('0')),
				projectWorkspaceRuntime.getClientView(),
			),
		);
	};
	const stopFocusRecoveryDocumentObservation =
		focusRecoveryStorage === undefined
			? undefined
			: projectWorkspaceRuntime.subscribeDocument(persistFocusRecovery);
	const stopFocusRecoveryViewObservation =
		focusRecoveryStorage === undefined
			? undefined
			: projectWorkspaceRuntime.subscribeClientView(persistFocusRecovery);

	const projectionClient = new ServerTerminalProjectionClient(connection, () => session.snapshot);
	const presentationSnapshots =
		new TerminalPresentationSnapshotCache<ProjectedTerminalPresentationSnapshot>();
	// A revision only orders metadata of one terminal, and a Presentation can be bound to another.
	const metadataCursors = new Map<
		TerminalPresentationId,
		{ readonly revision: bigint; readonly terminal: string | null }
	>();
	const terminalSurfaceCapability: ProjectedTerminalSurfaceCapability = {
		applicationLocaleSource,
		documentElement,
		inputConfigurationSource: terminalInputConfigurationSource,
		interactionConfigurationSource: terminalInteractionConfigurationSource,
		observeTerminalMetadata: (presentationId, metadata) => {
			if (disposed) return;
			const binding = findServerTerminalBinding(session.snapshot, presentationId);
			const terminal = binding === null ? null : `${binding.workerId}:${binding.terminalId}`;
			const previous = metadataCursors.get(presentationId);
			if (previous?.terminal === terminal && previous.revision > metadata.revision) return;
			metadataCursors.set(presentationId, { revision: metadata.revision, terminal });
			projectWorkspaceRuntime.applyTerminalMetadataObservation(
				presentationId,
				resolveTerminalMetadataObservation(metadata, terminalHostName),
			);
		},
		platformEffects: terminalPlatformEffects,
		preferredLocales,
		// A Server terminal's colors come from its Host, so no renderer joins a theme presentation.
		presentationConfigurationSource: {
			current: () => terminalPresentationConfigurationSource.current(),
			subscribe: (listener) => terminalPresentationConfigurationSource.subscribe(listener),
		},
		presentationSnapshots,
		projectionClient,
		...(shortcutBindingSource === undefined ? {} : { shortcutBindingSource }),
		shortcutPlatform,
		startStoppedTerminalSession: (presentationId) =>
			projectWorkspaceRuntime.startStoppedTerminalSession(presentationId),
		...(terminalCompletionReminderObserver === undefined
			? {}
			: { terminalCompletionReminderObserver }),
		...(terminalTrailingBlankRowsPreferenceSource === undefined
			? {}
			: { trailingBlankRowsPreferenceSource: terminalTrailingBlankRowsPreferenceSource }),
	};

	return {
		actionRuntime,
		dispose: () => {
			if (disposed) return;
			disposed = true;
			metadataCursors.clear();
			presentationSnapshots.clear();
			runApplicationCleanup(
				[
					() => stopPublication(),
					() => stopFocusRecoveryDocumentObservation?.(),
					() => stopFocusRecoveryViewObservation?.(),
					() => actionRuntime.dispose(),
					() => projectionClient.dispose(),
				],
				'Server Workbench runtime cleanup failed',
			);
		},
		projectWorkspaceRuntime,
		terminalSurfaceCapability,
	};
}
