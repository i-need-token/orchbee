import { ActionRuntime } from '@orchbee/actions';
import { createOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	type ProjectedTerminalPresentationSnapshot,
	type TerminalInteractionConfigurationSource,
} from '@orchbee/terminal';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	resolveTerminalMetadataObservation,
	resolveTerminalNaturalMetadataObservation,
	WorkbenchProjectWorkspaceRuntime,
	type TerminalPresentationId,
	type WorkbenchTerminalPresentationComposition,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
import type { ApplicationLocaleSource } from '../../application-locale';
import type { WorkbenchClientSessionClient } from '../client-session';
import {
	TerminalPresentationSnapshotCache,
	type ProjectedTerminalSurfaceCapability,
} from '../projected-terminal-surface';
import type { ProjectedTerminalShortcutBindingSource } from '../projected-terminal-actions';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../projected-terminal-renderer';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import type { TerminalProjectionClient } from '../terminal-projection';
import { WorkbenchDocumentCoordinator } from '../workbench-document-control';
import { runWorkbenchClientCleanup } from '../runWorkbenchClientCleanup';
import type { TerminalFileDropCapability } from '../../terminal-file-drop';
import type { TerminalCompletionReminderObserver } from '../../terminal-feedback-settings';
import type { TerminalTrailingBlankRowsPreferenceSource } from '../../terminal-interaction-settings';
import {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	type TerminalInputConfigurationSource,
} from '../../terminal-input-configuration';
import {
	createFocusRecoveredWorkbenchClientView,
	readWorkbenchFocusRecovery,
	resolveFocusedWorkbenchPresentationId,
	writeWorkbenchFocusRecovery,
	type WorkbenchFocusRecoveryStorage,
} from './workbench-focus-recovery-storage';

export interface WorkbenchClientRuntime {
	readonly actionRuntime: ActionRuntime;
	readonly applicationLocaleSource: ApplicationLocaleSource;
	readonly documentCoordinator: WorkbenchDocumentCoordinator;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly ready: Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	>;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
	dispose(): void;
}

export interface CreateWorkbenchClientRuntimeOptions {
	readonly applicationLocaleSource: ApplicationLocaleSource;
	readonly clientSession: WorkbenchClientSessionClient;
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly focusRecoveryStorage?: WorkbenchFocusRecoveryStorage;
	readonly preferredLocales: readonly string[];
	readonly projectionClient: TerminalProjectionClient;
	readonly shortcutBindingSource?: ProjectedTerminalShortcutBindingSource;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalPlatformEffects?: ProjectedTerminalPlatformEffects;
	readonly terminalFileDropCapability?: TerminalFileDropCapability;
	readonly terminalInputConfigurationSource?: TerminalInputConfigurationSource;
	readonly terminalInteractionConfigurationSource?: TerminalInteractionConfigurationSource;
	readonly terminalTrailingBlankRowsPreferenceSource?: TerminalTrailingBlankRowsPreferenceSource;
	readonly terminalCompletionReminderObserver?: TerminalCompletionReminderObserver;
	readonly terminalPresentationConfigurationSource?: TerminalPresentationConfigurationSource;
}

export function createWorkbenchClientRuntime({
	applicationLocaleSource,
	clientSession,
	documentElement,
	focusRecoveryStorage,
	preferredLocales,
	projectionClient,
	shortcutBindingSource,
	shortcutPlatform,
	terminalPlatformEffects = {},
	terminalFileDropCapability,
	terminalInputConfigurationSource = DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	terminalInteractionConfigurationSource = DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	terminalTrailingBlankRowsPreferenceSource,
	terminalCompletionReminderObserver,
	terminalPresentationConfigurationSource = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
}: CreateWorkbenchClientRuntimeOptions): WorkbenchClientRuntime {
	let disposed = false;
	const bootstrap = clientSession.bootstrap;
	const intl = createOrchbeeIntl(applicationLocaleSource.current());
	const initialDocument =
		bootstrap.documentState.kind === 'available'
			? bootstrap.documentState.document
			: bootstrap.initializationDocument;
	const initialClientView =
		focusRecoveryStorage === undefined
			? undefined
			: createFocusRecoveredWorkbenchClientView(
					initialDocument,
					readWorkbenchFocusRecovery(focusRecoveryStorage),
				);
	const actionRuntime = new ActionRuntime();
	const terminalPresentation: WorkbenchTerminalPresentationComposition = {
		createOperationId: () =>
			terminalPresentationOperationId(`workbench:operation:${createBrowserRandomIdentifier()}`),
		createPresentationId: () =>
			terminalPresentationId(`workbench:presentation:${createBrowserRandomIdentifier()}`),
		createRemovalId: () =>
			terminalPresentationRemovalId(`workbench:removal:${createBrowserRandomIdentifier()}`),
		dispatchCommands: () => undefined,
		initialPresentationId: bootstrap.initialPresentationId,
		kind: 'managed',
	};
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		initialClientView,
		initialDocument,
		standaloneCwd: bootstrap.defaultCwd,
		standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
		terminalPresentation,
	});
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
	const documentCoordinator = new WorkbenchDocumentCoordinator(
		projectWorkspaceRuntime,
		clientSession,
	);
	const terminalMetadataCursors = new Map<
		TerminalPresentationId,
		{
			readonly currentDirectory: string | null;
			readonly identityKey?: string;
			readonly revision: bigint;
			readonly title: string | null;
		}
	>();
	const presentationSnapshots =
		new TerminalPresentationSnapshotCache<ProjectedTerminalPresentationSnapshot>();
	const acceptTerminalNaturalMetadata = (
		presentationId: TerminalPresentationId,
		revision: bigint,
		metadata: { readonly currentDirectory: string | null; readonly title: string | null },
		identityKey?: string,
	): boolean => {
		if (disposed) return false;
		const previous = terminalMetadataCursors.get(presentationId);
		const knownDifferentIdentity =
			identityKey !== undefined &&
			previous?.identityKey !== undefined &&
			previous.identityKey !== identityKey;
		if (previous !== undefined && !knownDifferentIdentity) {
			if (previous.revision > revision) return false;
			if (previous.revision === revision) {
				// Title and cwd are immutable at a Host revision. An exact surface
				// observation may still carry a newer client-owned close activity.
				if (
					previous.currentDirectory !== metadata.currentDirectory ||
					previous.title !== metadata.title
				) {
					return false;
				}
				if (identityKey !== undefined) {
					if (previous.identityKey === undefined) {
						terminalMetadataCursors.set(presentationId, {
							currentDirectory: previous.currentDirectory,
							identityKey,
							revision: previous.revision,
							title: previous.title,
						});
					}
					return false;
				}
				return true;
			}
		}
		const retainedIdentityKey = identityKey ?? previous?.identityKey;
		terminalMetadataCursors.set(presentationId, {
			currentDirectory: metadata.currentDirectory,
			...(retainedIdentityKey === undefined ? {} : { identityKey: retainedIdentityKey }),
			revision,
			title: metadata.title,
		});
		return true;
	};
	const applyPublishedTerminalMetadata = (
		metadata: (typeof bootstrap.terminalMetadata)[number],
	): void => {
		const revision = BigInt(metadata.revision);
		if (
			!acceptTerminalNaturalMetadata(
				metadata.presentationId,
				revision,
				metadata,
				terminalIdentityKey(metadata.identity),
			)
		) {
			return;
		}
		projectWorkspaceRuntime.applyTerminalNaturalMetadataObservation(
			metadata.presentationId,
			resolveTerminalNaturalMetadataObservation(metadata, bootstrap.terminalHostName),
		);
	};
	for (const metadata of bootstrap.terminalMetadata) applyPublishedTerminalMetadata(metadata);
	const stopTerminalMetadataPublicationObservation = clientSession.subscribe((publication) => {
		if (publication.type !== 'terminal-metadata-published') return;
		applyPublishedTerminalMetadata(publication.metadata);
	});
	const terminalSurfaceCapability: ProjectedTerminalSurfaceCapability = {
		applicationLocaleSource,
		documentElement,
		preferredLocales,
		platformEffects: terminalPlatformEffects,
		...(terminalFileDropCapability === undefined ? {} : { terminalFileDropCapability }),
		...(terminalCompletionReminderObserver === undefined
			? {}
			: { terminalCompletionReminderObserver }),
		inputConfigurationSource: terminalInputConfigurationSource,
		interactionConfigurationSource: terminalInteractionConfigurationSource,
		...(terminalTrailingBlankRowsPreferenceSource === undefined
			? {}
			: { trailingBlankRowsPreferenceSource: terminalTrailingBlankRowsPreferenceSource }),
		presentationConfigurationSource: terminalPresentationConfigurationSource,
		presentationSnapshots,
		projectionClient,
		...(shortcutBindingSource === undefined ? {} : { shortcutBindingSource }),
		shortcutPlatform,
		observeTerminalMetadata: (presentationId, metadata) => {
			if (!acceptTerminalNaturalMetadata(presentationId, metadata.revision, metadata)) return;
			projectWorkspaceRuntime.applyTerminalMetadataObservation(
				presentationId,
				resolveTerminalMetadataObservation(metadata, bootstrap.terminalHostName),
			);
		},
		startStoppedTerminalSession: (presentationId) => {
			const result = projectWorkspaceRuntime.startStoppedTerminalSession(presentationId);
			if (result.kind !== 'rejected') terminalMetadataCursors.delete(presentationId);
			return result;
		},
	};
	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		terminalMetadataCursors.clear();
		presentationSnapshots.clear();
		runWorkbenchClientCleanup(
			[
				() => stopFocusRecoveryDocumentObservation?.(),
				() => stopFocusRecoveryViewObservation?.(),
				() => stopTerminalMetadataPublicationObservation(),
				() => actionRuntime.dispose(),
				() => documentCoordinator.dispose(),
				() => clientSession.dispose(),
				() => projectionClient.dispose(),
			],
			'Workbench runtime cleanup failed',
		);
	};
	const ready = documentCoordinator.start().then((result) => {
		if (result.kind === 'unavailable') return result;
		return disposed
			? ({ kind: 'unavailable', reason: 'control-unavailable' } as const)
			: ({ kind: 'ready' } as const);
	});

	return {
		actionRuntime,
		applicationLocaleSource,
		dispose,
		documentCoordinator,
		projectWorkspaceRuntime,
		ready,
		terminalSurfaceCapability,
	};
}

function terminalIdentityKey(identity: {
	readonly creationId: string;
	readonly hostEpoch: string;
	readonly terminalId: string;
}): string {
	return `${identity.hostEpoch}:${identity.terminalId}:${identity.creationId}`;
}
