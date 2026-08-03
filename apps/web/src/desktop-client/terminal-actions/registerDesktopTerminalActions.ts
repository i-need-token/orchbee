import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { desktopTerminalMessages } from '@orchbee/i18n';
import type { ProjectedTerminalSideEffectResult, TerminalOpenTarget } from '@orchbee/terminal';
import type { SelectionChanged } from '@orchbee/terminal-protocol';

import type { DesktopTerminalFindController } from '../terminal-find-surface';
import {
	DESKTOP_TERMINAL_ACTION_IDS,
	type DesktopTerminalActionCatalog,
	desktopTerminalLinkActionInput,
} from './desktop-terminal-action-ids';

export interface DesktopTerminalActionTarget {
	readonly selection: SelectionChanged;
	copySelectionAsync(): Promise<ProjectedTerminalSideEffectResult>;
	findNext(): boolean;
	findPrevious(): boolean;
	openLinkAsync(target: string): Promise<ProjectedTerminalSideEffectResult>;
	selectAll(): void;
}

export interface RegisterDesktopTerminalActionsOptions {
	readonly findController: DesktopTerminalFindController;
	readonly scope: ActionScope;
	readonly terminal: DesktopTerminalActionTarget;
	readonly typedRuntime: TypedActionRuntime<DesktopTerminalActionCatalog>;
}

export function registerDesktopTerminalActions({
	findController,
	scope,
	terminal,
	typedRuntime,
}: RegisterDesktopTerminalActionsOptions): readonly ActionDisposable[] {
	const category = {
		id: 'terminal',
		title: desktopTerminalMessages.actionCategory,
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
					reason: desktopTerminalMessages.actionUnavailable,
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
				id: DESKTOP_TERMINAL_ACTION_IDS.findOpen,
				input: noActionInput,
				title: desktopTerminalMessages.findOpenAction,
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
				id: DESKTOP_TERMINAL_ACTION_IDS.findNext,
				input: noActionInput,
				title: desktopTerminalMessages.findNextAction,
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
				id: DESKTOP_TERMINAL_ACTION_IDS.findPrevious,
				input: noActionInput,
				title: desktopTerminalMessages.findPreviousAction,
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
						: { kind: 'disabled', reason: desktopTerminalMessages.actionUnavailable },
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
				id: DESKTOP_TERMINAL_ACTION_IDS.copySelection,
				input: noActionInput,
				title: desktopTerminalMessages.copyAction,
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
				id: DESKTOP_TERMINAL_ACTION_IDS.selectAll,
				input: noActionInput,
				title: desktopTerminalMessages.selectAllAction,
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
						: { kind: 'disabled', reason: desktopTerminalMessages.actionUnavailable },
				handler: {
					execute: async (context, input) => {
						if (input.target.kind !== 'url' || !permittedExternalTarget(input.target)) {
							findController.setNotice({ kind: 'link-blocked' });
							return { code: 'link-policy-blocked', kind: 'blocked' };
						}
						if (context.signal.aborted) return { kind: 'cancelled' };
						const result = await terminal.openLinkAsync(input.target.url);
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
				id: DESKTOP_TERMINAL_ACTION_IDS.openLink,
				input: desktopTerminalLinkActionInput,
				title: desktopTerminalMessages.openLinkAction,
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
