import { ActionRuntime, type ActionDiscoveryResult } from '@orchbee/actions';
import { ProjectedTerminal, type ProjectedTerminalOptions } from '@orchbee/terminal';
import type { WorkbenchTerminalActionContextProvider } from '@orchbee/workbench';
import type { ProjectedTerminalCommand } from '@orchbee/workbench-client-protocol';
import {
	ActiveScreen,
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	encodeTerminalFrame,
	encodeTerminalRowWindow,
	HostEpochSchema,
	MouseAction,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	TerminalIdSchema,
	TerminalRowWindowDetail,
	TerminalThemeStateSchema,
	UnderlineKind,
	type TerminalFrame,
} from '@orchbee/terminal-protocol';

import {
	mountTerminalFindSurface,
	type MountTerminalFindSurfaceOptions,
} from '../../terminal-find-surface';
import {
	mountTerminalCommandOutputViewer,
	type MountTerminalCommandOutputViewerOptions,
} from '../../terminal-command-output-viewer';
import {
	installProjectedTerminalShortcuts,
	PROJECTED_TERMINAL_ACTION_IDS,
} from '../../projected-terminal-actions';
import type { ProjectedTerminalTransport } from '../../terminal-projection';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';

const attachmentId = AttachmentIdSchema.parse('55'.repeat(16));
const creationId = CreationIdSchema.parse('66'.repeat(16));
const hostEpoch = HostEpochSchema.parse('77'.repeat(16));
const terminalId = TerminalIdSchema.parse('88'.repeat(16));
const capabilityRevision = CanonicalUint64TextSchema.parse('1');
const emptyGraphics = {
	animationTick: 0n,
	images: [],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [],
	resourceRevision: 0n,
} as const;
const linkUrl = 'https://orchbee.dev/pinned-context';
const historyRowText = Array.from({ length: 10 }, (_, index) => `history row ${index}`);
const rowText = ['linked context', 'terminal actions', 'localized feedback', 'native command'];
const theme = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x10_20_30,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xf4_f4_f4,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});
const minimalActionIds = [
	PROJECTED_TERMINAL_ACTION_IDS.copySelection,
	PROJECTED_TERMINAL_ACTION_IDS.paste,
	PROJECTED_TERMINAL_ACTION_IDS.selectAll,
	PROJECTED_TERMINAL_ACTION_IDS.findOpen,
	PROJECTED_TERMINAL_ACTION_IDS.findNext,
	PROJECTED_TERMINAL_ACTION_IDS.findPrevious,
	PROJECTED_TERMINAL_ACTION_IDS.outputShowLast,
	PROJECTED_TERMINAL_ACTION_IDS.outputCopy,
	PROJECTED_TERMINAL_ACTION_IDS.promptPrevious,
	PROJECTED_TERMINAL_ACTION_IDS.promptNext,
	PROJECTED_TERMINAL_ACTION_IDS.scrollLineUp,
	PROJECTED_TERMINAL_ACTION_IDS.scrollLineDown,
	PROJECTED_TERMINAL_ACTION_IDS.scrollPageUp,
	PROJECTED_TERMINAL_ACTION_IDS.scrollPageDown,
	PROJECTED_TERMINAL_ACTION_IDS.scrollToTop,
	PROJECTED_TERMINAL_ACTION_IDS.scrollToBottom,
	PROJECTED_TERMINAL_ACTION_IDS.clear,
	PROJECTED_TERMINAL_ACTION_IDS.reset,
] as const;

const root = document.createElement('main');
root.id = 'terminal-actions-witness';
root.style.boxSizing = 'border-box';
root.style.height = '240px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';
document.body.replaceChildren(root);
document.body.style.margin = '0';

Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });
Object.defineProperty(navigator, 'platform', { configurable: true, value: 'MacIntel' });
root.dataset['witnessForcedCanvas'] = 'true';

const actionRuntime = new ActionRuntime();
const workbenchScope = actionRuntime.createScope('workbench.fixture.terminal');
const commands: ProjectedTerminalCommand[] = [];
let contextProvider: WorkbenchTerminalActionContextProvider | undefined;
let externalOpenCount = 0;
let outputClipboardWriteCount = 0;
let terminal: ProjectedTerminal | undefined;
let transportListener: (value: unknown) => void = () => undefined;
let suppressRowResponses = false;
let projectedScrollbackRows = 0n;
const dependencies: ProjectedTerminalRendererDependencies = {
	createTerminal: (options: ProjectedTerminalOptions) => {
		terminal = new ProjectedTerminal(options);
		return terminal;
	},
	installShortcuts: installProjectedTerminalShortcuts,
	mountFindSurface: (options: MountTerminalFindSurfaceOptions) => mountTerminalFindSurface(options),
	mountOutputViewer: (options: MountTerminalCommandOutputViewerOptions) =>
		mountTerminalCommandOutputViewer(options),
};
const transport: ProjectedTerminalTransport = {
	close: () => undefined,
	projectionBufferTrust: 'local-host',
	send: (command) => {
		commands.push(command);
		root.dataset['witnessKeyInputCount'] = String(
			commands.filter((candidate) => candidate.kind === 'key-input').length,
		);
		root.dataset['witnessOutputShortcutKeyInputCount'] = String(
			commands.filter(
				(candidate) => candidate.kind === 'key-input' && candidate.payload.key === 'KeyG',
			).length,
		);
		if (command.kind !== 'read-rows' || suppressRowResponses) return;
		const scrollbackRows = projectedScrollbackRows;
		queueMicrotask(() => {
			transportListener({ kind: 'rows', rows: terminalRows(command, scrollbackRows) });
		});
	},
	subscribe: (listener) => {
		transportListener = listener;
		return () => undefined;
	},
};

void runWitness();

async function runWitness(): Promise<void> {
	try {
		await startProjectedTerminalRenderer(
			root,
			document.documentElement,
			['zh-CN'],
			'apple',
			{
				attachmentId,
				canInput: true,
				canResize: false,
				capabilityRevision,
				creationId,
				hostEpoch,
				terminalId,
				terminalOutcome: null,
				theme,
			},
			transport,
			dependencies,
			{
				openExternal: () => {
					externalOpenCount += 1;
					return { kind: 'completed' };
				},
				readClipboardText: () => ({ kind: 'completed', text: 'fixture clipboard text' }),
				writeClipboardText: (text) => {
					outputClipboardWriteCount += 1;
					root.dataset['witnessOutputClipboardWriteCount'] = String(outputClipboardWriteCount);
					root.dataset['witnessOutputCopiedText'] = text;
					return { kind: 'completed' };
				},
			},
			undefined,
			undefined,
			undefined,
			undefined,
			undefined,
			undefined,
			{
				admitBellEffect: () => true,
				bindContextProvider: (provider) => {
					contextProvider = provider;
					return {
						dispose: () => {
							if (contextProvider === provider) contextProvider = undefined;
						},
					};
				},
				runtime: actionRuntime,
				scope: workbenchScope,
			},
		);
		const activeTerminal = terminal;
		const activeProvider = contextProvider;
		if (activeTerminal === undefined || activeProvider === undefined) {
			throw new Error('Terminal Action composition did not bind to its Workbench scope.');
		}
		projectedScrollbackRows = 0n;
		transportListener({ frame: terminalSnapshot(1n, 1n), kind: 'frame' });
		await nextPaint();
		const canvas = activeTerminal.canvas;
		const input = activeTerminal.textarea;
		if (canvas === undefined || input === undefined) {
			throw new Error('Terminal Action witness surface was not created.');
		}

		const catalog = actionRuntime.discover({ scope: workbenchScope, source: 'context-menu' });
		const catalogById = new Map(catalog.map((result) => [result.descriptor.id, result] as const));
		for (const actionId of minimalActionIds) {
			if (catalogById.get(actionId)?.kind !== 'ready') {
				throw new Error(`Minimal terminal Action ${actionId} was not synchronously discoverable.`);
			}
		}
		root.dataset['witnessCatalog'] = minimalActionIds.join(',');
		root.dataset['witnessAvailability'] = minimalActionIds
			.map((actionId) => `${actionId}:${catalogById.get(actionId)?.availability.kind}`)
			.join(',');
		root.dataset['witnessWorkbenchScope'] = String(
			minimalActionIds.every((actionId) => catalogById.get(actionId)?.scope === workbenchScope),
		);

		input.focus();
		const unboundCommand = appleCommand('KeyX', 'x');
		input.dispatchEvent(unboundCommand);
		const unavailableCommand = appleCommand('KeyC', 'c');
		input.dispatchEvent(unavailableCommand);
		const claimedCommand = appleCommand('KeyF', 'f');
		input.dispatchEvent(claimedCommand);
		const clearCommand = appleCommand('KeyK', 'k');
		input.dispatchEvent(clearCommand);
		root.dataset['witnessAppleCommands'] = [
			`unbound-native:${!unboundCommand.defaultPrevented}`,
			`unavailable-native:${!unavailableCommand.defaultPrevented}`,
			`available-claimed:${claimedCommand.defaultPrevented}`,
			`clear-claimed:${clearCommand.defaultPrevented}`,
		].join(',');

		const pasteResult = await executeReady(
			activeProvider,
			catalogById.get(PROJECTED_TERMINAL_ACTION_IDS.paste),
		);
		await until(() => terminalStatusText() === '已将剪贴板文本粘贴到终端。');
		const pasteFeedback = terminalStatusText();
		const scrollResult = await executeReady(
			activeProvider,
			catalogById.get(PROJECTED_TERMINAL_ACTION_IDS.scrollToBottom),
		);
		const clearResult = await executeReady(
			activeProvider,
			catalogById.get(PROJECTED_TERMINAL_ACTION_IDS.clear),
		);
		await until(() => terminalStatusText() === '已清除终端输出。');
		const clearFeedback = terminalStatusText();

		canvas.dispatchEvent(
			new PointerEvent('pointermove', {
				bubbles: true,
				buttons: 0,
				clientX: 4,
				clientY: 4,
				metaKey: true,
				pointerType: 'mouse',
				shiftKey: true,
			}),
		);
		await until(() => canvas.style.cursor === 'pointer');
		const applicationContext = activeProvider.capture(contextRequest(false));
		const productContext = activeProvider.capture(contextRequest(true));
		if (productContext.kind !== 'product') {
			throw new Error('Shift did not return terminal pointer ownership to the product.');
		}
		const linkCatalog = actionRuntime.discover(
			{
				...(productContext.focus === undefined ? {} : { focus: productContext.focus }),
				scope: workbenchScope,
				source: 'context-menu',
				...(productContext.target === undefined ? {} : { target: productContext.target }),
			},
			{ inputByActionId: productContext.actionInputs ?? {} },
		);
		const linkAction = linkCatalog.find(
			(result) =>
				result.kind === 'ready' && result.descriptor.id === PROJECTED_TERMINAL_ACTION_IDS.openLink,
		);
		if (linkAction?.kind !== 'ready' || linkAction.availability.kind !== 'enabled') {
			throw new Error('Pinned terminal link context was not discoverable.');
		}
		projectedScrollbackRows = 0n;
		transportListener({ frame: terminalSnapshot(2n, 2n), kind: 'frame' });
		await nextPaint();
		const staleLinkResult = await activeProvider.executePrepared(linkAction.invocation).completion;
		await until(() => terminalStatusText() === '终端链接已发生变化，无法打开。');
		const staleFeedback = terminalStatusText();

		projectedScrollbackRows = 10n;
		transportListener({ frame: terminalSnapshot(3n, 3n, projectedScrollbackRows), kind: 'frame' });
		await nextPaint();
		suppressRowResponses = true;
		activeTerminal.scrollLines(-1);
		suppressRowResponses = false;
		activeTerminal.notifySelectionChanged({
			attachmentId,
			end: { column: 6, row: CanonicalUint64TextSchema.parse('10') },
			preview: 'linked',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			state: 'active',
		});
		const accessibilityRow = root.querySelector('[data-terminal-absolute-row="10"]');
		const accessibilityText = accessibilityRow?.firstChild;
		const accessibilitySelection = document.getSelection();
		if (accessibilityText === null || accessibilityText === undefined) {
			throw new Error('Pre-input witness could not resolve the accessibility row text.');
		}
		if (accessibilitySelection === null) {
			throw new Error('Pre-input witness could not resolve the document selection.');
		}
		const accessibilityRange = document.createRange();
		accessibilityRange.setStart(accessibilityText, 0);
		accessibilityRange.setEnd(accessibilityText, 6);
		accessibilitySelection.removeAllRanges();
		accessibilitySelection.addRange(accessibilityRange);
		document.dispatchEvent(new Event('selectionchange'));
		const buttonlessMotionCommandStart = commands.length;
		canvas.dispatchEvent(
			new PointerEvent('pointermove', {
				bubbles: true,
				buttons: 0,
				clientX: 4,
				clientY: 4,
				pointerType: 'mouse',
			}),
		);
		const buttonlessMotionCommands = commands.slice(buttonlessMotionCommandStart);
		root.dataset['witnessButtonlessMotionReportCount'] = String(
			buttonlessMotionCommands.filter(
				(command) =>
					command.kind === 'mouse-input' &&
					command.payload.action === MouseAction.motion &&
					command.payload.button === null &&
					command.payload.pressedButtons === 0,
			).length,
		);
		root.dataset['witnessButtonlessMotionSelectionClearCount'] = String(
			buttonlessMotionCommands.filter(
				(command) => command.kind === 'selection-command' && command.payload.command === 'clear',
			).length,
		);
		root.dataset['witnessButtonlessMotionHistoryBottomCount'] = String(
			buttonlessMotionCommands.filter(
				(command) =>
					command.kind === 'set-history-demand' && command.payload.demand === 'following-bottom',
			).length,
		);
		root.dataset['witnessButtonlessMotionSelectionPreserved'] = String(
			activeTerminal.getSelectionRange() !== null,
		);

		const focusLossCommandStart = commands.length;
		window.dispatchEvent(new Event('blur'));
		const documentSelection = document.getSelection();
		if (documentSelection === null || documentSelection.rangeCount !== 1) {
			throw new Error('Focus-loss witness did not retain the accessible selection mirror.');
		}
		const documentRange = documentSelection.getRangeAt(0);
		documentRange.collapse(true);
		document.dispatchEvent(new Event('selectionchange'));
		const focusLossCommands = commands.slice(focusLossCommandStart);
		root.dataset['witnessFocusLossSelectionClearCount'] = String(
			focusLossCommands.filter(
				(command) => command.kind === 'selection-command' && command.payload.command === 'clear',
			).length,
		);
		root.dataset['witnessFocusLossSelectionPreserved'] = String(
			activeTerminal.getSelectionRange() !== null,
		);

		root.dataset['witnessActionInputs'] = Object.keys(productContext.actionInputs ?? {}).join(',');
		root.dataset['witnessApplicationMouseOwned'] = String(
			applicationContext.kind === 'terminal-owned',
		);
		root.dataset['witnessConfirmationCount'] = String(
			root.querySelectorAll('[role="dialog"]').length,
		);
		root.dataset['witnessExternalOpenCount'] = String(externalOpenCount);
		root.dataset['witnessFeedback'] = [pasteFeedback, clearFeedback, staleFeedback].join('|');
		root.dataset['witnessPinnedContext'] = String(
			productContext.actionInputs?.[PROJECTED_TERMINAL_ACTION_IDS.openLink] !== undefined,
		);
		root.dataset['witnessTypedResults'] = [
			`paste:${pasteResult.kind}`,
			`scroll:${scrollResult.kind}`,
			`clear:${clearResult.kind}`,
			`stale-link:${staleLinkResult.kind}:${
				staleLinkResult.kind === 'blocked' ? staleLinkResult.code : 'unexpected'
			}`,
		].join(',');
		root.dataset['witnessReady'] = 'true';
		root.dataset['witnessOutputReady'] = 'true';
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	}
}

function appleCommand(code: string, key: string): KeyboardEvent {
	return new KeyboardEvent('keydown', {
		bubbles: true,
		cancelable: true,
		code,
		key,
		metaKey: true,
	});
}

function contextRequest(shiftKey: boolean) {
	return {
		altKey: false,
		clientX: 4,
		clientY: 4,
		ctrlKey: false,
		metaKey: true,
		shiftKey,
	};
}

async function executeReady(
	provider: WorkbenchTerminalActionContextProvider,
	result: ActionDiscoveryResult | undefined,
) {
	if (result?.kind !== 'ready' || result.availability.kind !== 'enabled') {
		throw new Error('Expected an enabled prepared terminal Action.');
	}
	return await provider.executePrepared(result.invocation).completion;
}

function terminalStatusText(): string | null {
	return (
		root.querySelector<HTMLElement>('[data-projected-terminal-status="true"]')?.textContent ?? null
	);
}

function terminalRows(
	command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>,
	scrollbackRows: bigint,
): Uint8Array {
	if (command.payload.origin.kind !== 'absolute') {
		throw new Error('Terminal link witness requested a non-absolute row window.');
	}
	const startRow = BigInt(command.payload.origin.row);
	const retainedRows = [...historyRowText.slice(0, Number(scrollbackRows)), ...rowText];
	const selectedRows = retainedRows.slice(
		Number(startRow),
		Number(startRow) + command.payload.rowCount,
	);
	const revision = BigInt(command.payload.revision);
	const sequence = BigInt(command.payload.requestSequence);
	const generation = BigInt(command.payload.generation);
	const frame = terminalFrame(revision, sequence, true, startRow);
	const linkRow = Number(scrollbackRows - startRow);
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: frame.columns,
		detail: TerminalRowWindowDetail.render,
		frame: {
			...frame,
			generation,
			rowData: terminalRowData(selectedRows, linkRow, startRow),
			rows: selectedRows.length,
			scrollbackRows: startRow,
		},
		generation,
		graphics: emptyGraphics,
		hostEpoch,
		revision,
		sequence,
		startRow,
		totalRows: BigInt(retainedRows.length),
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function terminalSnapshot(revision: bigint, sequence: bigint, scrollbackRows = 0n): Uint8Array {
	const encoded = encodeTerminalFrame(terminalFrame(revision, sequence, false, scrollbackRows));
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function terminalFrame(
	revision: bigint,
	sequence: bigint,
	link: boolean,
	scrollbackRows = 0n,
): TerminalFrame {
	const regularStyle: TerminalFrame['styles'][number] = {
		background: { kind: TerminalColorKind.default },
		blink: false,
		bold: false,
		faint: false,
		foreground: { kind: TerminalColorKind.default },
		hyperlink: null,
		inverse: false,
		invisible: false,
		italic: false,
		overline: false,
		strikethrough: false,
		underline: { kind: TerminalColorKind.none },
		underlineKind: UnderlineKind.none,
	};
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 24,
		cursor: {
			blinking: false,
			column: 3,
			row: 2,
			shape: CursorShape.bar,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/fixture',
		defaultColors: { backgroundRgb: 0x10_20_30, cursorRgb: null, foregroundRgb: 0xf4_f4_f4 },
		extensions: [],
		generation: 1n,
		hostEpoch,
		interaction: {
			applicationCursorKeys: false,
			applicationKeypad: false,
			bracketedPaste: false,
			focusReporting: false,
			kittyDisambiguateEscapeCodes: false,
			kittyReportAllKeysAsEscapeCodes: false,
			kittyReportAlternateKeys: false,
			kittyReportAssociatedText: false,
			kittyReportEventTypes: false,
			modifyOtherKeys2: false,
			mouseAny: true,
			mouseButton: false,
			mouseNormal: false,
			mouseSgr: true,
			mouseSgrPixels: false,
			mouseUtf8: false,
			mouseUrxvt: false,
			mouseX10: false,
		},
		kind: TerminalFrameKind.snapshot,
		revision,
		rowData: terminalRowData(rowText, link ? 0 : -1, scrollbackRows),
		rows: rowText.length,
		scrollbackRows,
		sequence,
		styles: [regularStyle, { ...regularStyle, hyperlink: linkUrl }],
		themeRevision: 0n,
		title: 'IN-09 IN-11 IN-12 terminal Actions witness',
	};
}

function terminalRowData(
	texts: readonly string[],
	linkRow: number,
	absoluteStartRow: bigint,
): TerminalFrame['rowData'] {
	return texts.map((text, row) => {
		const absoluteRow = absoluteStartRow + BigInt(row);
		const prompt = absoluteRow === 7n || absoluteRow === 10n;
		let semanticContent: SemanticContent = SemanticContent.output;
		if (prompt) semanticContent = SemanticContent.prompt;
		else if (absoluteRow > 10n) semanticContent = SemanticContent.input;
		return {
			cells: Array.from({ length: 24 }, (_, column) => ({
				grapheme: text[column] ?? '',
				protected: false,
				semanticContent,
				style: row === linkRow && column < text.length ? 1 : 0,
				width: CellWidth.narrow,
			})),
			index: row,
			promptClass: prompt ? RowPromptClass.prompt : RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		};
	});
}

async function nextPaint(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

async function until(predicate: () => boolean): Promise<void> {
	for (let attempt = 0; attempt < 40; attempt += 1) {
		if (predicate()) return;
		await new Promise<void>((resolve) => setTimeout(resolve, 10));
	}
	throw new Error('Terminal Action witness condition did not become observable.');
}
