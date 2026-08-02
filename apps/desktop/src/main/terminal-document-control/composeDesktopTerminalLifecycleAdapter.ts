import type { TerminalLaunchV1_1 } from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

import type { HostConnection } from '../host';
import { TerminalRecoveryCoordinator, type TerminalBindingStore } from '../recovery';
import type { DesktopTerminalBindingRegistry } from '../terminal-binding-registry';
import { TerminalBindingLifecycle, TerminalCloseOrchestrator } from '../terminal-lifetime';
import { DesktopTerminalLifecycleAdapter } from './DesktopTerminalLifecycleAdapter';

export interface DesktopTerminalLifecycleAdapterComposition {
	readonly allocateFreshBindingId: () => string;
	readonly host: HostConnection;
	readonly randomCreationId: () => string;
	readonly registry: DesktopTerminalBindingRegistry;
	readonly retireProjection: (presentationId: TerminalPresentationId) => Promise<void>;
	readonly store: TerminalBindingStore;
	readonly terminalLaunch: TerminalLaunchV1_1;
}

export function composeDesktopTerminalLifecycleAdapter(
	composition: DesktopTerminalLifecycleAdapterComposition,
): DesktopTerminalLifecycleAdapter {
	return new DesktopTerminalLifecycleAdapter({
		allocateFreshBindingId: composition.allocateFreshBindingId,
		createBindingLifecycle: (bindingId, launch) =>
			new TerminalBindingLifecycle(
				new TerminalRecoveryCoordinator(
					composition.host,
					composition.store,
					{ bindingId, launch },
					{ randomCreationId: composition.randomCreationId },
				),
				new TerminalCloseOrchestrator(composition.host, composition.store, bindingId),
			),
		registry: composition.registry,
		retireProjection: composition.retireProjection,
		terminalLaunch: composition.terminalLaunch,
	});
}
