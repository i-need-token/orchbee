import type { ActionAvailability, ActionExecutionContext, ActionResult } from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import type { TabStripTabCreationIntent } from '@orchbee/tab-strip';

import { executeWorkbenchTerminalCreate } from '../semantic-commands';
import type { WorkbenchTerminalCreateInput } from './workbench-action-ids';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchRuntimeTerminalCreateResult,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchTerminalCreateRequest } from '../types/workbench-host-actions';
import type {
	WorkbenchRemoteTerminalCreationCapability,
	WorkbenchRemoteTerminalCreationFailureReason,
	WorkbenchRemoteTerminalCreationResult,
} from '../types/workbench-remote-terminal-creation';

interface WorkbenchTerminalCreationOptions {
	readonly fallbackTitle: string;
	readonly onLocalTerminalCreated?:
		| ((creation: { readonly tabId: string; readonly terminalContextId: string }) => void)
		| undefined;
	readonly onRemoteTerminalCreated?:
		| ((creation: { readonly tabId: string; readonly terminalContextId: string }) => void)
		| undefined;
	readonly remoteTerminalCreation?: WorkbenchRemoteTerminalCreationCapability | undefined;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

export function getWorkbenchTerminalCreationAvailability(
	runtime: WorkbenchProjectWorkspaceRuntime,
	remoteTerminalCreation: WorkbenchRemoteTerminalCreationCapability | undefined,
	input: WorkbenchTerminalCreateInput,
): ActionAvailability {
	const route = runtime.resolveActiveTerminalCreationRoute(activeTerminalCreationRouteInput(input));
	return route.kind === 'rejected' ||
		(route.kind === 'remote' && remoteTerminalCreation === undefined)
		? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
		: { kind: 'enabled' };
}

export function getWorkbenchTabCreationAvailability(
	runtime: WorkbenchProjectWorkspaceRuntime,
	remoteTerminalCreation: WorkbenchRemoteTerminalCreationCapability | undefined,
	intent: TabStripTabCreationIntent,
): ActionAvailability {
	if (intent.kind === 'new-tab') {
		return getWorkbenchTerminalCreationAvailability(runtime, remoteTerminalCreation, {
			groupId: intent.groupId,
			...(intent.index === undefined ? {} : { index: intent.index }),
		});
	}
	const route = runtime.resolveActiveTerminalCreationRoute({ placement: { groupId: null } });
	return route.kind === 'local'
		? { kind: 'enabled' }
		: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
}

export function executeWorkbenchTerminalCreationAction(
	context: ActionExecutionContext,
	input: WorkbenchTerminalCreateRequest,
	options: WorkbenchTerminalCreationOptions,
): ActionResult<void> | Promise<ActionResult<void>> {
	if (context.signal.aborted) return { kind: 'cancelled' };
	if (input.currentDirectoryTarget !== undefined) {
		return settleLocalTerminalCreation(
			options,
			options.runtime.createTerminal(
				{
					currentDirectoryTarget: input.currentDirectoryTarget,
					groupId: input.groupId,
					...(input.index === undefined ? {} : { index: input.index }),
					source: context.source,
					title: options.fallbackTitle,
				},
				context.markCommitted,
			),
		);
	}
	const route = options.runtime.resolveActiveTerminalCreationRoute(
		activeTerminalCreationRouteInput(input),
	);
	if (route.kind === 'rejected') return { code: route.reason, kind: 'blocked' };
	if (route.kind === 'remote') {
		if (options.remoteTerminalCreation === undefined) {
			return { code: 'remote-terminal-capability-unavailable', kind: 'blocked' };
		}
		return executeRemoteTerminalCreation(context, options, options.remoteTerminalCreation, {
			workspaceId: route.workspaceId,
		});
	}
	const creation = executeWorkbenchTerminalCreate(
		options.runtime,
		{
			...(input.directoryId === undefined ? {} : { directoryId: input.directoryId }),
			fallbackTitle: options.fallbackTitle,
			location: route.location,
			placement: {
				groupId: input.groupId,
				...(input.index === undefined ? {} : { index: input.index }),
			},
		},
		context.markCommitted,
	);
	return settleLocalTerminalCreation(options, creation);
}

function settleLocalTerminalCreation(
	options: WorkbenchTerminalCreationOptions,
	creation: WorkbenchRuntimeTerminalCreateResult,
): ActionResult<void> {
	if (creation.kind === 'rejected') return { code: creation.reason, kind: 'blocked' };
	const terminalContext = options.runtime.getTerminalContext(creation.terminalContextId);
	if (terminalContext === null) {
		throw new Error('A committed Terminal result lost its exact terminal context.');
	}
	terminalContext.tabModel.selectTab(creation.tabId);
	options.onLocalTerminalCreated?.(creation);
	return { kind: 'completed' };
}

async function executeRemoteTerminalCreation(
	context: ActionExecutionContext,
	options: WorkbenchTerminalCreationOptions,
	capability: WorkbenchRemoteTerminalCreationCapability,
	target: { readonly workspaceId: string },
): Promise<ActionResult<void>> {
	const result = await capability.create({ ...target, signal: context.signal });
	if (result.kind === 'cancelled') return { kind: 'cancelled' };
	// A non-cancellation Main settlement is authoritative even if Abort raced with it.
	// Crossing this barrier prevents ActionRuntime from rewriting that result as cancellation.
	context.markCommitted();
	if (result.kind === 'failed') {
		return remoteTerminalFailureIsPolicy(result.reason)
			? { code: result.reason, kind: 'blocked' }
			: { failure: { code: result.reason }, kind: 'failed' };
	}
	if (result.kind === 'indeterminate') {
		return { failure: { code: `remote-terminal-${result.reason}` }, kind: 'failed' };
	}
	const publication = await waitForRemoteTerminalPublication(
		options.runtime,
		result,
		context.signal,
	);
	if (publication === 'unavailable') {
		return {
			failure: { code: 'remote-terminal-publication-unavailable' },
			kind: 'failed',
		};
	}
	const terminalContext = options.runtime.getTerminalContext(result.terminalContextId);
	if (terminalContext === null) {
		throw new Error('A published remote Terminal lost its exact terminal context.');
	}
	terminalContext.tabModel.selectTab(result.tabId);
	options.onRemoteTerminalCreated?.(result);
	return { kind: result.kind === 'created' ? 'completed' : 'unchanged' };
}

function activeTerminalCreationRouteInput(input: WorkbenchTerminalCreateInput) {
	return {
		...(input.directoryId === undefined ? {} : { directoryId: input.directoryId }),
		placement: {
			groupId: input.groupId,
			...(input.index === undefined ? {} : { index: input.index }),
		},
	};
}

function remoteTerminalFailureIsPolicy(
	reason: WorkbenchRemoteTerminalCreationFailureReason,
): boolean {
	switch (reason) {
		case 'busy':
		case 'environment-not-connected':
		case 'environment-not-found':
		case 'project-not-found':
		case 'remote-context-invalid':
		case 'workbench-missing':
		case 'workspace-environment-mismatch':
		case 'workspace-not-found':
			return true;
		case 'control-unavailable':
		case 'registry-unavailable':
		case 'terminal-host-busy':
		case 'terminal-host-compatibility-unsupported':
		case 'terminal-host-unavailable':
		case 'workbench-unavailable':
			return false;
	}
}

function waitForRemoteTerminalPublication(
	runtime: WorkbenchProjectWorkspaceRuntime,
	result: Extract<WorkbenchRemoteTerminalCreationResult, { readonly kind: 'created' | 'existing' }>,
	signal: AbortSignal,
): Promise<'published' | 'unavailable'> {
	if (runtime.hasPublishedTerminal(result)) return Promise.resolve('published');
	if (signal.aborted) return Promise.resolve('unavailable');
	return new Promise((resolve) => {
		let settled = false;
		let unsubscribe = (): void => undefined;
		const finish = (outcome: 'published' | 'unavailable'): void => {
			if (settled) return;
			settled = true;
			signal.removeEventListener('abort', onAbort);
			unsubscribe();
			resolve(outcome);
		};
		const onAbort = (): void => finish('unavailable');
		const observe = (): void => {
			if (runtime.hasPublishedTerminal(result)) finish('published');
		};
		unsubscribe = runtime.subscribe(observe);
		signal.addEventListener('abort', onAbort, { once: true });
		if (signal.aborted) finish('unavailable');
		else observe();
	});
}
