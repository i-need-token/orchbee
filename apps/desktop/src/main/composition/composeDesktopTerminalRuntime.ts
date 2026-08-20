import type {
	AttachmentCapabilities,
	TerminalExited,
	TerminalFailed,
	TerminalGeometry,
	TerminalIdentity,
	TerminalLaunch,
} from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';

import type { ProjectedAttachmentBootstrap } from '@orchbee/workbench-client-protocol';
import type {
	ProjectedTerminalBrokerEffectTarget,
	ProjectedTerminalBrokerLifecycle,
	ProjectedTerminalBrokerMetrics,
} from '../broker';
import { readTerminalInventory, type HostConnection } from '../host';
import { FileTerminalBindingStore } from '../recovery';
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
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

export interface DesktopTerminalRuntime extends DesktopTerminalLifecycleOperations {
	closeBindingsNotReferencedBy(
		referencedPresentationIds: ReadonlySet<TerminalPresentationId>,
	): Promise<DesktopTerminalBindingDrainResult>;
	dispose(): Promise<void>;
	getOwnedBindingCount(): number;
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
	readonly resolveTerminalLaunch: () => TerminalLaunch;
}

export function composeDesktopTerminalRuntime(
	composition: DesktopTerminalRuntimeComposition,
): DesktopTerminalRuntime {
	const store = new FileTerminalBindingStore(composition.configuration.terminalBindingStorePath);
	const registry = new DesktopTerminalBindingRegistry();
	let identityObservedListener: ((identity: TerminalIdentity) => void) | undefined;
	let projections: DesktopTerminalProjectionService | undefined;
	const adapter = composeDesktopTerminalLifecycleAdapter({
		allocateFreshBindingId: () => `binding-${composition.randomIdentifier()}`,
		host: composition.host,
		randomCreationId: composition.randomIdentifier,
		registry,
		resolveTerminalLaunch: composition.resolveTerminalLaunch,
		retireProjection: async (presentationId) => projections?.retirePresentation(presentationId),
		store,
		onIdentityObserved: (identity) => identityObservedListener?.(identity),
	});
	projections = new DesktopTerminalProjectionService({
		createBroker: composition.createBroker,
		createMessageChannel: composition.createMessageChannel,
		getProjectionTarget: (presentationId) => adapter.getProjectionTarget(presentationId),
		host: composition.host,
		markTerminalStarted: (presentationId, identity) =>
			adapter.markTerminalStarted(presentationId, identity),
		observeTerminalTheme: (presentationId, theme) =>
			adapter.observeTerminalTheme(presentationId, theme),
		randomAttachmentId: composition.randomIdentifier,
	});
	const projectionService = projections;
	return {
		closeBindingsNotReferencedBy: async (referencedPresentationIds) =>
			adapter.closeBindingsNotReferencedBy(referencedPresentationIds),
		dispose: async () => {
			try {
				await projectionService.dispose();
			} finally {
				adapter.dispose();
			}
		},
		execute: async (command) => adapter.execute(command),
		getProjectionMetrics: () => projectionService.getMetrics(),
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
		open: async (presentationId, geometry, options) =>
			projectionService.open(presentationId, geometry, options),
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
