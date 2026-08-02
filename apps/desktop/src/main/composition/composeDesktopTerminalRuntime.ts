import type { TerminalExited } from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';

import type { ProjectedAttachmentBootstrap } from '../../shared';
import type { ProjectedTerminalBrokerMetrics } from '../broker';
import type { HostConnection } from '../host';
import { FileTerminalBindingStore } from '../recovery';
import { DesktopTerminalBindingRegistry } from '../terminal-binding-registry';
import {
	composeDesktopTerminalLifecycleAdapter,
	type DesktopRecoveredTerminalPresentation,
	type DesktopTerminalLifecycleAdapterInitialization,
	type DesktopTerminalProjectionTarget,
	type TerminalDocumentControlOperations,
} from '../terminal-document-control';
import {
	DesktopTerminalProjectionService,
	type DesktopProjectionBrokerHandle,
	type DesktopTerminalProjectionLease,
} from '../terminal-projection-control';
import type { DesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

export interface DesktopTerminalRuntime extends TerminalDocumentControlOperations {
	dispose(): Promise<void>;
	getProjectionTarget(
		presentationId: TerminalPresentationId,
	): DesktopTerminalProjectionTarget | null;
	getProjectionMetrics(): readonly ProjectedTerminalBrokerMetrics[];
	getRecoveredPresentations(): readonly DesktopRecoveredTerminalPresentation[];
	initialize(): Promise<DesktopTerminalLifecycleAdapterInitialization>;
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
		dispose: async () => projectionService.dispose(),
		execute: async (command) => adapter.execute(command),
		getProjectionMetrics: () => projectionService.getMetrics(),
		getProjectionTarget: (presentationId) => adapter.getProjectionTarget(presentationId),
		getRecoveredPresentations: () => adapter.getRecoveredPresentations(),
		initialize: async () => adapter.initialize(),
		open: async (presentationId) => projectionService.open(presentationId),
		recordExit: async (event) => adapter.recordExit(event),
	};
}
