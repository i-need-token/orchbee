import type { TerminalExited, TerminalIdentity, TerminalLaunch } from '@orchbee/terminal-protocol';
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
	readonly randomCreationId: () => string;
	readonly registry: DesktopTerminalBindingRegistry;
	readonly resolveTerminalLaunch: () => TerminalLaunch;
	readonly retireProjection: (presentationId: TerminalPresentationId) => Promise<void>;
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
		resolveTerminalLaunch: composition.resolveTerminalLaunch,
	});
}
