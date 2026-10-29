import type {
	AttachmentCapabilities,
	CreationId,
	TerminalEnvironmentOverlay,
	TerminalExited,
	TerminalFailed,
	TerminalGeometry,
	TerminalIdentity,
	TerminalLaunch,
} from '@orchbee/terminal-protocol';
import type { MessagePortMain } from 'electron';

import type {
	ProjectedAttachmentBootstrap,
	TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';
import type {
	ProjectedTerminalBrokerEffectTarget,
	ProjectedTerminalBrokerLifecycle,
	ProjectedTerminalBrokerMetrics,
} from '../broker';
import { readTerminalInventory, type HostConnection } from '../host';
import { FileTerminalBindingStore } from '../recovery';
import {
	ArchivedTerminalProjectionService,
	FileTerminalArchiveStore,
	TerminalArchiveCaptureService,
} from '../terminal-archive';
import { DesktopTerminalBindingRegistry } from '../terminal-binding-registry';
import {
	composeDesktopTerminalLifecycleAdapter,
	type DesktopTerminalBindingDrainResult,
	type DesktopRecoveredTerminalPresentation,
	type DesktopTerminalExitRecordResult,
	type DesktopTerminalLifecycleAdapterInitialization,
	type DesktopTerminalLifecycleOperations,
	type DesktopTerminalProjectionTarget,
} from '../terminal-document-control';
import {
	DesktopTerminalProjectionService,
	type DesktopProjectionBrokerHandle,
	type DesktopTerminalProjectionLease,
	type DesktopTerminalProjectionOpenOptions,
} from '../terminal-projection-control';
import type { DesktopRuntimeConfiguration } from './resolve-desktop-runtime-configuration';

export interface DesktopTerminalRuntime extends DesktopTerminalLifecycleOperations {
	closeBindingsNotReferencedBy(
		referencedPresentationIds: ReadonlySet<TerminalPresentationId>,
	): Promise<DesktopTerminalBindingDrainResult>;
	dispose(): Promise<void>;
	getOwnedBindingCount(): number;
	findPresentationByIdentity(identity: TerminalIdentity): TerminalPresentationId | null;
	getProjectionTarget(
		presentationId: TerminalPresentationId,
	): DesktopTerminalProjectionTarget | null;
	getDurableTerminalExit(presentationId: TerminalPresentationId): TerminalExited | null;
	getTerminalFailure(presentationId: TerminalPresentationId): TerminalFailed | null;
	getProjectionMetrics(): readonly ProjectedTerminalBrokerMetrics[];
	getRecoveredPresentations(): readonly DesktopRecoveredTerminalPresentation[];
	initialize(): Promise<DesktopTerminalLifecycleAdapterInitialization>;
	inspectUnknownHostTerminalCount(): Promise<number>;
	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		options?: DesktopTerminalProjectionOpenOptions,
	): Promise<DesktopTerminalProjectionLease>;
	openReadOnly(presentationId: TerminalPresentationId): Promise<DesktopTerminalProjectionLease>;
	reconcileArchives(retainedPresentationIds: ReadonlySet<TerminalPresentationId>): Promise<void>;
	recordExit(event: TerminalExited): Promise<TerminalPresentationId | null>;
	recordExitOutcome(event: TerminalExited): Promise<DesktopTerminalExitRecordResult>;
	commitExitPublication?(event: TerminalExited): boolean;
	releasePendingExit?(event: TerminalExited): void;
	setIdentityObservedListener?(listener: (identity: TerminalIdentity) => void): void;
}

export interface DesktopTerminalRuntimeComposition {
	readonly configuration: DesktopRuntimeConfiguration;
	readonly createBroker: (
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
		effectTarget: ProjectedTerminalBrokerEffectTarget,
		lifecycle?: ProjectedTerminalBrokerLifecycle,
		initialCapabilities?: AttachmentCapabilities,
	) => DesktopProjectionBrokerHandle;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly host: HostConnection;
	readonly randomIdentifier: () => string;
	readonly resolveEnvironmentOverlay?: (
		presentationId: TerminalPresentationId,
		creationId: CreationId,
	) => TerminalEnvironmentOverlay;
	readonly resolveTerminalLaunch: () => TerminalLaunch | Promise<TerminalLaunch>;
	readonly retireEnvironmentOverlay?: (presentationId: TerminalPresentationId) => void;
	readonly writeClipboardText?: (text: string) => Promise<void>;
}

export function composeDesktopTerminalRuntime(
	composition: DesktopTerminalRuntimeComposition,
): DesktopTerminalRuntime {
	const store = new FileTerminalBindingStore(composition.configuration.terminalBindingStorePath);
	const archiveStore = new FileTerminalArchiveStore(
		composition.configuration.terminalArchiveDirectory,
	);
	const registry = new DesktopTerminalBindingRegistry();
	let identityObservedListener: ((identity: TerminalIdentity) => void) | undefined;
	let projections: DesktopTerminalProjectionService | undefined;
	let archiveCapture: TerminalArchiveCaptureService | undefined;
	const adapter = composeDesktopTerminalLifecycleAdapter({
		allocateFreshBindingId: () => `binding-${composition.randomIdentifier()}`,
		beforeDispose: async (presentationId, identity, minimumRevision) => {
			if (archiveCapture !== undefined) {
				await archiveCapture.capture(presentationId, identity, minimumRevision);
			}
		},
		host: composition.host,
		randomCreationId: composition.randomIdentifier,
		registry,
		resolveTerminalLaunch: composition.resolveTerminalLaunch,
		...(composition.resolveEnvironmentOverlay === undefined
			? {}
			: {
					resolveEnvironmentOverlay: ({ creationId, presentationId }) =>
						composition.resolveEnvironmentOverlay?.(presentationId, creationId) ?? [],
				}),
		retireProjection: async (presentationId) => projections?.retirePresentation(presentationId),
		...(composition.retireEnvironmentOverlay === undefined
			? {}
			: { retireEnvironmentOverlay: composition.retireEnvironmentOverlay }),
		store,
		onIdentityObserved: (identity) => identityObservedListener?.(identity),
	});
	projections = new DesktopTerminalProjectionService({
		createBroker: composition.createBroker,
		createMessageChannel: composition.createMessageChannel,
		getProjectionTarget: (presentationId) => adapter.getProjectionTarget(presentationId),
		host: composition.host,
		hostDefaultColors: {
			colorScheme: composition.configuration.terminalLaunch.terminalProfile.colorScheme,
			defaultBackgroundRgb:
				composition.configuration.terminalLaunch.terminalProfile.defaultBackgroundRgb,
			defaultForegroundRgb:
				composition.configuration.terminalLaunch.terminalProfile.defaultForegroundRgb,
		},
		markTerminalStarted: (presentationId, identity) =>
			adapter.markTerminalStarted(presentationId, identity),
		observeTerminalTheme: (presentationId, theme) =>
			adapter.observeTerminalTheme(presentationId, theme),
		randomAttachmentId: composition.randomIdentifier,
	});
	const projectionService = projections;
	const archivedProjections = new ArchivedTerminalProjectionService({
		createMessageChannel: composition.createMessageChannel,
		randomAttachmentId: composition.randomIdentifier,
		store: archiveStore,
		writeClipboardText:
			composition.writeClipboardText ??
			(() => Promise.reject(new Error('Desktop clipboard integration is unavailable.'))),
	});
	archiveCapture = new TerminalArchiveCaptureService({
		openCaptureReadOnly: (presentationId, identity) =>
			projectionService.openCaptureReadOnly(presentationId, identity),
		store: archiveStore,
	});
	return {
		closeBindingsNotReferencedBy: async (referencedPresentationIds) =>
			adapter.closeBindingsNotReferencedBy(referencedPresentationIds),
		dispose: async () => {
			try {
				await archivedProjections.dispose();
			} finally {
				try {
					await projectionService.dispose();
				} finally {
					adapter.dispose();
				}
			}
		},
		execute: async (command) => {
			if (command.type === 'terminal-close-requested' && command.disposition === 'remove') {
				await archiveStore.remove(command.presentationId).catch(() => undefined);
			}
			return adapter.execute(command);
		},
		findPresentationByIdentity: (identity) => adapter.findPresentationByIdentity(identity),
		getProjectionMetrics: () => [
			...projectionService.getMetrics(),
			...archivedProjections.getMetrics(),
		],
		getOwnedBindingCount: () => adapter.bindingCount,
		getDurableTerminalExit: (presentationId) => adapter.getDurableTerminalExit(presentationId),
		getTerminalFailure: (presentationId) => adapter.getTerminalFailure(presentationId),
		getProjectionTarget: (presentationId) => adapter.getProjectionTarget(presentationId),
		getRecoveredPresentations: () => adapter.getRecoveredPresentations(),
		initialize: async () => adapter.initialize(),
		inspectUnknownHostTerminalCount: async () => {
			if (adapter.bindingCount !== 0) {
				throw new Error('Unknown Host terminals cannot be classified while owned bindings remain.');
			}
			return (await readTerminalInventory(composition.host)).length;
		},
		open: async (presentationId, geometry, options) => {
			try {
				return await projectionService.open(presentationId, geometry, options);
			} catch (liveError) {
				if (
					options?.allowArchivedFallback === false ||
					options?.brokerEffectTarget === 'client' ||
					adapter.getProjectionTarget(presentationId) !== null
				) {
					throw liveError;
				}
				try {
					const archived = await archivedProjections.open(presentationId);
					if (archived !== null) return archived;
				} catch {
					await archiveStore.remove(presentationId).catch(() => undefined);
				}
				throw liveError;
			}
		},
		openReadOnly: async (presentationId) => {
			const target = adapter.getProjectionTarget(presentationId);
			if (target !== null) {
				return projectionService.openCaptureReadOnly(presentationId, target.identity);
			}
			try {
				const archived = await archivedProjections.open(presentationId);
				if (archived !== null) return archived;
			} catch {
				await archiveStore.remove(presentationId).catch(() => undefined);
			}
			throw terminalProjectionFailure('terminal-projection-unavailable', true);
		},
		reconcileArchives: async (retainedPresentationIds) => {
			await archiveStore.reconcile(retainedPresentationIds).catch(() => undefined);
		},
		recordExit: async (event) => adapter.recordExit(event),
		recordExitOutcome: async (event) => adapter.recordExitOutcome(event),
		recordFailure: async (event) => adapter.recordFailure(event),
		commitExitPublication: (event) => adapter.commitExitPublication(event),
		releasePendingExit: (event) => adapter.releasePendingExit(event),
		setIdentityObservedListener: (listener) => {
			identityObservedListener = listener;
		},
	};
}

function terminalProjectionFailure(code: string, retryable: boolean): Error {
	return new Error(code, { cause: { code, retryable } });
}
