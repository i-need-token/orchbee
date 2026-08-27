import {
	createTypedActionRuntime,
	type ActionExecution,
	type ActionInputFor,
	type ActionInvocationSource,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
	type PreparedActionInvocation,
} from '@orchbee/actions';

import type { TerminalFindController } from '../terminal-find-surface';
import { TerminalCommandOutputViewerController } from '../terminal-command-output-viewer';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import {
	PROJECTED_TERMINAL_ACTION_IDS,
	type ProjectedTerminalActionCatalog,
} from './projected-terminal-action-ids';
import { createProjectedTerminalClipboardActionCoordinator } from './createProjectedTerminalClipboardActionCoordinator';
import {
	registerProjectedTerminalActions,
	type ProjectedTerminalActionTarget,
} from './registerProjectedTerminalActions';

export interface ProjectedTerminalActionComposition {
	readonly findScope: ActionScope;
	readonly terminalScope: ActionScope;
	dispose(): void;
	execute<Id extends keyof ProjectedTerminalActionCatalog & string>(
		actionId: Id,
		input: ActionInputFor<ProjectedTerminalActionCatalog, Id>,
		scope: ActionScope,
		source: ActionInvocationSource,
	): ActionExecution;
	invokeShortcut<Id extends keyof ProjectedTerminalActionCatalog & string>(
		actionId: Id,
		input: ActionInputFor<ProjectedTerminalActionCatalog, Id>,
		scope: ActionScope,
		options?: { readonly nativePasteFallback: true },
	): boolean;
	executePrepared(invocation: PreparedActionInvocation): ActionExecution;
}

export interface ProjectedTerminalActionResultEvent {
	readonly actionId: string;
	readonly result: ActionResult;
}

interface ProjectedTerminalActionCompositionOptions {
	readonly outputController?: TerminalCommandOutputViewerController | undefined;
	readonly platformEffects?: ProjectedTerminalPlatformEffects | undefined;
	readonly reportResult?: ((event: ProjectedTerminalActionResultEvent) => void) | undefined;
	readonly runtime: ActionRuntime;
	readonly terminalScope: ActionScope;
}

/** Create the action scopes and registrations owned by one projected terminal mount. */
export function createProjectedTerminalActionComposition(
	terminal: ProjectedTerminalActionTarget,
	findController: TerminalFindController,
	options: ProjectedTerminalActionCompositionOptions,
): ProjectedTerminalActionComposition {
	const {
		outputController = new TerminalCommandOutputViewerController(),
		platformEffects = {},
		reportResult,
		runtime,
		terminalScope,
	} = options;
	const findScope = runtime.createScope('workbench.terminal.find', terminalScope);
	const typedRuntime = createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime);
	const activeExecutions = new Set<ActionExecution>();
	const clipboardActionCoordinator = createProjectedTerminalClipboardActionCoordinator();
	let disposed = false;
	const registrations = registerProjectedTerminalActions({
		clipboardActionCoordinator,
		findController,
		outputController,
		platformEffects,
		scope: terminalScope,
		terminal,
		typedRuntime,
	});

	return {
		dispose: () => {
			if (disposed) return;
			disposed = true;
			for (const execution of activeExecutions) execution.cancel();
			for (const registration of registrations) registration.dispose();
			outputController.dispose();
		},
		execute: (actionId, input, scope, source) => {
			const execution = typedRuntime.execute(actionId, input, { scope, source });
			observeResult(actionId, execution);
			return execution;
		},
		executePrepared: (invocation) => {
			const execution = runtime.executePrepared(invocation);
			observeResult(invocation.actionId, execution);
			return execution;
		},
		findScope,
		invokeShortcut: (actionId, input, scope, shortcutOptions) => {
			if (
				actionId === PROJECTED_TERMINAL_ACTION_IDS.paste &&
				!clipboardActionCoordinator.shouldInvokePasteShortcut(
					shortcutOptions?.nativePasteFallback === true,
				)
			) {
				return false;
			}
			const invocation = { scope, source: 'keyboard' as const };
			if (
				typedRuntime.resolveAction(actionId, input, invocation)?.availability.kind !== 'enabled'
			) {
				return false;
			}
			const execution = typedRuntime.execute(actionId, input, invocation);
			observeResult(actionId, execution);
			return true;
		},
		terminalScope,
	};

	function observeResult(actionId: string, execution: ActionExecution): void {
		activeExecutions.add(execution);
		void execution.completion.then((result) => {
			activeExecutions.delete(execution);
			if (!disposed) reportResult?.({ actionId, result });
		});
	}
}
