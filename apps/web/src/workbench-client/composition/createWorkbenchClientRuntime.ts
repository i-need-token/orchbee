import { ActionRuntime } from '@orchbee/actions';
import {
	createOrchbeeIntl,
	getOrchbeeTextDirection,
	resolveOrchbeeLocale,
	type OrchbeeLocale,
	workbenchTabMessages,
} from '@orchbee/i18n';
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
import type { WorkbenchClientSessionClient } from '../client-session';
import type { ProjectedTerminalSurfaceCapability } from '../projected-terminal-surface';
import type { ProjectedTerminalShortcutBindingSource } from '../projected-terminal-actions';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../projected-terminal-renderer';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import type { TerminalProjectionClient } from '../terminal-projection';
import { WorkbenchDocumentCoordinator } from '../workbench-document-control';
import type { TerminalFileDropCapability } from '../../terminal-file-drop';
import type { TerminalCompletionReminderObserver } from '../../terminal-feedback-settings';
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
	readonly documentCoordinator: WorkbenchDocumentCoordinator;
	readonly locale: OrchbeeLocale;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly ready: Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	>;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
	dispose(): void;
}

export interface CreateWorkbenchClientRuntimeOptions {
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
	readonly terminalCompletionReminderObserver?: TerminalCompletionReminderObserver;
	readonly terminalPresentationConfigurationSource?: TerminalPresentationConfigurationSource;
}

export function createWorkbenchClientRuntime({
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
	terminalCompletionReminderObserver,
	terminalPresentationConfigurationSource = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
}: CreateWorkbenchClientRuntimeOptions): WorkbenchClientRuntime {
	const bootstrap = clientSession.bootstrap;
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
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
	const latestTerminalMetadataRevisions = new Map<
		TerminalPresentationId,
		{ readonly identityKey?: string; readonly revision: bigint }
	>();
	const acceptsTerminalMetadataRevision = (
		presentationId: TerminalPresentationId,
		revision: bigint,
		identityKey?: string,
	): boolean => {
		const previous = latestTerminalMetadataRevisions.get(presentationId);
		if (
			previous !== undefined &&
			(identityKey === undefined || previous.identityKey === identityKey) &&
			previous.revision > revision
		) {
			return false;
		}
		const retainedIdentityKey = identityKey ?? previous?.identityKey;
		latestTerminalMetadataRevisions.set(presentationId, {
			...(retainedIdentityKey === undefined ? {} : { identityKey: retainedIdentityKey }),
			revision,
		});
		return true;
	};
	const applyPublishedTerminalMetadata = (
		metadata: (typeof bootstrap.terminalMetadata)[number],
	): void => {
		const revision = BigInt(metadata.revision);
		if (
			!acceptsTerminalMetadataRevision(
				metadata.presentationId,
				revision,
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
	let disposed = false;
	const terminalSurfaceCapability: ProjectedTerminalSurfaceCapability = {
		documentElement,
		preferredLocales,
		platformEffects: terminalPlatformEffects,
		...(terminalFileDropCapability === undefined ? {} : { terminalFileDropCapability }),
		...(terminalCompletionReminderObserver === undefined
			? {}
			: { terminalCompletionReminderObserver }),
		inputConfigurationSource: terminalInputConfigurationSource,
		presentationConfigurationSource: terminalPresentationConfigurationSource,
		projectionClient,
		...(shortcutBindingSource === undefined ? {} : { shortcutBindingSource }),
		shortcutPlatform,
		observeTerminalMetadata: (presentationId, metadata) => {
			if (!acceptsTerminalMetadataRevision(presentationId, metadata.revision)) return;
			projectWorkspaceRuntime.applyTerminalMetadataObservation(
				presentationId,
				resolveTerminalMetadataObservation(metadata, bootstrap.terminalHostName),
			);
		},
		startStoppedTerminalSession: (presentationId) => {
			const result = projectWorkspaceRuntime.startStoppedTerminalSession(presentationId);
			if (result.kind !== 'rejected') latestTerminalMetadataRevisions.delete(presentationId);
			return result;
		},
	};
	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		stopFocusRecoveryDocumentObservation?.();
		stopFocusRecoveryViewObservation?.();
		stopTerminalMetadataPublicationObservation();
		latestTerminalMetadataRevisions.clear();
		actionRuntime.dispose();
		documentCoordinator.dispose();
		clientSession.dispose();
		projectionClient.dispose();
	};
	const ready = documentCoordinator.start().then((result) => {
		if (result.kind === 'unavailable') return result;
		return disposed
			? ({ kind: 'unavailable', reason: 'control-unavailable' } as const)
			: ({ kind: 'ready' } as const);
	});

	return {
		actionRuntime,
		dispose,
		documentCoordinator,
		locale,
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
