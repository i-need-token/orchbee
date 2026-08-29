import type {
	TerminalEnvironmentOverlay,
	TerminalExited,
	TerminalIdentity,
	TerminalLaunch,
} from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

import type { HostConnection } from '../host';
import { TerminalRecoveryCoordinator, type TerminalBindingStore } from '../recovery';
import type { DesktopTerminalBindingRegistry } from '../terminal-binding-registry';
import { TerminalBindingLifecycle, TerminalCloseOrchestrator } from '../terminal-lifetime';
import { DesktopTerminalLifecycleAdapter } from './DesktopTerminalLifecycleAdapter';

export interface DesktopTerminalLifecycleAdapterComposition {
	readonly allocateFreshBindingId: () => string;
	readonly beforeDispose?: (
		presentationId: TerminalPresentationId,
		identity: TerminalIdentity,
		minimumRevision: TerminalExited['finalRevision'],
	) => Promise<void>;
	readonly host: HostConnection;
	readonly onIdentityObserved?: (identity: TerminalIdentity) => void;
	readonly resolveEnvironmentOverlay?: (generation: {
		readonly creationId: TerminalIdentity['creationId'];
		readonly presentationId: TerminalPresentationId;
	}) => TerminalEnvironmentOverlay;
	readonly randomCreationId: () => string;
	readonly registry: DesktopTerminalBindingRegistry;
	readonly resolveTerminalLaunch: () => TerminalLaunch;
	readonly retireProjection: (presentationId: TerminalPresentationId) => Promise<void>;
	readonly retireEnvironmentOverlay?: (presentationId: TerminalPresentationId) => void;
	readonly store: TerminalBindingStore;
}

export function composeDesktopTerminalLifecycleAdapter(
	composition: DesktopTerminalLifecycleAdapterComposition,
): DesktopTerminalLifecycleAdapter {
	const beforeDispose = composition.beforeDispose;
	return new DesktopTerminalLifecycleAdapter({
		allocateFreshBindingId: composition.allocateFreshBindingId,
		createBindingLifecycle: (bindingId, launch, presentationId, onIdentityObserved) =>
			new TerminalBindingLifecycle(
				new TerminalRecoveryCoordinator(
					composition.host,
					composition.store,
					{ bindingId, launch, presentationId },
					{
						onIdentityObserved,
						randomCreationId: composition.randomCreationId,
						...(composition.resolveEnvironmentOverlay === undefined
							? {}
							: { resolveEnvironmentOverlay: composition.resolveEnvironmentOverlay }),
					},
				),
				new TerminalCloseOrchestrator(
					composition.host,
					composition.store,
					bindingId,
					beforeDispose === undefined
						? {}
						: {
								beforeDispose: (identity, minimumRevision) =>
									beforeDispose(presentationId, identity, minimumRevision),
							},
				),
			),
		registry: composition.registry,
		loadBindings: () => composition.store.load(),
		...(composition.onIdentityObserved === undefined
			? {}
			: { onIdentityObserved: composition.onIdentityObserved }),
		retireProjection: composition.retireProjection,
		...(composition.retireEnvironmentOverlay === undefined
			? {}
			: { onPresentationRetired: composition.retireEnvironmentOverlay }),
		resolveTerminalLaunch: composition.resolveTerminalLaunch,
	});
}
