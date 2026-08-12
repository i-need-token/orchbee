import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { projectedTerminalMessages } from '@orchbee/i18n';
import type { ProjectedTerminalSideEffectResult, TerminalOpenTarget } from '@orchbee/terminal';
import type { SelectionChanged } from '@orchbee/terminal-protocol';

import type { TerminalFindController } from '../terminal-find-surface';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import {
	PROJECTED_TERMINAL_ACTION_IDS,
	type ProjectedTerminalActionCatalog,
	projectedTerminalLinkActionInput,
} from './projected-terminal-action-ids';

export interface ProjectedTerminalActionTarget {
	readonly canControlTerminal: boolean;
	readonly selection: SelectionChanged;
	copySelectionAsync(): Promise<ProjectedTerminalSideEffectResult>;
	findNext(): boolean;
	findPrevious(): boolean;
	openLinkAsync(target: string): Promise<ProjectedTerminalSideEffectResult>;
	reset(): void;
	selectAll(): void;
}

export interface RegisterProjectedTerminalActionsOptions {
	readonly findController: TerminalFindController;
	readonly platformEffects?: ProjectedTerminalPlatformEffects;
	readonly scope: ActionScope;
	readonly terminal: ProjectedTerminalActionTarget;
	readonly typedRuntime: TypedActionRuntime<ProjectedTerminalActionCatalog>;
}

export function registerProjectedTerminalActions({
	findController,
	platformEffects = {},
	scope,
	terminal,
	typedRuntime,
}: RegisterProjectedTerminalActionsOptions): readonly ActionDisposable[] {
	const category = {
		id: 'terminal',
		title: projectedTerminalMessages.actionCategory,
	};
	const searchExecution = {
		concurrency: 'replace-latest' as const,
		concurrencyKey: () => 'terminal-search-navigation',
		mode: 'foreground' as const,
	};
	const searchAvailable = () =>
		findController.getSnapshot().isOpen && findController.getSnapshot().query.length > 0
			? ({ kind: 'enabled' } as const)
			: ({
					kind: 'disabled',
					reason: projectedTerminalMessages.actionUnavailable,
				} as const);

	return [
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'terminal-find-surface',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context) => {
						findController.open();
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.findOpen,
				input: noActionInput,
				title: projectedTerminalMessages.findOpenAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'terminal-reset',
					mode: 'foreground',
				},
				getAvailability: () =>
					terminal.canControlTerminal
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: projectedTerminalMessages.actionUnavailable },
				handler: {
					execute: (context) => {
						terminal.reset();
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.reset,
				input: noActionInput,
				title: projectedTerminalMessages.resetAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: searchExecution,
				getAvailability: searchAvailable,
				handler: {
					execute: (context) => navigateSearch(context, terminal.findNext()),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.findNext,
				input: noActionInput,
				title: projectedTerminalMessages.findNextAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: searchExecution,
				getAvailability: searchAvailable,
				handler: {
					execute: (context) => navigateSearch(context, terminal.findPrevious()),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.findPrevious,
				input: noActionInput,
				title: projectedTerminalMessages.findPreviousAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'terminal-selection-copy',
					mode: 'foreground',
				},
				getAvailability: () =>
					terminal.selection.state === 'active'
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: projectedTerminalMessages.actionUnavailable },
				handler: {
					execute: async (context) => {
						if (context.signal.aborted) return { kind: 'cancelled' };
						const result = await terminal.copySelectionAsync();
						if (result.kind === 'completed') {
							context.markCommitted();
							findController.setNotice({
								byteLength: result.byteLength ?? 0,
								kind: 'copy-completed',
							});
							return { kind: 'completed' };
						}
						if (result.kind === 'cancelled') return { kind: 'cancelled' };
						findController.setNotice({ kind: 'copy-failed' });
						return { failure: { code: result.code }, kind: 'failed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.copySelection,
				input: noActionInput,
				title: projectedTerminalMessages.copyAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'replace-latest',
					concurrencyKey: () => 'terminal-selection',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context) => {
						terminal.selectAll();
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.selectAll,
				input: noActionInput,
				title: projectedTerminalMessages.selectAllAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'terminal-link-open',
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					permittedExternalTarget(input.target)
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: projectedTerminalMessages.actionUnavailable },
				handler: {
					execute: async (context, input) => {
						if (input.target.kind !== 'url' || !permittedExternalTarget(input.target)) {
							findController.setNotice({ kind: 'link-blocked' });
							return { code: 'link-policy-blocked', kind: 'blocked' };
						}
						if (context.signal.aborted) return { kind: 'cancelled' };
						const result = await (platformEffects.openExternal?.(input.target.url) ??
							terminal.openLinkAsync(input.target.url));
						if (result.kind === 'completed') {
							context.markCommitted();
							return { kind: 'completed' };
						}
						if (result.kind === 'cancelled') return { kind: 'cancelled' };
						findController.setNotice(
							result.code === 'link-policy-blocked'
								? { kind: 'link-blocked' }
								: { kind: 'link-failed' },
						);
						return { failure: { code: result.code }, kind: 'failed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.openLink,
				input: projectedTerminalLinkActionInput,
				title: projectedTerminalMessages.openLinkAction,
			}),
		),
	];
}

function navigateSearch(context: ActionExecutionContext, moved: boolean) {
	if (!moved) return { kind: 'unchanged' as const };
	context.markCommitted();
	return { kind: 'completed' as const };
}

function permittedExternalTarget(target: TerminalOpenTarget): boolean {
	if (target.kind !== 'url') return false;
	try {
		const protocol = new URL(target.url).protocol;
		return protocol === 'http:' || protocol === 'https:' || protocol === 'mailto:';
	} catch {
		return false;
	}
}
