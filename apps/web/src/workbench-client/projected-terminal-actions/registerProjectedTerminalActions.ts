import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { projectedTerminalMessages } from '@orchbee/i18n';
import {
	normalizePermittedExternalTerminalUrl,
	type ProjectedPasteResult,
	type ProjectedTerminalLinkOpenRequest,
	type ProjectedTerminalSideEffectResult,
	type TerminalOpenTarget,
} from '@orchbee/terminal';
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
	clear(): boolean;
	copySelectionAsync(): Promise<ProjectedTerminalSideEffectResult>;
	findNext(): boolean;
	findPrevious(): boolean;
	linkOpenRequestIsCurrent(request: ProjectedTerminalLinkOpenRequest): boolean;
	openLinkAsync(target: string): Promise<ProjectedTerminalSideEffectResult>;
	paste(text: string, source: 'clipboard'): Promise<ProjectedPasteResult>;
	reset(): void;
	scrollToBottom(): boolean;
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
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'terminal-clipboard-paste',
					mode: 'foreground',
				},
				getAvailability: () =>
					terminal.canControlTerminal && platformEffects.readClipboardText !== undefined
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: projectedTerminalMessages.actionUnavailable },
				handler: {
					execute: async (context) => {
						if (context.signal.aborted) return { kind: 'cancelled' };
						const clipboard = await platformEffects.readClipboardText?.();
						if (clipboard === undefined) {
							return { code: 'clipboard-read-unavailable', kind: 'blocked' };
						}
						if (clipboard.kind === 'cancelled' || context.signal.aborted) {
							return { kind: 'cancelled' };
						}
						if (clipboard.kind === 'failed') {
							return { failure: { code: clipboard.code }, kind: 'failed' };
						}
						return pasteClipboardText(context, await terminal.paste(clipboard.text, 'clipboard'));
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.paste,
				input: noActionInput,
				title: projectedTerminalMessages.pasteAction,
			}),
		),
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
						findController.open(
							terminal.selection.state === 'active'
								? (terminal.selection.preview ?? undefined)
								: undefined,
						);
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
					concurrencyKey: () => 'terminal-clear',
					mode: 'foreground',
				},
				getAvailability: () =>
					terminal.canControlTerminal
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: projectedTerminalMessages.actionUnavailable },
				handler: {
					execute: (context) => commitBooleanResult(context, terminal.clear()),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.clear,
				input: noActionInput,
				title: projectedTerminalMessages.clearAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'replace-latest',
					concurrencyKey: () => 'terminal-history-scroll',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context) => commitBooleanResult(context, terminal.scrollToBottom()),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.scrollToBottom,
				input: noActionInput,
				title: projectedTerminalMessages.scrollToBottomAction,
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
					terminal.selection.state === 'active' && clipboardWriteIsAvailable(platformEffects)
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: projectedTerminalMessages.actionUnavailable },
				handler: {
					execute: async (context) => {
						if (context.signal.aborted) return { kind: 'cancelled' };
						const result = await terminal.copySelectionAsync();
						if (result.kind === 'completed') {
							context.markCommitted();
							return { kind: 'completed' };
						}
						if (result.kind === 'cancelled') return { kind: 'cancelled' };
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
				getAvailability: (_context, input) => {
					const request = terminalLinkOpenRequest(input);
					return request !== null && normalizedExternalTarget(request.target) !== null
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: projectedTerminalMessages.actionUnavailable };
				},
				handler: {
					execute: async (context, input) => {
						const request = terminalLinkOpenRequest(input);
						const normalizedTarget =
							request === null ? null : normalizedExternalTarget(request.target);
						if (
							request === null ||
							normalizedTarget === null ||
							!terminal.linkOpenRequestIsCurrent(request)
						) {
							return { code: 'stale-terminal-link-context', kind: 'blocked' };
						}
						if (context.signal.aborted) return { kind: 'cancelled' };
						const result = await (platformEffects.openExternal?.(normalizedTarget) ??
							terminal.openLinkAsync(normalizedTarget));
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
	return commitBooleanResult(context, moved);
}

function commitBooleanResult(context: ActionExecutionContext, committed: boolean) {
	if (!committed) return { kind: 'unchanged' as const };
	context.markCommitted();
	return { kind: 'completed' as const };
}

function pasteClipboardText(context: ActionExecutionContext, result: ProjectedPasteResult) {
	if (result.kind === 'empty') return { kind: 'unchanged' as const };
	if (result.kind === 'blocked') return { code: 'paste-blocked', kind: 'blocked' as const };
	context.markCommitted();
	return { kind: 'completed' as const };
}

function clipboardWriteIsAvailable(platformEffects: ProjectedTerminalPlatformEffects): boolean {
	const isClipboardWriteAvailable = platformEffects.isClipboardWriteAvailable;
	if (isClipboardWriteAvailable === undefined) return true;
	try {
		return isClipboardWriteAvailable();
	} catch {
		return false;
	}
}

function terminalLinkOpenRequest(
	input: ProjectedTerminalActionCatalog[typeof PROJECTED_TERMINAL_ACTION_IDS.openLink]['input'],
): ProjectedTerminalLinkOpenRequest | null {
	return input.context !== undefined && input.target.kind === 'url'
		? { context: input.context, target: input.target }
		: null;
}

function normalizedExternalTarget(target: TerminalOpenTarget): string | null {
	return target.kind === 'url' ? normalizePermittedExternalTerminalUrl(target.url) : null;
}
