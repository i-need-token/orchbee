import {
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { projectedTerminalMessages } from '@orchbee/i18n';
import {
	normalizePermittedExternalTerminalUrl,
	type ProjectedPasteResult,
	type ProjectedCommandOutputReadResult,
	type ProjectedScrollbackReadResult,
	type TerminalHintChoiceResult,
	type ProjectedTerminalLinkOpenRequest,
	type ProjectedTerminalSideEffectResult,
	type TerminalOpenTarget,
} from '@orchbee/terminal';
import type { SelectionChanged } from '@orchbee/terminal-protocol';

import type { TerminalFindController } from '../terminal-find-surface';
import {
	commandOutputViewerDocument,
	scrollbackViewerDocument,
	TerminalTextViewerController,
} from '../terminal-text-viewer';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import {
	createProjectedTerminalClipboardActionCoordinator,
	type ProjectedTerminalClipboardActionCoordinator,
} from './createProjectedTerminalClipboardActionCoordinator';
import { createProjectedTerminalPromptNavigationCoordinator } from './createProjectedTerminalPromptNavigationCoordinator';
import { createProjectedTerminalTextViewerPreparationCoordinator } from './createProjectedTerminalTextViewerPreparationCoordinator';
import {
	terminalHistoryUnavailableReason,
	terminalInputUnavailableReason,
	terminalNotDrawnReason,
	terminalScreenUnavailableReason,
	UNKNOWN_PROJECTED_TERMINAL_ACTION_CONDITIONS,
	type ProjectedTerminalActionConditions,
} from './projected-terminal-action-conditions';
import {
	PROJECTED_TERMINAL_ACTION_IDS,
	type ProjectedTerminalActionCatalog,
	projectedTerminalLinkActionInput,
} from './projected-terminal-action-ids';

export interface ProjectedTerminalActionTarget {
	/** Input needs terminal control. */
	readonly canControlTerminal: boolean;
	/** Clear and Reset are screen operations any viewer may run without terminal control. */
	readonly canControlScreen: boolean;
	readonly canShowHints: boolean;
	readonly canNavigatePrompts: boolean;
	readonly canShowCommandOutput: boolean;
	readonly canShowScrollback: boolean;
	readonly canScrollHistory: boolean;
	readonly selection: SelectionChanged;
	clear(): boolean;
	copySelectionAsync(): Promise<ProjectedTerminalSideEffectResult>;
	findNext(): boolean;
	findPrevious(): boolean;
	chooseVisibleHint(signal: AbortSignal): Promise<TerminalHintChoiceResult>;
	linkOpenRequestIsCurrent(request: ProjectedTerminalLinkOpenRequest): boolean;
	openLinkAsync(target: string): Promise<ProjectedTerminalSideEffectResult>;
	openTargetAsync(
		target: Extract<TerminalOpenTarget, { readonly kind: 'file' | 'url' }>,
	): Promise<ProjectedTerminalSideEffectResult>;
	paste(text: string, source: 'api' | 'clipboard'): Promise<ProjectedPasteResult>;
	reset(): void;
	readLastCommandOutput(signal: AbortSignal): Promise<ProjectedCommandOutputReadResult>;
	readScrollback(signal: AbortSignal): Promise<ProjectedScrollbackReadResult>;
	scrollLines(amount: number): boolean;
	scrollPages(amount: number): boolean;
	scrollToNextPrompt(signal: AbortSignal): Promise<boolean>;
	scrollToBottom(): boolean;
	scrollToPreviousPrompt(signal: AbortSignal): Promise<boolean>;
	scrollToTop(): boolean;
	selectAll(): void;
}

export interface RegisterProjectedTerminalActionsOptions {
	readonly clipboardActionCoordinator?: ProjectedTerminalClipboardActionCoordinator;
	/** Lets a disabled action name the surface state that stops it. */
	readonly conditions?: ProjectedTerminalActionConditions;
	readonly findController: TerminalFindController;
	readonly textViewerController?: TerminalTextViewerController;
	readonly platformEffects?: ProjectedTerminalPlatformEffects;
	readonly scope: ActionScope;
	readonly terminal: ProjectedTerminalActionTarget;
	readonly typedRuntime: TypedActionRuntime<ProjectedTerminalActionCatalog>;
}

export function registerProjectedTerminalActions({
	clipboardActionCoordinator = createProjectedTerminalClipboardActionCoordinator(),
	conditions = UNKNOWN_PROJECTED_TERMINAL_ACTION_CONDITIONS,
	findController,
	textViewerController = new TerminalTextViewerController(),
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
	const searchAvailable = (): ActionAvailability => {
		const find = findController.getSnapshot();
		if (!find.isOpen) {
			return { kind: 'disabled', reason: projectedTerminalMessages.unavailableFindClosed };
		}
		return find.query.length > 0
			? { kind: 'enabled' }
			: { kind: 'disabled', reason: projectedTerminalMessages.unavailableFindEmpty };
	};
	const promptNavigationExecution = {
		concurrency: 'replace-latest' as const,
		concurrencyKey: () => 'terminal-prompt-navigation',
		mode: 'foreground' as const,
	};
	const historyAvailability = (available: boolean): ActionAvailability =>
		available
			? { kind: 'enabled' }
			: { kind: 'disabled', reason: terminalHistoryUnavailableReason(conditions) };
	const inputAvailability = (): ActionAvailability =>
		terminal.canControlTerminal
			? { kind: 'enabled' }
			: { kind: 'disabled', reason: terminalInputUnavailableReason(conditions) };
	const screenAvailability = (): ActionAvailability => {
		const reason = terminalScreenUnavailableReason(conditions);
		return terminal.canControlScreen && reason === null
			? { kind: 'enabled' }
			: {
					kind: 'disabled',
					reason: reason ?? projectedTerminalMessages.unavailableInputClosed,
				};
	};
	const clipboardWriteAvailability = (): ActionAvailability =>
		clipboardWriteIsAvailable(platformEffects)
			? { kind: 'enabled' }
			: { kind: 'disabled', reason: projectedTerminalMessages.unavailableClipboardWrite };
	const viewerCopyAvailability = (source: 'command-output' | 'scrollback'): ActionAvailability => {
		const document = textViewerController.getSnapshot().document;
		if (document?.kind !== 'text' || document.source !== source) {
			return {
				kind: 'disabled',
				reason:
					source === 'command-output'
						? projectedTerminalMessages.unavailableOutputHidden
						: projectedTerminalMessages.unavailableScrollbackHidden,
			};
		}
		return platformEffects.writeClipboardText === undefined
			? { kind: 'disabled', reason: projectedTerminalMessages.unavailableClipboardWrite }
			: clipboardWriteAvailability();
	};
	const promptNavigationAvailable = () => historyAvailability(terminal.canNavigatePrompts);
	const promptNavigationCoordinator = createProjectedTerminalPromptNavigationCoordinator();
	const textViewerPreparationCoordinator =
		createProjectedTerminalTextViewerPreparationCoordinator();
	const historyScrollExecution = {
		concurrency: 'replace-latest' as const,
		concurrencyKey: () => 'terminal-history-scroll',
		mode: 'foreground' as const,
	};
	const historyScrollAvailable = () => historyAvailability(terminal.canScrollHistory);
	return [
		promptNavigationCoordinator,
		textViewerPreparationCoordinator,
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'replace-latest',
					concurrencyKey: () => 'terminal-text-viewer',
					mode: 'foreground',
				},
				getAvailability: () => historyAvailability(terminal.canShowCommandOutput),
				handler: {
					execute: async (context) => {
						const prepared = await textViewerPreparationCoordinator.run(context.signal, (signal) =>
							terminal.readLastCommandOutput(signal),
						);
						if (prepared.kind === 'cancelled' || prepared.value.kind === 'cancelled') {
							return { kind: 'cancelled' };
						}
						const result = prepared.value;
						findController.close();
						textViewerController.open(commandOutputViewerDocument(result));
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.outputShowLast,
				input: noActionInput,
				title: projectedTerminalMessages.outputShowLastAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'replace-latest',
					concurrencyKey: () => 'terminal-text-viewer',
					mode: 'foreground',
				},
				getAvailability: () => historyAvailability(terminal.canShowScrollback),
				handler: {
					execute: async (context) => {
						const prepared = await textViewerPreparationCoordinator.run(context.signal, (signal) =>
							terminal.readScrollback(signal),
						);
						if (prepared.kind === 'cancelled' || prepared.value.kind === 'cancelled') {
							return { kind: 'cancelled' };
						}
						const result = prepared.value;
						findController.close();
						textViewerController.open(scrollbackViewerDocument(result));
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.historyShow,
				input: noActionInput,
				title: projectedTerminalMessages.scrollbackShowAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'terminal-command-output-copy',
					mode: 'foreground',
				},
				getAvailability: () => viewerCopyAvailability('command-output'),
				handler: {
					execute: async (context) => {
						if (context.signal.aborted) return { kind: 'cancelled' };
						const document = textViewerController.getSnapshot().document;
						const writeClipboardText = platformEffects.writeClipboardText;
						if (
							document?.kind !== 'text' ||
							document.source !== 'command-output' ||
							writeClipboardText === undefined
						) {
							return { code: 'command-output-unavailable', kind: 'blocked' };
						}
						let result: ProjectedTerminalSideEffectResult;
						try {
							result = await writeClipboardText(document.text);
						} catch {
							result = {
								code: 'client-clipboard-failed',
								kind: 'failed',
								retryable: false,
							};
						}
						if (result.kind === 'completed') {
							context.markCommitted();
							return { kind: 'completed' };
						}
						if (result.kind === 'cancelled') return { kind: 'cancelled' };
						return { failure: { code: result.code }, kind: 'failed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.outputCopy,
				input: noActionInput,
				title: projectedTerminalMessages.outputCopyAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'terminal-scrollback-copy',
					mode: 'foreground',
				},
				getAvailability: () => viewerCopyAvailability('scrollback'),
				handler: {
					execute: async (context) => {
						if (context.signal.aborted) return { kind: 'cancelled' };
						const document = textViewerController.getSnapshot().document;
						const writeClipboardText = platformEffects.writeClipboardText;
						if (
							document?.kind !== 'text' ||
							document.source !== 'scrollback' ||
							writeClipboardText === undefined
						) {
							return { code: 'scrollback-unavailable', kind: 'blocked' };
						}
						let result: ProjectedTerminalSideEffectResult;
						try {
							result = await writeClipboardText(document.text);
						} catch {
							result = {
								code: 'client-clipboard-failed',
								kind: 'failed',
								retryable: false,
							};
						}
						if (result.kind === 'completed') {
							context.markCommitted();
							return { kind: 'completed' };
						}
						if (result.kind === 'cancelled') return { kind: 'cancelled' };
						return { failure: { code: result.code }, kind: 'failed' };
					},
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.historyCopy,
				input: noActionInput,
				title: projectedTerminalMessages.scrollbackCopyAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: promptNavigationExecution,
				getAvailability: promptNavigationAvailable,
				handler: {
					execute: (context) =>
						navigatePrompt(context, promptNavigationCoordinator, (signal) =>
							terminal.scrollToPreviousPrompt(signal),
						),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.promptPrevious,
				input: noActionInput,
				title: projectedTerminalMessages.promptPreviousAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'replace-latest',
					concurrencyKey: () => 'terminal-hints',
					mode: 'foreground',
				},
				getAvailability: () => {
					if (terminal.canShowHints) return { kind: 'enabled' };
					return {
						kind: 'disabled',
						reason: terminal.canControlTerminal
							? terminalNotDrawnReason(conditions)
							: terminalInputUnavailableReason(conditions),
					};
				},
				handler: {
					execute: async (context) =>
						completeTerminalHintChoice(
							context,
							await terminal.chooseVisibleHint(context.signal),
							terminal,
							platformEffects,
						),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.hintsOpen,
				input: noActionInput,
				title: projectedTerminalMessages.hintsOpenAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: promptNavigationExecution,
				getAvailability: promptNavigationAvailable,
				handler: {
					execute: (context) =>
						navigatePrompt(context, promptNavigationCoordinator, (signal) =>
							terminal.scrollToNextPrompt(signal),
						),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.promptNext,
				input: noActionInput,
				title: projectedTerminalMessages.promptNextAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: historyScrollExecution,
				getAvailability: historyScrollAvailable,
				handler: {
					execute: (context) => commitBooleanResult(context, terminal.scrollLines(-1)),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.scrollLineUp,
				input: noActionInput,
				title: projectedTerminalMessages.scrollLineUpAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: historyScrollExecution,
				getAvailability: historyScrollAvailable,
				handler: {
					execute: (context) => commitBooleanResult(context, terminal.scrollLines(1)),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.scrollLineDown,
				input: noActionInput,
				title: projectedTerminalMessages.scrollLineDownAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: historyScrollExecution,
				getAvailability: historyScrollAvailable,
				handler: {
					execute: (context) => commitBooleanResult(context, terminal.scrollPages(-1)),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.scrollPageUp,
				input: noActionInput,
				title: projectedTerminalMessages.scrollPageUpAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: historyScrollExecution,
				getAvailability: historyScrollAvailable,
				handler: {
					execute: (context) => commitBooleanResult(context, terminal.scrollPages(1)),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.scrollPageDown,
				input: noActionInput,
				title: projectedTerminalMessages.scrollPageDownAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: historyScrollExecution,
				getAvailability: historyScrollAvailable,
				handler: {
					execute: (context) => commitBooleanResult(context, terminal.scrollToTop()),
				},
				id: PROJECTED_TERMINAL_ACTION_IDS.scrollToTop,
				input: noActionInput,
				title: projectedTerminalMessages.scrollToTopAction,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: historyScrollExecution,
				getAvailability: historyScrollAvailable,
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
					concurrencyKey: () => 'terminal-clipboard-paste',
					mode: 'foreground',
				},
				getAvailability: () => {
					if (!terminal.canControlTerminal) return inputAvailability();
					return platformEffects.readClipboardText === undefined
						? { kind: 'disabled', reason: projectedTerminalMessages.unavailableClipboardRead }
						: { kind: 'enabled' };
				},
				handler: {
					execute: (context) =>
						clipboardActionCoordinator.runPaste(async () => {
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
						}),
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
						textViewerController.close();
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
				getAvailability: screenAvailability,
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
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'terminal-reset',
					mode: 'foreground',
				},
				getAvailability: screenAvailability,
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
				getAvailability: () => {
					switch (terminal.selection.state) {
						case 'none':
							return { kind: 'disabled', reason: projectedTerminalMessages.unavailableNoSelection };
						case 'evicted':
							return {
								kind: 'disabled',
								reason: projectedTerminalMessages.unavailableSelectionEvicted,
							};
						case 'active':
							return clipboardWriteAvailability();
					}
				},
				handler: {
					execute: async (context) => {
						if (context.signal.aborted) return { kind: 'cancelled' };
						const result: ProjectedTerminalSideEffectResult =
							await clipboardActionCoordinator.trackSelectionCopy(terminal.copySelectionAsync());
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
					if (request === null) {
						return { kind: 'disabled', reason: projectedTerminalMessages.unavailableLinkMissing };
					}
					return terminalOpenTarget(request.target) === null
						? { kind: 'disabled', reason: projectedTerminalMessages.unavailableLinkScheme }
						: { kind: 'enabled' };
				},
				handler: {
					execute: async (context, input) => {
						const request = terminalLinkOpenRequest(input);
						const target = request === null ? null : terminalOpenTarget(request.target);
						if (
							request === null ||
							target === null ||
							!terminal.linkOpenRequestIsCurrent(request)
						) {
							return { code: 'stale-terminal-link-context', kind: 'blocked' };
						}
						if (context.signal.aborted) return { kind: 'cancelled' };
						const result =
							target.kind === 'url'
								? await (platformEffects.openExternal?.(target.url) ??
										terminal.openLinkAsync(target.url))
								: await terminal.openTargetAsync(target);
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

async function navigatePrompt(
	context: ActionExecutionContext,
	coordinator: ReturnType<typeof createProjectedTerminalPromptNavigationCoordinator>,
	navigate: (signal: AbortSignal) => Promise<boolean>,
) {
	const result = await coordinator.run(context.signal, navigate);
	if (result === 'cancelled') return { kind: 'cancelled' as const };
	return commitBooleanResult(context, result === 'moved');
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
	if (input.context === undefined) return null;
	if (input.target.kind === 'url') return { context: input.context, target: input.target };
	if (input.target.kind !== 'file') return null;
	return {
		context: input.context,
		target: {
			kind: 'file',
			path: input.target.path,
			...(typeof input.target.line === 'number' ? { line: input.target.line } : {}),
			...(typeof input.target.column === 'number' ? { column: input.target.column } : {}),
		},
	};
}

function terminalOpenTarget(
	target: TerminalOpenTarget,
): Extract<TerminalOpenTarget, { readonly kind: 'file' | 'url' }> | null {
	if (target.kind === 'file') return target;
	if (target.kind !== 'url') return null;
	const url = normalizePermittedExternalTerminalUrl(target.url);
	return url === null ? null : { kind: 'url', url };
}

function normalizedExternalTarget(target: TerminalOpenTarget): string | null {
	return target.kind === 'url' ? normalizePermittedExternalTerminalUrl(target.url) : null;
}

async function completeTerminalHintChoice(
	context: ActionExecutionContext,
	choice: TerminalHintChoiceResult,
	terminal: ProjectedTerminalActionTarget,
	platformEffects: ProjectedTerminalPlatformEffects,
) {
	switch (choice.kind) {
		case 'cancelled':
			return { kind: 'cancelled' as const };
		case 'empty':
			return { kind: 'unchanged' as const };
		case 'stale':
			return { code: 'stale-terminal-hints', kind: 'blocked' as const };
		case 'unavailable':
			return { code: 'terminal-hints-unavailable', kind: 'blocked' as const };
		case 'selected': {
			if (context.signal.aborted) return { kind: 'cancelled' as const };
			const target = normalizedExternalTarget(choice.hint.target);
			if (choice.hint.kind === 'url') {
				if (target === null) {
					return { code: 'terminal-hint-policy-blocked', kind: 'blocked' as const };
				}
				const result = await (platformEffects.openExternal?.(target) ??
					terminal.openLinkAsync(target));
				if (result.kind === 'cancelled') {
					return { kind: 'cancelled' as const };
				}
				if (result.kind === 'failed') {
					return { failure: { code: result.code }, kind: 'failed' as const };
				}
				context.markCommitted();
				return { kind: 'completed' as const };
			}
			const result = await terminal.paste(choice.hint.text, 'api');
			if (result.kind === 'empty') return { kind: 'unchanged' as const };
			if (result.kind === 'blocked') {
				return { code: 'terminal-hint-insert-blocked', kind: 'blocked' as const };
			}
			context.markCommitted();
			return { kind: 'completed' as const };
		}
	}
}
