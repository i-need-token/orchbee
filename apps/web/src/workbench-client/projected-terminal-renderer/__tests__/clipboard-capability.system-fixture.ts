import { ActionRuntime } from '@orchbee/actions';
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

import { createBrowserTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import {
	mountTerminalFindSurface,
	type MountTerminalFindSurfaceOptions,
} from '../../terminal-find-surface';
import {
	mountTerminalTextViewer,
	type MountTerminalTextViewerOptions,
} from '../../terminal-text-viewer';
import {
	installProjectedTerminalShortcuts,
	PROJECTED_TERMINAL_ACTION_IDS,
} from '../../projected-terminal-actions';
import type { ProjectedTerminalTransport } from '../../terminal-projection';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';

const attachmentId = AttachmentIdSchema.parse('a1'.repeat(16));
const creationId = CreationIdSchema.parse('a2'.repeat(16));
const hostEpoch = HostEpochSchema.parse('a3'.repeat(16));
const terminalId = TerminalIdSchema.parse('a4'.repeat(16));
const capabilityRevision = CanonicalUint64TextSchema.parse('1');
const emptyGraphics = {
	animationTick: 0n,
	images: [],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [],
	resourceRevision: 0n,
} as const;
const selectionText = 'LAN Browser selection';
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

const activation = document.createElement('button');
activation.id = 'terminal-clipboard-capability-activation';
activation.textContent = 'Run clipboard capability witness';
const root = document.createElement('main');
root.id = 'terminal-clipboard-capability-witness';
root.style.boxSizing = 'border-box';
root.style.height = '180px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';
document.body.replaceChildren(activation, root);
document.body.style.margin = '0';
Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });
root.dataset['witnessForcedCanvas'] = 'true';

const actionRuntime = new ActionRuntime();
const workbenchScope = actionRuntime.createScope('workbench.fixture.clipboard-capability');
const clipboardWrites: string[] = [];
let clipboardWriterAvailable = true;
let controlledSecureContext = true;
let rejectNextWrite = false;
let selectionCopyCommands = 0;
let terminal: ProjectedTerminal | undefined;
let contextProvider: WorkbenchTerminalActionContextProvider | undefined;
let transportListener: (value: unknown) => void = () => undefined;

const platformEffects = createBrowserTerminalPlatformEffects({
	getClipboardTextWriter: () =>
		clipboardWriterAvailable
			? async (text) => {
					clipboardWrites.push(text);
					if (!rejectNextWrite) return;
					rejectNextWrite = false;
					throw new DOMException('Clipboard permission denied.', 'NotAllowedError');
				}
			: undefined,
	hasTransientUserActivation: () => navigator.userActivation?.isActive === true,
	isSecureContext: () => controlledSecureContext && window.isSecureContext,
	openExternal: () => undefined,
});
const dependencies: ProjectedTerminalRendererDependencies = {
	createTerminal: (options: ProjectedTerminalOptions) => {
		terminal = new ProjectedTerminal(options);
		return terminal;
	},
	installShortcuts: installProjectedTerminalShortcuts,
	mountFindSurface: (options: MountTerminalFindSurfaceOptions) => mountTerminalFindSurface(options),
	mountTextViewer: (options: MountTerminalTextViewerOptions) => mountTerminalTextViewer(options),
};
const transport: ProjectedTerminalTransport = {
	close: () => undefined,
	projectionBufferTrust: 'local-host',
	send: (command) => {
		if (command.kind === 'selection-copy') {
			selectionCopyCommands += 1;
			const content = new TextEncoder().encode(selectionText);
			queueMicrotask(() => {
				transportListener({
					content: content.buffer,
					kind: 'selection-copy-content',
					requestSequence: command.payload.requestSequence,
				});
			});
			return;
		}
		if (command.kind !== 'read-rows') return;
		queueMicrotask(() => {
			transportListener({ kind: 'rows', rows: terminalRows(command) });
		});
	},
	subscribe: (listener) => {
		transportListener = listener;
		return () => undefined;
	},
};

void prepareWitness();

async function prepareWitness(): Promise<void> {
	try {
		const session = await startProjectedTerminalRenderer(
			root,
			document.documentElement,
			['en-US'],
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
			platformEffects,
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
					return { dispose: () => undefined };
				},
				runtime: actionRuntime,
				scope: workbenchScope,
			},
		);
		const activeTerminal = terminal;
		if (activeTerminal === undefined) throw new Error('Clipboard witness terminal is unavailable.');
		transportListener({ frame: terminalSnapshot(), kind: 'frame' });
		activeTerminal.notifySelectionChanged({
			attachmentId,
			end: { column: selectionText.length - 1, row: CanonicalUint64TextSchema.parse('0') },
			preview: selectionText,
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active',
		});
		await nextPaint();

		root.dataset['witnessActualSecureContext'] = String(window.isSecureContext);
		root.dataset['witnessInitialUserActivation'] = String(
			navigator.userActivation?.isActive === true,
		);
		root.dataset['witnessInitialAvailability'] = copyAvailability();
		const initialResult = await executeCopy();
		await until(() => statusText() === 'This terminal action is unavailable.');
		root.dataset['witnessInitialResult'] = resultLabel(initialResult);
		root.dataset['witnessInitialFeedback'] = statusText();
		root.dataset['witnessInitialSelectionCommands'] = String(selectionCopyCommands);
		root.dataset['witnessInitialWrites'] = String(clipboardWrites.length);
		root.dataset['witnessReadCapability'] = String('readClipboardText' in platformEffects);
		root.dataset['witnessSetupReady'] = 'true';

		activation.addEventListener(
			'click',
			() => {
				void runActivatedWitness().finally(() => {
					session.dispose();
					actionRuntime.dispose();
				});
			},
			{ once: true },
		);
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	}
}

async function runActivatedWitness(): Promise<void> {
	try {
		root.dataset['witnessClickUserActivation'] = String(
			navigator.userActivation?.isActive === true,
		);
		root.dataset['witnessActivatedAvailability'] = copyAvailability();
		const completed = await executeCopy();
		await until(() => root.dataset['projectedTerminalCopyState'] === 'completed');
		root.dataset['witnessActivatedResult'] = resultLabel(completed);
		root.dataset['witnessActivatedFeedback'] = statusText();
		root.dataset['witnessActivatedSelectionCommands'] = String(selectionCopyCommands);
		root.dataset['witnessActivatedWrites'] = String(clipboardWrites.length);

		rejectNextWrite = true;
		const commandsBeforeRejection = selectionCopyCommands;
		const writesBeforeRejection = clipboardWrites.length;
		const rejected = await executeCopy();
		await until(
			() =>
				root.dataset['projectedTerminalCopyState'] === 'failed' &&
				root.dataset['projectedTerminalCopyFailure'] === 'browser-clipboard-write-failed',
		);
		root.dataset['witnessRejectedResult'] = resultLabel(rejected);
		root.dataset['witnessRejectedFeedback'] = statusText();
		root.dataset['witnessRejectedCommandDelta'] = String(
			selectionCopyCommands - commandsBeforeRejection,
		);
		root.dataset['witnessRejectedWriteDelta'] = String(
			clipboardWrites.length - writesBeforeRejection,
		);

		clipboardWriterAvailable = false;
		root.dataset['witnessMissingAvailability'] = copyAvailability();
		const commandsBeforeMissing = selectionCopyCommands;
		const writesBeforeMissing = clipboardWrites.length;
		const missing = await executeCopy();
		await until(() => statusText() === 'This terminal action is unavailable.');
		root.dataset['witnessMissingResult'] = resultLabel(missing);
		root.dataset['witnessMissingFeedback'] = statusText();
		root.dataset['witnessMissingCommandDelta'] = String(
			selectionCopyCommands - commandsBeforeMissing,
		);
		root.dataset['witnessMissingWriteDelta'] = String(clipboardWrites.length - writesBeforeMissing);

		clipboardWriterAvailable = true;
		controlledSecureContext = false;
		root.dataset['witnessInsecureAvailability'] = copyAvailability();
		const commandsBeforeInsecure = selectionCopyCommands;
		const writesBeforeInsecure = clipboardWrites.length;
		const insecureAction = await executeCopy();
		const insecureEffect = await platformEffects.writeClipboardText?.('insecure direct write');
		root.dataset['witnessInsecureActionResult'] = resultLabel(insecureAction);
		root.dataset['witnessInsecureEffectResult'] = resultLabel(insecureEffect);
		root.dataset['witnessInsecureFeedback'] = statusText();
		root.dataset['witnessInsecureCommandDelta'] = String(
			selectionCopyCommands - commandsBeforeInsecure,
		);
		root.dataset['witnessInsecureWriteDelta'] = String(
			clipboardWrites.length - writesBeforeInsecure,
		);
		root.dataset['witnessWrites'] = clipboardWrites.join('|');
		root.dataset['witnessReady'] = 'true';
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	}
}

function copyAvailability(): string {
	return (
		actionRuntime.resolveAction(PROJECTED_TERMINAL_ACTION_IDS.copySelection, undefined, {
			scope: workbenchScope,
			source: 'keyboard',
		})?.availability.kind ?? 'missing'
	);
}

async function executeCopy() {
	const provider = contextProvider;
	if (provider === undefined) throw new Error('Clipboard witness Action provider is unavailable.');
	const action = actionRuntime
		.discover({ scope: workbenchScope, source: 'keyboard' })
		.find((result) => result.descriptor.id === PROJECTED_TERMINAL_ACTION_IDS.copySelection);
	if (action?.kind !== 'ready') throw new Error('Clipboard witness Copy Action is unavailable.');
	return provider.executePrepared(action.invocation).completion;
}

function resultLabel(
	result:
		| {
				readonly code?: string;
				readonly failure?: { readonly code: string };
				readonly kind: string;
		  }
		| undefined,
): string {
	if (result === undefined) return 'missing';
	if (result.code !== undefined) return `${result.kind}:${result.code}`;
	if (result.failure !== undefined) return `${result.kind}:${result.failure.code}`;
	return result.kind;
}

function statusText(): string {
	return root.querySelector('[data-projected-terminal-status]')?.textContent ?? '';
}

function terminalRows(
	command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>,
): Uint8Array {
	if (command.payload.origin.kind !== 'absolute') {
		throw new Error('Clipboard witness requested a non-absolute row window.');
	}
	const startRow = BigInt(command.payload.origin.row);
	const requestSequence = BigInt(command.payload.requestSequence);
	const frame = terminalFrame(requestSequence);
	const rows = startRow === 0n ? 1 : 0;
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: frame.columns,
		detail: TerminalRowWindowDetail.render,
		frame: {
			...frame,
			rowData: rows === 0 ? [] : frame.rowData,
			rows,
			scrollbackRows: startRow,
		},
		generation: 1n,
		graphics: emptyGraphics,
		hostEpoch,
		revision: 1n,
		sequence: requestSequence,
		startRow,
		totalRows: 1n,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function terminalSnapshot(): Uint8Array {
	const encoded = encodeTerminalFrame(terminalFrame(1n));
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function terminalFrame(sequence: bigint): TerminalFrame {
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
		columns: 64,
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
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
			alternateScroll: false,
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
			mouseAny: false,
			mouseButton: false,
			mouseNormal: false,
			mouseSgr: false,
			mouseSgrPixels: false,
			mouseUtf8: false,
			mouseUrxvt: false,
			mouseX10: false,
		},
		kind: TerminalFrameKind.snapshot,
		revision: 1n,
		rowData: [
			{
				cells: Array.from({ length: 64 }, (_, column) => ({
					grapheme: selectionText[column] ?? '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				})),
				index: 0,
				promptClass: RowPromptClass.none,
				wrap: false,
				wrapContinuation: false,
			},
		],
		rows: 1,
		scrollbackRows: 0n,
		sequence,
		styles: [regularStyle],
		themeRevision: 0n,
		title: 'LC-05 Browser clipboard capability witness',
	};
}

async function nextPaint(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

async function until(predicate: () => boolean): Promise<void> {
	for (let attempt = 0; attempt < 80; attempt += 1) {
		if (predicate()) return;
		await new Promise<void>((resolve) => setTimeout(resolve, 10));
	}
	throw new Error('Clipboard capability witness condition did not become observable.');
}
