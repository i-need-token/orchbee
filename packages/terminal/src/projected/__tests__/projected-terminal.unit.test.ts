import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	encodeTerminalRowWindow,
	encodeTerminalFrame,
	MouseAction,
	MouseButton,
	parseAttachmentId,
	parseCreationId,
	parseHostEpoch,
	parseTerminalId,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalRowWindowDetail,
	UnderlineKind,
	type TerminalFrame,
	type TerminalExited,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	ProjectedHistoryPhase,
	ProjectedTerminal,
	type ProjectedTerminalCommand,
	type ProjectedTerminalErrorEvent,
} from '../projected-terminal';

interface FakeElement {
	readonly attributes: Map<string, string>;
	readonly capturedPointers: Set<number>;
	readonly children: FakeElement[];
	readonly dataset: DOMStringMap;
	readonly listeners: Map<string, Set<EventListener>>;
	clientHeight: number;
	clientWidth: number;
	height: number;
	parentElement: FakeElement | null;
	style: Record<string, string>;
	textContent: string | null;
	value: string;
	width: number;
	appendChild(child: FakeElement): FakeElement;
	dispatch(type: string, event: Event): void;
	releasePointerCapture(pointerId: number): void;
	remove(): void;
	setPointerCapture(pointerId: number): void;
}

interface FakeBrowser {
	readonly document: Document;
	readonly frames: Map<number, FrameRequestCallback>;
	readonly mediaQueries: FakeMediaQueryList[];
	readonly root: HTMLElement;
	dispatchWindow(type: string): void;
	flushFrames(): void;
	setDevicePixelRatio(value: number): void;
}

interface FakeMediaQueryList {
	readonly media: string;
	dispatchChange(): void;
}

const interaction = {
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
};

function identifier<T>(
	result: { readonly ok: true; readonly value: T } | { readonly ok: false },
): T {
	if (!result.ok) throw new Error('test identifier is invalid');
	return result.value;
}

const hostEpoch = identifier(parseHostEpoch('11111111111111111111111111111111'));
const attachmentId = identifier(parseAttachmentId('22222222222222222222222222222222'));
const creationId = identifier(parseCreationId('33333333333333333333333333333333'));
const otherCreationId = identifier(parseCreationId('55555555555555555555555555555555'));
const terminalId = identifier(parseTerminalId('44444444444444444444444444444444'));

afterEach(() => {
	vi.unstubAllGlobals();
});

function terminalFrame(
	kind: 'delta' | 'snapshot',
	sequence: bigint,
	revision: bigint,
	text: readonly [string, string],
	baseRevision: bigint | null = null,
	rowIndices: readonly number[] = [0, 1],
	scrollbackRows = 0n,
	frameInteraction: TerminalFrame['interaction'] = interaction,
	cursor: { readonly column: number; readonly row: number } = { column: 0, row: 0 },
): Uint8Array {
	const frame: TerminalFrame = {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 2,
		cursor: {
			blinking: false,
			column: cursor.column,
			row: cursor.row,
			shape: CursorShape.block,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/tmp',
		defaultColors: { backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
		extensions: [],
		generation: 1n,
		hostEpoch,
		interaction: frameInteraction,
		kind: kind === 'snapshot' ? TerminalFrameKind.snapshot : TerminalFrameKind.delta,
		revision,
		rowData: rowIndices.map((index) => ({
			cells: [
				{
					grapheme: text[index] ?? '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
				{
					grapheme: '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
			],
			index,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows: 2,
		scrollbackRows,
		sequence,
		styles: [
			{
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
			},
		],
		title: 'test',
	};
	const encoded = encodeTerminalFrame(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function renderRowWindow(
	sequence: bigint,
	revision: bigint,
	startRow: bigint,
	totalRows: bigint,
	text: readonly string[],
): Uint8Array {
	const decodedFrame = {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 2,
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
			shape: CursorShape.block,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/tmp',
		defaultColors: { backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
		extensions: [],
		generation: 1n,
		hostEpoch,
		interaction,
		kind: TerminalFrameKind.snapshot,
		revision,
		rowData: text.map((value, index) => ({
			cells: [
				{
					grapheme: value,
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
				{
					grapheme: '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
			],
			index,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows: text.length,
		scrollbackRows: startRow,
		sequence,
		styles: [
			{
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
			},
		],
		title: 'history',
	};
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: 2,
		detail: TerminalRowWindowDetail.render,
		frame: decodedFrame,
		generation: 1n,
		hostEpoch,
		revision,
		sequence,
		startRow,
		totalRows,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function createContext(): CanvasRenderingContext2D {
	return {
		beginPath: () => {},
		clearRect: () => {},
		clip: () => {},
		drawImage: () => {},
		fillRect: () => {},
		fillText: () => {},
		font: '10px sans-serif',
		getImageData: (_x: number, _y: number, width: number, height: number) =>
			({ data: new Uint8ClampedArray(width * height * 4) }) as ImageData,
		globalAlpha: 1,
		imageSmoothingEnabled: false,
		lineTo: () => {},
		measureText: () =>
			({
				actualBoundingBoxAscent: 9,
				actualBoundingBoxDescent: 3,
				fontBoundingBoxAscent: 12,
				fontBoundingBoxDescent: 4,
				width: 8,
			}) as TextMetrics,
		moveTo: () => {},
		rect: () => {},
		restore: () => {},
		save: () => {},
		scale: () => {},
		setLineDash: () => {},
		setTransform: () => {},
		stroke: () => {},
		translate: () => {},
	} as unknown as CanvasRenderingContext2D;
}

function createFakeBrowser(): FakeBrowser {
	let nextFrame = 1;
	const frames = new Map<number, FrameRequestCallback>();
	const mediaQueries: FakeMediaQueryList[] = [];
	const windowListeners = new Map<string, Set<EventListener>>();
	const context = createContext();
	const createElement = (tagName: string): FakeElement => {
		const listeners = new Map<string, Set<EventListener>>();
		const element: FakeElement & Record<string, unknown> = {
			attributes: new Map(),
			capturedPointers: new Set<number>(),
			children: [],
			clientHeight: 360,
			clientWidth: 720,
			height: 0,
			dataset: {},
			listeners,
			ownerDocument: undefined,
			parentElement: null,
			style: {},
			textContent: null,
			value: '',
			width: 0,
			addEventListener: (type: string, listener: EventListener) => {
				const entries = listeners.get(type) ?? new Set<EventListener>();
				entries.add(listener);
				listeners.set(type, entries);
			},
			appendChild: (child: FakeElement) => {
				child.parentElement = element;
				element.children.push(child);
				return child;
			},
			dispatch: (type: string, event: Event) => {
				for (const listener of listeners.get(type) ?? []) listener(event);
			},
			focus: () => element.dispatch('focus', { type: 'focus' } as Event),
			getBoundingClientRect: () => ({
				bottom: 360,
				height: 360,
				left: 0,
				right: 720,
				top: 0,
				width: 720,
				x: 0,
				y: 0,
				toJSON: () => ({}),
			}),
			getContext: tagName === 'canvas' ? () => context : undefined,
			remove: () => {
				const parent = element.parentElement;
				if (parent) parent.children.splice(parent.children.indexOf(element), 1);
				element.parentElement = null;
			},
			releasePointerCapture: (pointerId: number) => element.capturedPointers.delete(pointerId),
			removeEventListener: (type: string, listener: EventListener) => {
				listeners.get(type)?.delete(listener);
			},
			setPointerCapture: (pointerId: number) => element.capturedPointers.add(pointerId),
			setAttribute: (name: string, value: string) => element.attributes.set(name, value),
		};
		Object.defineProperty(element, 'ownerDocument', { get: () => document });
		return element;
	};

	const defaultView = {
		addEventListener: (type: string, listener: EventListener) => {
			const entries = windowListeners.get(type) ?? new Set<EventListener>();
			entries.add(listener);
			windowListeners.set(type, entries);
		},
		cancelAnimationFrame: (handle: number) => frames.delete(handle),
		devicePixelRatio: 1,
		getComputedStyle: () => ({
			paddingBottom: '0px',
			paddingLeft: '0px',
			paddingRight: '0px',
			paddingTop: '0px',
		}),
		matchMedia: (media: string) => {
			const listeners = new Set<EventListener>();
			const query: FakeMediaQueryList = {
				media,
				dispatchChange: () => {
					for (const listener of listeners) listener({ type: 'change' } as Event);
				},
			};
			mediaQueries.push(query);
			return {
				addEventListener: (_type: string, listener: EventListener) => listeners.add(listener),
				matches: true,
				media,
				removeEventListener: (_type: string, listener: EventListener) => listeners.delete(listener),
			};
		},
		removeEventListener: (type: string, listener: EventListener) => {
			windowListeners.get(type)?.delete(listener);
		},
		requestAnimationFrame: (callback: FrameRequestCallback) => {
			const handle = nextFrame++;
			frames.set(handle, callback);
			return handle;
		},
	};
	const document = {
		addEventListener: () => undefined,
		createElement,
		defaultView,
		getSelection: () => null,
		removeEventListener: () => undefined,
	} as unknown as Document;
	const root = createElement('div') as unknown as HTMLElement;
	return {
		document,
		dispatchWindow: (type) => {
			for (const listener of windowListeners.get(type) ?? []) listener({ type } as Event);
		},
		frames,
		mediaQueries,
		root,
		flushFrames: () => {
			const callbacks = [...frames.values()];
			frames.clear();
			for (const callback of callbacks) callback(16);
		},
		setDevicePixelRatio: (value) => {
			defaultView.devicePixelRatio = value;
		},
	};
}

function exitEvent(finalRevision: string): TerminalExited {
	return {
		exit: { code: 0, kind: 'exit-code' },
		finalRevision: finalRevision as TerminalExited['finalRevision'],
		identity: { creationId, hostEpoch, terminalId },
	};
}

describe('ProjectedTerminal', () => {
	it('waits for the configured font and keeps the IME textarea font in sync', async () => {
		const browser = createFakeBrowser();
		let finishFontLoad: (() => void) | undefined;
		const fontLoad = vi.fn(
			() =>
				new Promise<FontFace[]>((resolve) => {
					finishFontLoad = () => resolve([]);
				}),
		);
		Object.defineProperty(browser.document, 'fonts', {
			configurable: true,
			value: { load: fontLoad },
		});
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			renderer: { fontFamily: '"Menlo", monospace', fontSize: 17, fontWeight: 500 },
			terminalId,
		});

		const opening = terminal.open(browser.root);
		await vi.waitFor(() => expect(fontLoad).toHaveBeenCalledOnce());
		expect(fontLoad).toHaveBeenCalledWith('500 17px "Menlo", monospace', 'MW你🚀');
		expect(commands).toEqual([]);
		finishFontLoad?.();
		await opening;

		expect(terminal.textarea?.style.fontFamily).toBe('"Menlo", monospace');
		expect(terminal.textarea?.style.fontSize).toBe('17px');
		expect(terminal.textarea?.style.fontWeight).toBe('500');
		terminal.dispose();
	});

	it('anchors the native IME candidate window to the projected cursor', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => {} },
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame(
				'snapshot',
				1n,
				1n,
				['a', 'b'],
				null,
				[0, 1],
				0n,
				interaction,
				{ column: 1, row: 1 },
			),
		);
		const input = terminal.textarea as unknown as FakeElement & {
			selectionEnd: number;
			selectionStart: number;
		};
		input.selectionStart = 1;
		input.selectionEnd = 1;

		input.dispatch('compositionstart', {
			data: '你',
			type: 'compositionstart',
		} as unknown as CompositionEvent);

		expect(input.style.left).toBe('4px');
		expect(input.style.top).toBe('16px');
		expect(input.style.width).toBe('16px');
		expect(input.style.height).toBe('16px');
		expect(input.style.lineHeight).toBe('16px');
		terminal.dispose();
	});

	it('activates only after opening, acknowledges after scheduling, and renders packed rows', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'Localized input', screen: 'Localized screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		const telemetry = vi.fn<(event: unknown) => void>();
		terminal.onFrameTelemetry(telemetry);

		expect(commands).toEqual([]);
		await terminal.open(browser.root);
		expect(commands).toEqual([
			{
				kind: 'set-projection-demand',
				payload: { attachmentId, demand: 'active' },
			},
		]);

		const snapshot = terminalFrame('snapshot', 1n, 1n, ['a', 'b']);
		expect(terminal.applyFrame(snapshot)).toBe('committed');
		expect(browser.frames.size).toBe(1);
		expect(commands.at(-1)).toEqual({
			kind: 'frame-applied',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '1' },
		});
		expect(telemetry).not.toHaveBeenCalled();
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				backend: 'canvas2d',
				cellsVisited: 4,
				revision: '1',
				rowsRendered: 2,
			}),
		);
		expect('write' in terminal).toBe(false);

		expect(terminal.applyFrame(snapshot)).toBe('duplicate');
		expect(commands.at(-1)).toEqual({
			kind: 'frame-applied',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '1' },
		});
		terminal.dispose();
	});

	it('requests recovery without mutating the committed grid and delays exit until final revision', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: {
				historyRange: (start, end, total) => `${start}-${end}/${total}`,
				input: 'input',
				screen: 'screen',
			},
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		const exited = vi.fn<(event: TerminalExited) => void>();
		terminal.onExit(exited);
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		terminal.notifyExited(exitEvent('2'));
		expect(exited).not.toHaveBeenCalled();

		expect(terminal.applyFrame(terminalFrame('delta', 2n, 3n, ['x', ''], 2n, [0]))).toBe(
			'snapshot-required',
		);
		expect(commands.at(-1)).toEqual({
			kind: 'need-snapshot',
			payload: { attachmentId, reason: 'base-mismatch', sequence: '2' },
		});
		expect(terminal.metadata?.revision).toBe(1n);

		terminal.applyFrame(terminalFrame('snapshot', 2n, 2n, ['x', 'b']));
		expect(exited).toHaveBeenCalledWith(exitEvent('2'));
		terminal.notifyExited(exitEvent('1'));
		terminal.notifyExited(exitEvent('3'));
		expect(exited).toHaveBeenCalledTimes(1);
		terminal.applyFrame(terminalFrame('snapshot', 3n, 3n, ['x', 'b']));
		expect(exited).toHaveBeenCalledTimes(2);
		expect(exited).toHaveBeenLastCalledWith(exitEvent('3'));
		terminal.dispose();
	});

	it('commits one bounded history window and keeps it visible until a fresh bottom snapshot', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: {
				historyRange: (start, end, total) => `${start}-${end}/${total}`,
				input: 'input',
				screen: 'screen',
			},
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 10n));
		const accessibilityRoot = (browser.root as unknown as FakeElement).children[2];
		expect(accessibilityRoot?.attributes.get('aria-live')).toBe('polite');
		expect(accessibilityRoot?.attributes.get('aria-description')).toBe('11-12/12');
		expect(accessibilityRoot?.children[0]?.attributes.get('data-terminal-absolute-row')).toBe('10');

		terminal.scrollLines(-1);

		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.requestingHistory);
		expect(commands.slice(-3)).toEqual([
			{
				kind: 'set-history-demand',
				payload: { attachmentId, demand: 'viewing-history' },
			},
			{
				kind: 'set-projection-demand',
				payload: { attachmentId, demand: 'paused' },
			},
			expect.objectContaining({
				kind: 'read-rows',
				payload: expect.objectContaining({
					origin: { kind: 'absolute', row: '0' },
					purpose: 'viewport',
					rowCount: 12,
				}),
			}),
		]);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('history request was not sent');
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(read.payload.requestSequence),
					1n,
					0n,
					12n,
					Array.from({ length: 12 }, (_, index) => {
						if (index === 9) return 'history-a';
						if (index === 10) return 'history-b';
						return `overscan-${index}`;
					}),
				),
			),
		).toBe('committed');
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.viewingHistory);
		expect(accessibilityRoot?.attributes.get('aria-live')).toBe('off');
		expect(accessibilityRoot?.children[0]?.attributes.get('data-terminal-absolute-row')).toBe('9');
		expect(commands.at(-1)).toEqual({
			kind: 'rows-applied',
			payload: {
				attachmentId,
				generation: '1',
				requestSequence: read.payload.requestSequence,
				revision: '1',
			},
		});

		terminal.scrollToBottom();
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.recovering);
		expect(terminal.metadata?.title).toBe('history');
		expect(commands.slice(-2)).toEqual([
			{
				kind: 'set-history-demand',
				payload: { attachmentId, demand: 'following-bottom' },
			},
			{
				kind: 'set-projection-demand',
				payload: { attachmentId, demand: 'active' },
			},
		]);

		terminal.applyFrame(terminalFrame('snapshot', 2n, 2n, ['new-a', 'new-b'], null, [0, 1], 10n));
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.followingBottom);
		expect(terminal.metadata?.title).toBe('test');
		terminal.dispose();
	});

	it('retries an oversized history overscan with the exact visible viewport', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		terminal.onError(errors);
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));

		terminal.scrollLines(-1);
		const overscanned = commands.at(-1);
		if (overscanned?.kind !== 'read-rows') throw new Error('history request was not sent');
		expect(overscanned.payload.rowCount).toBe(12);

		terminal.notifyBrokerFailure({
			code: 'limit-exceeded',
			message: 'overscan exceeds the negotiated row-window limit',
			operation: 'history',
			retryable: true,
		});

		expect(commands.at(-1)).toEqual(
			expect.objectContaining({
				kind: 'read-rows',
				payload: expect.objectContaining({
					origin: { kind: 'absolute', row: '9' },
					rowCount: 2,
				}),
			}),
		);
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.requestingHistory);
		expect(errors).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('routes only locally approved paste text as semantic input', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const reviewPaste = vi.fn<(review: unknown) => boolean>(() => false);
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			reviewPaste,
			terminalId,
		});
		await terminal.open(browser.root);

		await expect(terminal.paste('safe')).resolves.toEqual(
			expect.objectContaining({ kind: 'sent' }),
		);
		expect(commands.at(-1)).toEqual({
			kind: 'paste-input',
			payload: { attachmentId, text: 'safe' },
		});
		await expect(terminal.paste('one\ntwo')).resolves.toEqual(
			expect.objectContaining({ kind: 'blocked' }),
		);
		expect(reviewPaste).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('keeps observed key input behind an asynchronous paste decision', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let finishReview: ((approved: boolean) => void) | undefined;
		const reviewPaste = vi.fn(
			() =>
				new Promise<boolean>((resolve) => {
					finishReview = resolve;
				}),
		);
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			reviewPaste,
			terminalId,
		});
		await terminal.open(browser.root);

		const paste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(reviewPaste).toHaveBeenCalledOnce());
		(terminal.textarea as unknown as FakeElement).dispatch('keydown', {
			altKey: false,
			code: 'KeyA',
			ctrlKey: false,
			getModifierState: () => false,
			isComposing: false,
			key: 'a',
			keyCode: 65,
			metaKey: false,
			preventDefault: () => {},
			repeat: false,
			shiftKey: false,
			stopPropagation: () => {},
			type: 'keydown',
		} as unknown as KeyboardEvent);

		expect(
			commands.filter((command) => command.kind === 'key-input' || command.kind === 'paste-input'),
		).toEqual([]);
		finishReview?.(true);
		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'sent' }));
		await vi.waitFor(() =>
			expect(
				commands
					.filter(
						(command) => command.kind === 'key-input' || command.kind === 'paste-input',
					)
					.map((command) => command.kind),
			).toEqual(['paste-input', 'key-input']),
		);
		terminal.dispose();
	});

	it('rechecks terminal lifetime after an asynchronous paste decision', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let finishReview: ((approved: boolean) => void) | undefined;
		const reviewPaste = vi.fn(
			() =>
				new Promise<boolean>((resolve) => {
					finishReview = resolve;
				}),
		);
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			reviewPaste,
			terminalId,
		});
		await terminal.open(browser.root);

		const paste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(reviewPaste).toHaveBeenCalledOnce());
		terminal.dispose();
		finishReview?.(true);

		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
		expect(commands.some((command) => command.kind === 'paste-input')).toBe(false);
	});

	it('releases pressed keys before reporting focus loss', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', {
			altKey: false,
			code: 'KeyA',
			ctrlKey: false,
			getModifierState: () => false,
			isComposing: false,
			key: 'a',
			keyCode: 65,
			metaKey: false,
			preventDefault: () => {},
			repeat: false,
			shiftKey: false,
			stopPropagation: () => {},
			type: 'keydown',
		} as unknown as KeyboardEvent);
		input.dispatch('blur', { type: 'blur' } as FocusEvent);

		expect(
			commands
				.filter((command) => command.kind === 'key-input' || command.kind === 'focus-input')
				.map((command) =>
					command.kind === 'key-input' ? `${command.kind}:${command.payload.action}` : command.kind,
				),
		).toEqual(['key-input:press', 'key-input:release', 'focus-input']);
		terminal.dispose();
	});

	it('releases application-owned mouse buttons on focus loss', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseButton: true,
			}),
		);
		(terminal.canvas as unknown as FakeElement).dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			metaKey: false,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);
		(terminal.textarea as unknown as FakeElement).dispatch('blur', {
			type: 'blur',
		} as FocusEvent);

		const mouse = commands.filter((command) => command.kind === 'mouse-input');
		expect(mouse.map((command) => command.payload.action)).toEqual([
			MouseAction.press,
			MouseAction.release,
		]);
		expect(mouse.at(-1)?.payload).toMatchObject({
			button: MouseButton.left,
			pressedButtons: 0,
		});
		terminal.dispose();
	});

	it('keeps local selection available when PTY input is read-only', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			inputEnabled: false,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 7,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);

		expect(commands.at(-1)).toEqual({
			kind: 'selection-gesture',
			payload: expect.objectContaining({ action: 'start', attachmentId }),
		});
		expect(canvas.capturedPointers.has(7)).toBe(true);
		expect(commands.some((command) => command.kind === 'focus-input')).toBe(false);
		expect(commands.some((command) => command.kind === 'mouse-input')).toBe(false);

		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		canvas.dispatch('pointerup', {
			altKey: false,
			button: 0,
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 7,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerup',
		} as unknown as PointerEvent);
		browser.flushFrames();

		expect(commands.at(-1)).toEqual({
			kind: 'selection-gesture',
			payload: expect.objectContaining({ action: 'end', attachmentId }),
		});
		expect(canvas.capturedPointers.has(7)).toBe(false);
		terminal.dispose();
	});

	it('ends an active selection when pointer capture is lost', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 9,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);
		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		canvas.dispatch('pointermove', {
			altKey: false,
			buttons: 1,
			clientX: 12,
			clientY: 2,
			ctrlKey: false,
			metaKey: false,
			pointerId: 9,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		browser.flushFrames();
		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		canvas.capturedPointers.delete(9);
		canvas.dispatch('lostpointercapture', {
			pointerId: 9,
			type: 'lostpointercapture',
		} as unknown as PointerEvent);
		browser.flushFrames();

		const gestures = commands.filter((command) => command.kind === 'selection-gesture');
		expect(gestures.map((command) => command.payload.action)).toEqual(['start', 'update', 'end']);
		terminal.dispose();
	});

	it('identifies the active button on mouse drag motion', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseButton: true,
			}),
		);

		const preventDefault = vi.fn<() => void>();
		(terminal.canvas as unknown as FakeElement).dispatch('pointermove', {
			altKey: false,
			buttons: 1,
			clientX: 8,
			clientY: 18,
			ctrlKey: false,
			metaKey: false,
			preventDefault,
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);

		expect(commands.at(-1)).toEqual({
			kind: 'mouse-input',
			payload: expect.objectContaining({
				action: MouseAction.motion,
				attachmentId,
				button: MouseButton.left,
				pressedButtons: 1,
			}),
		});
		expect(preventDefault).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('rejects a terminal exit from a different creation identity', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => {} },
			terminalId,
		});
		const errors = vi.fn<(event: unknown) => void>();
		const exited = vi.fn<(event: TerminalExited) => void>();
		terminal.onError(errors);
		terminal.onExit(exited);
		await terminal.open(browser.root);

		terminal.notifyExited({
			...exitEvent('0'),
			identity: { creationId: otherCreationId, hostEpoch, terminalId },
		});

		expect(errors).toHaveBeenCalledOnce();
		expect(exited).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('correlates Main-owned copy and link effects without exposing their payloads', async () => {
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});

		const copy = terminal.copySelectionAsync();
		const copyCommand = commands.at(-1);
		if (copyCommand?.kind !== 'selection-copy') throw new Error('copy command missing');
		terminal.notifySelectionCopied(8, copyCommand.payload.requestSequence);
		await expect(copy).resolves.toEqual({ byteLength: 8, kind: 'completed' });

		const link = terminal.openLinkAsync('https://orchbee.dev');
		const linkCommand = commands.at(-1);
		if (linkCommand?.kind !== 'link-open') throw new Error('link command missing');
		terminal.notifyBrokerFailure({
			code: 'link-policy-blocked',
			message: 'blocked',
			operation: 'link',
			requestSequence: linkCommand.payload.requestSequence,
			retryable: false,
		});
		await expect(link).resolves.toEqual({
			code: 'link-policy-blocked',
			kind: 'failed',
			retryable: false,
		});
		terminal.dispose();
	});

	it('caps resize geometry to the PTY total-pixel bounds', async () => {
		const browser = createFakeBrowser();
		const root = browser.root as unknown as FakeElement;
		root.clientWidth = 1_000_000;
		root.clientHeight = 1_000_000;
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			canResize: true,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			renderer: { devicePixelRatio: 512 },
			terminalId,
		});

		await terminal.open(browser.root);

		expect(commands.at(-1)).toEqual({
			kind: 'resize-terminal',
			payload: {
				attachmentId,
				cellPixelHeight: 4_096,
				cellPixelWidth: 4_096,
				columns: 15,
				rows: 15,
			},
		});
		terminal.dispose();
	});

	it('updates renderer and PTY pixel geometry when the device pixel ratio changes', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			canResize: true,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			terminalId,
		});

		await terminal.open(browser.root);
		const initialResize = commands.findLast((command) => command.kind === 'resize-terminal');
		if (initialResize?.kind !== 'resize-terminal') throw new Error('initial resize is missing');
		expect(browser.mediaQueries.at(-1)?.media).toBe('(resolution: 1dppx)');

		browser.setDevicePixelRatio(2);
		browser.mediaQueries.at(-1)?.dispatchChange();

		const resizeCommands = commands.filter((command) => command.kind === 'resize-terminal');
		expect(resizeCommands).toHaveLength(2);
		const changedResize = resizeCommands.at(-1);
		if (changedResize?.kind !== 'resize-terminal') throw new Error('DPR resize is missing');
		expect(changedResize.payload).toEqual({
			...initialResize.payload,
			cellPixelHeight: initialResize.payload.cellPixelHeight * 2,
			cellPixelWidth: initialResize.payload.cellPixelWidth * 2,
		});
		expect(browser.mediaQueries.at(-1)?.media).toBe('(resolution: 2dppx)');
		terminal.dispose();

		browser.setDevicePixelRatio(3);
		browser.dispatchWindow('resize');
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(2);
	});
});
