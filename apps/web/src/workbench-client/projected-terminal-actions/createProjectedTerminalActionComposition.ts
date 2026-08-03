import {
	ActionRuntime,
	createTypedActionRuntime,
	type ActionExecution,
	type ActionInputFor,
	type ActionScope,
} from '@orchbee/actions';

import type { TerminalFindController } from '../terminal-find-surface';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import type { ProjectedTerminalActionCatalog } from './projected-terminal-action-ids';
import {
	registerProjectedTerminalActions,
	type ProjectedTerminalActionTarget,
} from './registerProjectedTerminalActions';

export interface ProjectedTerminalActionComposition {
	readonly applicationScope: ActionScope;
	readonly findScope: ActionScope;
	readonly terminalScope: ActionScope;
	dispose(): void;
	execute<Id extends keyof ProjectedTerminalActionCatalog & string>(
		actionId: Id,
		input: ActionInputFor<ProjectedTerminalActionCatalog, Id>,
		scope: ActionScope,
		source: 'button' | 'keyboard' | 'pointer' | 'programmatic',
	): ActionExecution;
}

/** Create the action scopes and registrations owned by one projected terminal mount. */
export function createProjectedTerminalActionComposition(
	terminal: ProjectedTerminalActionTarget,
	findController: TerminalFindController,
	platformEffects: ProjectedTerminalPlatformEffects = {},
): ProjectedTerminalActionComposition {
	const runtime = new ActionRuntime();
	const applicationScope = runtime.createScope('workbench.application');
	const terminalScope = runtime.createScope('workbench.terminal', applicationScope);
	const findScope = runtime.createScope('workbench.terminal.find', terminalScope);
	const typedRuntime = createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime);
	const registrations = registerProjectedTerminalActions({
		findController,
		platformEffects,
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
