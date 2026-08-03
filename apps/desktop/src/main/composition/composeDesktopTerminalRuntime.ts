import type { TerminalExited } from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';

import type { ProjectedAttachmentBootstrap } from '@orchbee/workbench-client-protocol';
import type { ProjectedTerminalBrokerMetrics } from '../broker';
import { readTerminalInventory, type HostConnection } from '../host';
import { FileTerminalBindingStore } from '../recovery';
import { DesktopTerminalBindingRegistry } from '../terminal-binding-registry';
import {
	composeDesktopTerminalLifecycleAdapter,
	type DesktopTerminalBindingDrainResult,
	type DesktopRecoveredTerminalPresentation,
	type DesktopTerminalLifecycleAdapterInitialization,
	type DesktopTerminalLifecycleOperations,
	type DesktopTerminalProjectionTarget,
} from '../terminal-document-control';
import {
	DesktopTerminalProjectionService,
	type DesktopProjectionBrokerHandle,
	type DesktopTerminalProjectionLease,
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
	getProjectionMetrics(): readonly ProjectedTerminalBrokerMetrics[];
	getRecoveredPresentations(): readonly DesktopRecoveredTerminalPresentation[];
	initialize(): Promise<DesktopTerminalLifecycleAdapterInitialization>;
	inspectUnknownHostTerminalCount(): Promise<number>;
	open(presentationId: TerminalPresentationId): Promise<DesktopTerminalProjectionLease>;
	recordExit(event: TerminalExited): Promise<TerminalPresentationId | null>;
}

export interface DesktopTerminalRuntimeComposition {
	readonly configuration: DesktopRuntimeConfiguration;
	readonly createBroker: (
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
	) => DesktopProjectionBrokerHandle;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly host: HostConnection;
	readonly randomIdentifier: () => string;
}

export function composeDesktopTerminalRuntime(
	composition: DesktopTerminalRuntimeComposition,
): DesktopTerminalRuntime {
	const store = new FileTerminalBindingStore(composition.configuration.terminalBindingStorePath);
	const registry = new DesktopTerminalBindingRegistry();
	let projections: DesktopTerminalProjectionService | undefined;
	const adapter = composeDesktopTerminalLifecycleAdapter({
		allocateFreshBindingId: () => `binding-${composition.randomIdentifier()}`,
		host: composition.host,
		randomCreationId: composition.randomIdentifier,
		registry,
		retireProjection: async (presentationId) => projections?.retirePresentation(presentationId),
		store,
		terminalLaunch: composition.configuration.terminalLaunch,
	});
	projections = new DesktopTerminalProjectionService({
		createBroker: composition.createBroker,
		createMessageChannel: composition.createMessageChannel,
		getProjectionTarget: (presentationId) => adapter.getProjectionTarget(presentationId),
		host: composition.host,
		randomAttachmentId: composition.randomIdentifier,
	});
	const projectionService = projections;
	return {
		closeBindingsNotReferencedBy: async (referencedPresentationIds) =>
			adapter.closeBindingsNotReferencedBy(referencedPresentationIds),
		dispose: async () => projectionService.dispose(),
		execute: async (command) => adapter.execute(command),
		getProjectionMetrics: () => projectionService.getMetrics(),
		getOwnedBindingCount: () => adapter.bindingCount,
		getProjectionTarget: (presentationId) => adapter.getProjectionTarget(presentationId),
		getRecoveredPresentations: () => adapter.getRecoveredPresentations(),
		initialize: async () => adapter.initialize(),
		inspectUnknownHostTerminalCount: async () => {
			if (adapter.bindingCount !== 0) {
				throw new Error('Unknown Host terminals cannot be classified while owned bindings remain.');
			}
			return (await readTerminalInventory(composition.host)).length;
		},
		open: async (presentationId) => projectionService.open(presentationId),
		recordExit: async (event) => adapter.recordExit(event),
	};
}
