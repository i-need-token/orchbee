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
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchTerminalPresentationComposition,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';

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
		initialDocument:
			bootstrap.documentState.kind === 'available'
				? bootstrap.documentState.document
				: bootstrap.initializationDocument,
		standaloneCwd: bootstrap.defaultCwd,
		standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle, { number: 1 }),
		terminalPresentation,
	});
	const documentCoordinator = new WorkbenchDocumentCoordinator(
		projectWorkspaceRuntime,
		clientSession,
	);
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
			projectWorkspaceRuntime.applyTerminalMetadataObservation(
				presentationId,
				resolveTerminalMetadataObservation(metadata, bootstrap.terminalHostName),
			);
		},
		startStoppedTerminalSession: (presentationId) =>
			projectWorkspaceRuntime.startStoppedTerminalSession(presentationId),
	};
	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
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
