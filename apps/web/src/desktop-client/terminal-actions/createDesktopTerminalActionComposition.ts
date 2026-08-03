import {
	ActionRuntime,
	createTypedActionRuntime,
	type ActionExecution,
	type ActionInputFor,
	type ActionScope,
} from '@orchbee/actions';

import type { DesktopTerminalFindController } from '../terminal-find-surface';
import type { DesktopTerminalActionCatalog } from './desktop-terminal-action-ids';
import {
	registerDesktopTerminalActions,
	type DesktopTerminalActionTarget,
} from './registerDesktopTerminalActions';

export interface DesktopTerminalActionComposition {
	readonly applicationScope: ActionScope;
	readonly findScope: ActionScope;
	readonly terminalScope: ActionScope;
	dispose(): void;
	execute<Id extends keyof DesktopTerminalActionCatalog & string>(
		actionId: Id,
		input: ActionInputFor<DesktopTerminalActionCatalog, Id>,
		scope: ActionScope,
		source: 'button' | 'keyboard' | 'pointer' | 'programmatic',
	): ActionExecution;
}

/** Create the action scopes and registrations owned by one Desktop terminal mount. */
export function createDesktopTerminalActionComposition(
	terminal: DesktopTerminalActionTarget,
	findController: DesktopTerminalFindController,
): DesktopTerminalActionComposition {
	const runtime = new ActionRuntime();
	const applicationScope = runtime.createScope('desktop.application');
	const terminalScope = runtime.createScope('desktop.terminal', applicationScope);
	const findScope = runtime.createScope('desktop.terminal.find', terminalScope);
	const typedRuntime = createTypedActionRuntime<DesktopTerminalActionCatalog>(runtime);
	const registrations = registerDesktopTerminalActions({
		findController,
		scope: terminalScope,
		terminal,
		typedRuntime,
	});

	return {
		applicationScope,
		dispose: () => {
			for (const registration of registrations) registration.dispose();
			runtime.dispose();
		},
		execute: (actionId, input, scope, source) =>
			typedRuntime.execute(actionId, input, { scope, source }),
		findScope,
		terminalScope,
	};
}
