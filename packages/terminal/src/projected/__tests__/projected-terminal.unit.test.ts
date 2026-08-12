import {
	ActiveScreen,
	CanonicalUint64TextSchema,
	type CanonicalUint64Text,
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
	PROTOCOL_LIMITS,
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

import type { TerminalBufferRow } from '../../buffer-rows';
import { WebGPURenderer } from '../../rendering';
import type { ProjectedSearchWorkerLike } from '../../search';
import {
	ProjectedHistoryPhase,
	ProjectedTerminal,
	type ProjectedTerminalCommand,
	type ProjectedTerminalErrorEvent,
	type ProjectedTerminalFrameTelemetry,
} from '../projected-terminal';

interface FakeElement {
	readonly attributes: Map<string, string>;
	readonly capturedPointers: Set<number>;
	readonly children: FakeElement[];
	readonly dataset: DOMStringMap;
	readonly firstChild: Node | null;
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
	closest(selector: string): FakeElement | null;
	contains(node: unknown): boolean;
	dispatch(type: string, event: Event): void;
	getAttribute(name: string): string | null;
	releasePointerCapture(pointerId: number): void;
	remove(): void;
	setPointerCapture(pointerId: number): void;
}

interface FakeBrowser {
	readonly document: Document;
	readonly frames: Map<number, FrameRequestCallback>;
	readonly mediaQueries: FakeMediaQueryList[];
	readonly root: HTMLElement;
	dispatchDocument(type: string): void;
	dispatchWindow(type: string): void;
	flushFrames(): void;
	setDevicePixelRatio(value: number): void;
	setVisibilityState(value: DocumentVisibilityState): void;
	selectAccessibilityText(
		startRow: number,
		startOffset: number,
		endRow: number,
		endOffset: number,
	): void;
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

function canonicalText(value: string): CanonicalUint64Text {
	return CanonicalUint64TextSchema.parse(value);
}

const hostEpoch = identifier(parseHostEpoch('11111111111111111111111111111111'));
const attachmentId = identifier(parseAttachmentId('22222222222222222222222222222222'));
const creationId = identifier(parseCreationId('33333333333333333333333333333333'));
const otherCreationId = identifier(parseCreationId('55555555555555555555555555555555'));
const terminalId = identifier(parseTerminalId('44444444444444444444444444444444'));
const otherAttachmentId = identifier(parseAttachmentId('66666666666666666666666666666666'));

afterEach(() => {
	vi.useRealTimers();
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
	generation = 1n,
	columns = 2,
): Uint8Array {
	const frame: TerminalFrame = {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns,
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
		generation,
		hostEpoch,
		interaction: frameInteraction,
		kind: kind === 'snapshot' ? TerminalFrameKind.snapshot : TerminalFrameKind.delta,
		revision,
		rowData: rowIndices.map((index) => ({
			cells: Array.from({ length: columns }, (_, column) =>
				column === 0
					? {
							grapheme: text[index] ?? '',
							protected: false,
							semanticContent: SemanticContent.output,
							style: 0,
							width: CellWidth.narrow,
						}
					: {
							grapheme: '',
							protected: false,
							semanticContent: SemanticContent.output,
							style: 0,
							width: CellWidth.narrow,
						},
			),
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
	options: { readonly hyperlink?: string; readonly hyperlinkRowIndex?: number } = {},
): Uint8Array {
	const defaultStyle: TerminalFrame['styles'][number] = {
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
					style:
						options.hyperlink === undefined || index !== (options.hyperlinkRowIndex ?? 0) ? 0 : 1,
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
		styles:
			options.hyperlink === undefined
				? [defaultStyle]
				: [defaultStyle, { ...defaultStyle, hyperlink: options.hyperlink }],
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

function searchWorkerWithMatch(row: number): ProjectedSearchWorkerLike {
	type SearchWorkerRequest = Parameters<ProjectedSearchWorkerLike['postMessage']>[0];
	const messageListeners: Array<(event: { readonly data: unknown }) => void> = [];
	let start: Extract<SearchWorkerRequest, { kind: 'start' }> | undefined;
	const worker = {
		addEventListener: (type: string, listener: (event: { readonly data: unknown }) => void) => {
			if (type === 'message') messageListeners.push(listener);
		},
		postMessage: (message: SearchWorkerRequest) => {
			if (message.kind === 'start') {
				start = message;
				return;
			}
			if (!message.final || start === undefined) return;
			const response = {
				generation: start.generation,
				kind: 'result',
				matches: [
					{
						end: { column: 6, row },
						start: { column: 0, row },
						text: 'needle',
					},
				],
				revision: start.revision,
				searchId: start.searchId,
			};
			for (const listener of messageListeners) {
				listener({ data: response });
			}
		},
		terminate: () => {},
	};
	return worker as unknown as ProjectedSearchWorkerLike;
}

/**
 * Hand-assembles a one-row text-detail row window whose single mapped cell spans
 * `textLength` narrow ASCII bytes, so tests can drive the strict decoder with a
 * corpus the encoder refuses to emit after the per-cell bound is enforced.
 */
function textRowWindow(sequence: bigint, textLength: number): Uint8Array {
	const bodyLength = 12 + textLength + 12;
	const total = 112 + bodyLength;
	const bytes = new Uint8Array(total);
	const view = new DataView(bytes.buffer);
	bytes.set([0x4f, 0x42, 0x52, 0x57], 0); // OBRW
	bytes[4] = 0;
	bytes[5] = 0;
	bytes[6] = 2; // text detail
	bytes[7] = 0;
	view.setUint32(8, total, true);
	view.setUint16(12, 112, true);
	view.setUint16(14, 1, true); // columns
	bytes.fill(0x11, 16, 32); // host epoch
	bytes.fill(0x22, 32, 48); // attachment id
	view.setBigUint64(48, sequence, true);
	view.setBigUint64(56, 42n, true); // revision
	view.setBigUint64(64, 1n, true); // generation
	view.setBigUint64(72, 12n, true); // total rows
	view.setBigUint64(80, 0n, true); // start row
	view.setUint16(88, 1, true); // row count
	view.setUint32(92, 1, true); // entry count
	view.setUint32(96, bodyLength, true);
	bytes[100] = 1; // format patch
	bytes[112] = 0; // row flags: no wrap, not identity-ascii
	bytes[113] = 0; // prompt class: none
	view.setUint16(114, 0, true);
	view.setUint32(116, textLength, true);
	view.setUint32(120, 1, true);
	bytes.fill(0x61, 124, 124 + textLength); // narrow ASCII text
	view.setUint32(124 + textLength, 0, true); // byte offset
	view.setUint32(124 + textLength + 4, 0, true); // UTF-16 offset
	view.setUint16(124 + textLength + 8, 0, true); // column
	bytes[124 + textLength + 10] = 1; // width
	bytes[124 + textLength + 11] = 0; // reserved
	return bytes;
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
	const documentListeners = new Map<string, Set<EventListener>>();
	interface FakeRange {
		endContainer: Node;
		endOffset: number;
		setEnd(node: Node, offset: number): void;
		setStart(node: Node, offset: number): void;
		startContainer: Node;
		startOffset: number;
	}
	let selectedRange: FakeRange | undefined;
	const domSelection = {
		addRange: (range: Range) => {
			selectedRange = range as unknown as FakeRange;
		},
		get anchorNode() {
			return selectedRange?.startContainer ?? null;
		},
		get anchorOffset() {
			return selectedRange?.startOffset ?? 0;
		},
		get focusNode() {
			return selectedRange?.endContainer ?? null;
		},
		get focusOffset() {
			return selectedRange?.endOffset ?? 0;
		},
		get isCollapsed() {
			return (
				selectedRange === undefined ||
				(selectedRange.startContainer === selectedRange.endContainer &&
					selectedRange.startOffset === selectedRange.endOffset)
			);
		},
		get rangeCount() {
			return selectedRange === undefined ? 0 : 1;
		},
		getRangeAt: () => {
			if (selectedRange === undefined) throw new Error('selection range is unavailable');
			return selectedRange as unknown as Range;
		},
		removeAllRanges: () => {
			selectedRange = undefined;
		},
	} as unknown as Selection;
	let selection: Selection | null = domSelection;
	let visibilityState: DocumentVisibilityState = 'visible';
	const context = createContext();
	const createElement = (tagName: string): FakeElement => {
		const listeners = new Map<string, Set<EventListener>>();
		const element: FakeElement & Record<string, unknown> = {
			attributes: new Map(),
			capturedPointers: new Set<number>(),
			children: [],
			clientHeight: 360,
			clientWidth: 720,
			get firstChild() {
				return element.textContent === null
					? null
					: ({ nodeType: 3, parentElement: element } as unknown as Node);
			},
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
			closest: (selector: string) => {
				if (
					selector === '[data-terminal-absolute-row]' &&
					element.attributes.has('data-terminal-absolute-row')
				) {
					return element;
				}
				return element.parentElement?.closest(selector) ?? null;
			},
			contains: (node: unknown) => {
				if (node === element) return true;
				if (
					typeof node === 'object' &&
					node !== null &&
					'parentElement' in node &&
					(node as { readonly parentElement?: unknown }).parentElement === element
				) {
					return true;
				}
				return element.children.some((child) => child.contains(node));
			},
			dispatch: (type: string, event: Event) => {
				for (const listener of listeners.get(type) ?? []) {
					listener(event);
					if (event.cancelBubble) return;
				}
				if (event.bubbles) element.parentElement?.dispatch(type, event);
			},
			focus: () => element.dispatch('focus', { type: 'focus' } as Event),
			getAttribute: (name: string) => element.attributes.get(name) ?? null,
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
		getComputedStyle: (target: Element) => {
			const style = (target as unknown as FakeElement).style;
			return {
				paddingBottom: style['paddingBottom'] ?? '0px',
				paddingLeft: style['paddingLeft'] ?? '0px',
				paddingRight: style['paddingRight'] ?? '0px',
				paddingTop: style['paddingTop'] ?? '0px',
			};
		},
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
		addEventListener: (type: string, listener: EventListener) => {
			const entries = documentListeners.get(type) ?? new Set<EventListener>();
			entries.add(listener);
			documentListeners.set(type, entries);
		},
		createElement,
		createRange: () => {
			const range: FakeRange = {
				endContainer: root as unknown as Node,
				endOffset: 0,
				setEnd: (node, offset) => {
					range.endContainer = node;
					range.endOffset = offset;
				},
				setStart: (node, offset) => {
					range.startContainer = node;
					range.startOffset = offset;
				},
				startContainer: root as unknown as Node,
				startOffset: 0,
			};
			return range as unknown as Range;
		},
		defaultView,
		getSelection: () => selection,
		get visibilityState() {
			return visibilityState;
		},
		removeEventListener: (type: string, listener: EventListener) => {
			documentListeners.get(type)?.delete(listener);
		},
	} as unknown as Document;
	const root = createElement('div') as unknown as HTMLElement;
	return {
		document,
		dispatchDocument: (type) => {
			for (const listener of documentListeners.get(type) ?? []) listener({ type } as Event);
		},
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
		setVisibilityState: (value) => {
			visibilityState = value;
		},
		selectAccessibilityText: (startRow, startOffset, endRow, endOffset) => {
			const accessibilityRoot = (root as unknown as FakeElement).children[2];
			const startElement = accessibilityRoot?.children[startRow];
			const endElement = accessibilityRoot?.children[endRow];
			if (!startElement || !endElement) throw new Error('accessibility row is unavailable');
			const startContainer = { nodeType: 3, parentElement: startElement } as unknown as Node;
			const endContainer = { nodeType: 3, parentElement: endElement } as unknown as Node;
			selection = {
				getRangeAt: () => ({ endContainer, endOffset, startContainer, startOffset }),
				isCollapsed: false,
				rangeCount: 1,
			} as unknown as Selection;
			for (const listener of documentListeners.get('selectionchange') ?? []) {
				listener({ type: 'selectionchange' } as Event);
			}
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
	it('emits each scoped bell sequence once and accepts a baselined sequence gap', () => {
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		const bells = vi.fn<Parameters<ProjectedTerminal['onBell']>[0]>();
		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		terminal.onBell(bells);
		terminal.onError(errors);

		terminal.notifyBell({ attachmentId, sequence: canonicalText('7') });
		terminal.notifyBell({ attachmentId, sequence: canonicalText('7') });
		terminal.notifyBell({ attachmentId, sequence: canonicalText('6') });
		terminal.notifyBell({ attachmentId, sequence: canonicalText('9') });
		terminal.notifyBell({ attachmentId: otherAttachmentId, sequence: canonicalText('10') });

		expect(bells.mock.calls.map(([event]) => event.sequence)).toEqual(['7', '9']);
		expect(errors).toHaveBeenCalledWith(expect.objectContaining({ operation: 'bell' }));
		terminal.dispose();
		terminal.notifyBell({ attachmentId, sequence: canonicalText('10') });
		expect(bells).toHaveBeenCalledTimes(2);
	});

	it('waits for the configured font and keeps the IME textarea font in sync', async () => {
		const browser = createFakeBrowser();
		let finishFontLoad: (() => void) | undefined;
		const fontLoad = vi.fn<() => Promise<FontFace[]>>(
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
		expect(terminal.textarea?.style.opacity).toBe('0');
		expect(terminal.textarea?.style.caretColor).toBe('transparent');
		expect(terminal.textarea?.style.clipPath).toBe('inset(50%)');
		expect(terminal.textarea?.style.margin).toBe('0px');
		expect((browser.root as unknown as FakeElement).children[2]?.style['userSelect']).toBe('text');
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
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, interaction, {
				column: 1,
				row: 1,
			}),
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

		expect(input.style['left']).toBe('4px');
		expect(input.style['top']).toBe('16px');
		expect(input.style['width']).toBe('16px');
		expect(input.style['height']).toBe('16px');
		expect(input.style['lineHeight']).toBe('16px');
		terminal.dispose();
	});

	it('uses pinned Ghostty widths for non-BMP CJK preedit geometry', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
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
				{ column: 0, row: 0 },
				1n,
				6,
			),
		);
		const input = terminal.textarea as unknown as FakeElement & {
			selectionEnd: number;
			selectionStart: number;
		};
		input.selectionStart = 4;
		input.selectionEnd = 4;

		input.dispatch('compositionstart', {
			data: '\u{20000}\u{30000}',
			type: 'compositionstart',
		} as unknown as CompositionEvent);

		expect(input.style['width']).toBe('32px');
		terminal.dispose();
	});

	it('suppresses one composition in history and restores it only with the live cursor', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const setPreedit = vi.spyOn(WebGPURenderer.prototype, 'setPreedit');
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
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
		expect(setPreedit).toHaveBeenLastCalledWith([{ text: '你', width: 2 }], {
			start: 2,
			end: 2,
		});

		terminal.scrollLines(-1);
		expect(setPreedit).toHaveBeenLastCalledWith([]);
		expect(input.style['left']).toBe('0px');
		expect(input.style['top']).toBe('0px');
		expect(input.style['width']).toBe('1px');

		terminal.scrollToBottom();
		terminal.applyFrame(terminalFrame('snapshot', 2n, 2n, ['c', 'd']));
		expect(setPreedit).toHaveBeenLastCalledWith([{ text: '你', width: 2 }], {
			start: 2,
			end: 2,
		});
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

	it('pins each coalesced draw checkpoint to the latest committed generation and revision', async () => {
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
		const telemetry = vi.fn<(event: ProjectedTerminalFrameTelemetry) => void>();
		terminal.onFrameTelemetry(telemetry);
		await terminal.open(browser.root);

		expect(terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']))).toBe('committed');
		expect(terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['c', ''], 1n, [0]))).toBe(
			'committed',
		);
		expect(browser.frames.size).toBe(1);
		expect(commands.slice(-2)).toEqual([
			{
				kind: 'frame-applied',
				payload: { attachmentId, generation: '1', revision: '1', sequence: '1' },
			},
			{
				kind: 'frame-applied',
				payload: { attachmentId, generation: '1', revision: '2', sequence: '2' },
			},
		]);
		browser.flushFrames();
		expect(telemetry).toHaveBeenLastCalledWith(
			expect.objectContaining({ generation: '1', revision: '2' }),
		);

		expect(
			terminal.applyFrame(
				terminalFrame(
					'snapshot',
					3n,
					3n,
					['d', 'e'],
					null,
					[0, 1],
					0n,
					interaction,
					{ column: 0, row: 0 },
					2n,
				),
			),
		).toBe('committed');
		browser.flushFrames();
		expect(telemetry).toHaveBeenLastCalledWith(
			expect.objectContaining({ generation: '2', revision: '3' }),
		);
		expect(telemetry).toHaveBeenCalledTimes(2);
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
		const accessibilityAnnouncement = (browser.root as unknown as FakeElement).children[3];
		expect(accessibilityRoot?.attributes.get('aria-live')).toBe('off');
		expect(accessibilityAnnouncement?.attributes.get('aria-live')).toBe('polite');
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
		expect(accessibilityAnnouncement?.attributes.get('aria-live')).toBe('off');
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

	it('routes fixed scrollbar ownership through bounded absolute requests and latest-intent coalescing', async () => {
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
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n, {
				...interaction,
				mouseButton: true,
			}),
		);
		browser.flushFrames();
		commands.length = 0;
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointermove', {
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		await vi.waitFor(() =>
			expect(commands.some((command) => command.kind === 'read-rows')).toBe(true),
		);
		const linkRead = commands.at(-1);
		if (linkRead?.kind !== 'read-rows') throw new Error('link lookup did not request Host rows');
		if (linkRead.payload.origin.kind !== 'absolute') {
			throw new Error('link lookup did not use an absolute Host row');
		}
		const linkStartRow = BigInt(linkRead.payload.origin.row);
		const linkRowIndex = Number(98n - linkStartRow);
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(linkRead.payload.requestSequence),
					1n,
					linkStartRow,
					100n,
					Array.from({ length: linkRead.payload.rowCount }, (_, index) =>
						index === linkRowIndex ? 'linked' : '',
					),
					{
						hyperlink: 'https://example.com/scrollbar-preemption',
						hyperlinkRowIndex: linkRowIndex,
					},
				),
			),
		).toBe('committed');
		await vi.waitFor(() => expect(canvas.style['cursor']).toBe('pointer'));
		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 8,
			clientY: 18,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 40,
			preventDefault: () => {},
			shiftKey: true,
			type: 'pointerdown',
		} as unknown as PointerEvent);
		expect(canvas.capturedPointers.has(40)).toBe(true);
		commands.length = 0;
		const preventDefault = vi.fn<() => void>();
		const stopImmediatePropagation = vi.fn<() => void>();

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 41,
			preventDefault,
			shiftKey: false,
			stopImmediatePropagation,
			type: 'pointerdown',
		} as unknown as PointerEvent);

		expect(preventDefault).toHaveBeenCalledOnce();
		expect(stopImmediatePropagation).toHaveBeenCalledOnce();
		expect(canvas.capturedPointers.has(40)).toBe(false);
		expect(canvas.capturedPointers.has(41)).toBe(true);
		expect(canvas.style['cursor']).toBe('');
		expect(commands.some((command) => command.kind === 'mouse-input')).toBe(false);
		expect(commands.some((command) => command.kind === 'selection-gesture')).toBe(false);
		const firstRead = commands.find((command) => command.kind === 'read-rows');
		if (firstRead?.kind !== 'read-rows') throw new Error('scrollbar request was not sent');
		expect(firstRead.payload).toMatchObject({
			origin: { kind: 'absolute', row: '33' },
			purpose: 'viewport',
			rowCount: 34,
		});

		for (const clientY of [100, 260]) {
			canvas.dispatch('pointermove', {
				altKey: false,
				buttons: 1,
				clientX: 715,
				clientY,
				ctrlKey: false,
				metaKey: false,
				pointerId: 41,
				preventDefault: () => {},
				shiftKey: false,
				stopImmediatePropagation: () => {},
				type: 'pointermove',
			} as unknown as PointerEvent);
		}
		expect(commands.filter((command) => command.kind === 'read-rows')).toHaveLength(1);

		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(firstRead.payload.requestSequence),
					1n,
					33n,
					100n,
					Array.from({ length: 34 }, (_, index) => `row-${index}`),
				),
			),
		).toBe('committed');
		const reads = commands.filter((command) => command.kind === 'read-rows');
		expect(reads).toHaveLength(2);
		expect(reads.at(-1)?.payload).toMatchObject({
			origin: { kind: 'absolute', row: '57' },
			purpose: 'viewport',
			rowCount: 34,
		});
		terminal.dispose();
	});

	it('keeps scrollbar ownership fixed to the original primary pointer', async () => {
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
		const linkRequests = vi.fn<Parameters<ProjectedTerminal['onLinkOpenRequest']>[0]>();
		terminal.onLinkOpenRequest(linkRequests);
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n, {
				...interaction,
				mouseButton: true,
			}),
		);
		browser.flushFrames();
		commands.length = 0;
		const canvas = terminal.canvas as unknown as FakeElement;
		const dispatchOwned = (
			type: 'pointercancel' | 'pointerdown' | 'pointermove' | 'pointerup',
			values: Record<string, unknown>,
		): void => {
			const preventDefault = vi.fn<() => void>();
			const stopImmediatePropagation = vi.fn<() => void>();
			canvas.dispatch(type, {
				altKey: false,
				button: 0,
				buttons: 1,
				clientX: 715,
				clientY: 180,
				ctrlKey: false,
				detail: 1,
				metaKey: false,
				preventDefault,
				shiftKey: false,
				stopImmediatePropagation,
				type,
				...values,
			} as unknown as PointerEvent);
			expect(preventDefault).toHaveBeenCalledOnce();
			expect(stopImmediatePropagation).toHaveBeenCalledOnce();
		};

		dispatchOwned('pointerdown', { pointerId: 41 });
		expect(canvas.capturedPointers.has(41)).toBe(true);
		const firstRead = commands.find((command) => command.kind === 'read-rows');
		if (firstRead?.kind !== 'read-rows') throw new Error('scrollbar request was not sent');

		dispatchOwned('pointerdown', { clientY: 40, pointerId: 42 });
		dispatchOwned('pointerdown', {
			clientX: 8,
			clientY: 18,
			pointerId: 43,
			shiftKey: true,
		});
		dispatchOwned('pointermove', { clientX: 8, clientY: 18, pointerId: 42 });
		dispatchOwned('pointerup', { buttons: 0, pointerId: 42 });
		dispatchOwned('pointercancel', { buttons: 0, pointerId: 42 });
		dispatchOwned('pointerdown', { button: 2, buttons: 3, pointerId: 41 });
		dispatchOwned('pointerup', { button: 2, buttons: 1, pointerId: 41 });

		expect(canvas.capturedPointers.has(41)).toBe(true);
		expect(canvas.capturedPointers.has(42)).toBe(false);
		expect(canvas.capturedPointers.has(43)).toBe(false);
		expect(commands.filter((command) => command.kind === 'read-rows')).toHaveLength(1);
		expect(commands.some((command) => command.kind === 'mouse-input')).toBe(false);
		expect(commands.some((command) => command.kind === 'selection-gesture')).toBe(false);
		expect(linkRequests).not.toHaveBeenCalled();

		dispatchOwned('pointermove', { clientY: 260, pointerId: 41 });
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(firstRead.payload.requestSequence),
					1n,
					33n,
					100n,
					Array.from({ length: 34 }, (_, index) => `row-${index}`),
				),
			),
		).toBe('committed');
		expect(
			commands.filter((command) => command.kind === 'read-rows').at(-1)?.payload,
		).toMatchObject({
			origin: { kind: 'absolute', row: '57' },
			purpose: 'viewport',
			rowCount: 34,
		});
		dispatchOwned('pointerup', { buttons: 0, pointerId: 41 });
		expect(canvas.capturedPointers.has(41)).toBe(false);
		terminal.dispose();
	});

	it('owns wheel and the synthesized click for an active scrollbar drag', async () => {
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
		const linkRequests = vi.fn<Parameters<ProjectedTerminal['onLinkOpenRequest']>[0]>();
		terminal.onLinkOpenRequest(linkRequests);
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['linked', ''], null, [0, 1], 98n, {
				...interaction,
				mouseButton: true,
			}),
		);
		browser.flushFrames();
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointermove', {
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		await vi.waitFor(() =>
			expect(commands.some((command) => command.kind === 'read-rows')).toBe(true),
		);
		const linkRead = commands.at(-1);
		if (linkRead?.kind !== 'read-rows') throw new Error('link lookup did not request Host rows');
		if (linkRead.payload.origin.kind !== 'absolute') {
			throw new Error('link lookup did not use an absolute Host row');
		}
		const linkStartRow = BigInt(linkRead.payload.origin.row);
		const linkRowIndex = Number(98n - linkStartRow);
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(linkRead.payload.requestSequence),
					1n,
					linkStartRow,
					100n,
					Array.from({ length: linkRead.payload.rowCount }, (_, index) =>
						index === linkRowIndex ? 'linked' : '',
					),
					{
						hyperlink: 'https://example.com/scrollbar-click',
						hyperlinkRowIndex: linkRowIndex,
					},
				),
			),
		).toBe('committed');
		await vi.waitFor(() => expect(canvas.style['cursor']).toBe('pointer'));
		commands.length = 0;

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 45,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerdown',
		} as unknown as PointerEvent);
		const wheelPreventDefault = vi.fn<() => void>();
		const wheelStopImmediatePropagation = vi.fn<() => void>();
		canvas.dispatch('wheel', {
			buttons: 0,
			deltaY: 18,
			preventDefault: wheelPreventDefault,
			shiftKey: false,
			stopImmediatePropagation: wheelStopImmediatePropagation,
			type: 'wheel',
		} as unknown as WheelEvent);
		expect(wheelPreventDefault).toHaveBeenCalledOnce();
		expect(wheelStopImmediatePropagation).toHaveBeenCalledOnce();
		expect(commands.filter((command) => command.kind === 'read-rows')).toHaveLength(1);
		expect(commands.some((command) => command.kind === 'mouse-input')).toBe(false);

		canvas.dispatch('pointerup', {
			altKey: false,
			button: 0,
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			metaKey: false,
			pointerId: 45,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerup',
		} as unknown as PointerEvent);
		const bubbledClick = vi.fn<(event: Event) => void>();
		browser.root.addEventListener('click', bubbledClick);
		const clickPreventDefault = vi.fn<() => void>();
		const clickStopImmediatePropagation = vi.fn<() => void>();
		const click = {
			bubbles: true,
			button: 0,
			get cancelBubble() {
				return clickStopImmediatePropagation.mock.calls.length > 0;
			},
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			preventDefault: clickPreventDefault,
			stopImmediatePropagation: clickStopImmediatePropagation,
			type: 'click',
		} as unknown as MouseEvent;
		canvas.dispatch('click', click);
		expect(clickPreventDefault).toHaveBeenCalledOnce();
		expect(clickStopImmediatePropagation).toHaveBeenCalledOnce();
		expect(bubbledClick).not.toHaveBeenCalled();
		expect(linkRequests).not.toHaveBeenCalled();
		expect(commands.some((command) => command.kind === 'mouse-input')).toBe(false);

		const laterBubbledClick = vi.fn<(event: Event) => void>();
		browser.root.addEventListener('click', laterBubbledClick);
		const laterPreventDefault = vi.fn<() => void>();
		const laterStopImmediatePropagation = vi.fn<() => void>();
		const laterClick = {
			bubbles: true,
			button: 0,
			get cancelBubble() {
				return laterStopImmediatePropagation.mock.calls.length > 0;
			},
			clientX: 8,
			clientY: 18,
			ctrlKey: false,
			metaKey: false,
			preventDefault: laterPreventDefault,
			stopImmediatePropagation: laterStopImmediatePropagation,
			type: 'click',
		} as unknown as MouseEvent;
		canvas.dispatch('click', laterClick);
		expect(laterPreventDefault).not.toHaveBeenCalled();
		expect(laterStopImmediatePropagation).not.toHaveBeenCalled();
		expect(laterBubbledClick).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('shows navigation and drag activity immediately and fades only after 1500ms', async () => {
		vi.useFakeTimers();
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const setHistoryScrollbar = vi.spyOn(WebGPURenderer.prototype, 'setHistoryScrollbar');
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n));
		browser.flushFrames();
		setHistoryScrollbar.mockClear();

		terminal.scrollLines(-1);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(1);
		vi.advanceTimersByTime(1_499);
		expect(setHistoryScrollbar.mock.calls.some(([state]) => state?.opacity === 0)).toBe(false);
		vi.advanceTimersByTime(1);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(0);

		setHistoryScrollbar.mockClear();
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 42,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerdown',
		} as unknown as PointerEvent);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(1);
		setHistoryScrollbar.mockClear();
		vi.advanceTimersByTime(2_000);
		expect(setHistoryScrollbar.mock.calls.some(([state]) => state?.opacity === 0)).toBe(false);

		canvas.dispatch('pointerup', {
			altKey: false,
			button: 0,
			buttons: 0,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			metaKey: false,
			pointerId: 42,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerup',
		} as unknown as PointerEvent);
		expect(canvas.capturedPointers.has(42)).toBe(false);
		vi.advanceTimersByTime(1_499);
		expect(setHistoryScrollbar.mock.calls.some(([state]) => state?.opacity === 0)).toBe(false);
		vi.advanceTimersByTime(1);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(0);
		expect(canvas.attributes.get('aria-hidden')).toBe('true');
		expect(canvas.getAttribute('role')).toBeNull();
		terminal.dispose();
		setHistoryScrollbar.mockRestore();
	});

	it('shows and fades the scrollbar when search reveals an offscreen history row', async () => {
		vi.useFakeTimers();
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const setHistoryScrollbar = vi.spyOn(WebGPURenderer.prototype, 'setHistoryScrollbar');
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
			searchWorkerFactory: () => searchWorkerWithMatch(1),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n));
		browser.flushFrames();
		commands.length = 0;
		setHistoryScrollbar.mockClear();
		const rows: TerminalBufferRow[] = Array.from({ length: 100 }, (_, index) => ({
			cells: null,
			identityAscii: true,
			index: BigInt(index),
			mappings: [],
			promptClass: RowPromptClass.none,
			text: index === 1 ? 'needle' : '',
			wrap: false,
			wrapContinuation: false,
		}));
		const readBufferRows = vi
			.spyOn(terminal, 'readBufferRows')
			.mockImplementation(async (request) => {
				const start = Number(request.startRow);
				const page = rows.slice(start, start + request.rowCount);
				return {
					byteLength: page.length,
					detail: request.detail,
					generation: request.generation,
					kind: 'rows',
					revision: request.revision,
					rows: page,
					startRow: request.startRow,
					totalRows: 100n,
				};
			});

		terminal.updateSearch('needle');
		await vi.advanceTimersByTimeAsync(200);

		expect(readBufferRows).toHaveBeenCalledOnce();
		expect(
			commands.find(
				(command) => command.kind === 'read-rows' && command.payload.purpose === 'viewport',
			)?.payload,
		).toMatchObject({
			origin: { kind: 'absolute', row: '0' },
			purpose: 'viewport',
		});
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(1);
		setHistoryScrollbar.mockClear();
		vi.advanceTimersByTime(1_499);
		expect(setHistoryScrollbar.mock.calls.some(([state]) => state?.opacity === 0)).toBe(false);
		vi.advanceTimersByTime(1);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(0);
		terminal.dispose();
		readBufferRows.mockRestore();
		setHistoryScrollbar.mockRestore();
	});

	it('defers search and navigation fade until an active scrollbar drag ends', async () => {
		vi.useFakeTimers();
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const setHistoryScrollbar = vi.spyOn(WebGPURenderer.prototype, 'setHistoryScrollbar');
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			searchWorkerFactory: () => searchWorkerWithMatch(1),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n));
		browser.flushFrames();
		const rows: TerminalBufferRow[] = Array.from({ length: 100 }, (_, index) => ({
			cells: null,
			identityAscii: true,
			index: BigInt(index),
			mappings: [],
			promptClass: RowPromptClass.none,
			text: index === 1 ? 'needle' : '',
			wrap: false,
			wrapContinuation: false,
		}));
		const readBufferRows = vi
			.spyOn(terminal, 'readBufferRows')
			.mockImplementation(async (request) => {
				const start = Number(request.startRow);
				const page = rows.slice(start, start + request.rowCount);
				return {
					byteLength: page.length,
					detail: request.detail,
					generation: request.generation,
					kind: 'rows',
					revision: request.revision,
					rows: page,
					startRow: request.startRow,
					totalRows: 100n,
				};
			});
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 46,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerdown',
		} as unknown as PointerEvent);
		setHistoryScrollbar.mockClear();

		terminal.updateSearch('needle');
		await vi.advanceTimersByTimeAsync(200);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(1);
		setHistoryScrollbar.mockClear();
		vi.advanceTimersByTime(2_000);
		expect(setHistoryScrollbar.mock.calls.some(([state]) => state?.opacity === 0)).toBe(false);

		terminal.scrollLines(-1);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(1);
		setHistoryScrollbar.mockClear();
		vi.advanceTimersByTime(2_000);
		expect(setHistoryScrollbar.mock.calls.some(([state]) => state?.opacity === 0)).toBe(false);

		canvas.dispatch('pointerup', {
			altKey: false,
			button: 0,
			buttons: 0,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			metaKey: false,
			pointerId: 46,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerup',
		} as unknown as PointerEvent);
		vi.advanceTimersByTime(1_499);
		expect(setHistoryScrollbar.mock.calls.some(([state]) => state?.opacity === 0)).toBe(false);
		vi.advanceTimersByTime(1);
		expect(setHistoryScrollbar.mock.calls.at(-1)?.[0]?.opacity).toBe(0);
		terminal.dispose();
		readBufferRows.mockRestore();
		setHistoryScrollbar.mockRestore();
	});

	it.each([
		'buttonless pointer movement',
		'pointer cancellation',
		'lost pointer capture',
		'textarea blur',
		'window blur',
		'visibility loss',
		'reset',
		'disposal',
	])('repairs scrollbar ownership after %s', async (repair) => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n));
		browser.flushFrames();
		commands.length = 0;
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 43,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerdown',
		} as unknown as PointerEvent);
		expect(canvas.capturedPointers.has(43)).toBe(true);

		switch (repair) {
			case 'buttonless pointer movement':
				canvas.dispatch('pointermove', {
					altKey: false,
					buttons: 0,
					clientX: 715,
					clientY: 180,
					ctrlKey: false,
					metaKey: false,
					pointerId: 43,
					preventDefault: () => {},
					shiftKey: false,
					stopImmediatePropagation: () => {},
					type: 'pointermove',
				} as unknown as PointerEvent);
				break;
			case 'pointer cancellation':
				canvas.dispatch('pointercancel', {
					pointerId: 43,
					preventDefault: () => {},
					stopImmediatePropagation: () => {},
					type: 'pointercancel',
				} as unknown as PointerEvent);
				break;
			case 'lost pointer capture':
				canvas.capturedPointers.delete(43);
				canvas.dispatch('lostpointercapture', {
					pointerId: 43,
					preventDefault: () => {},
					stopImmediatePropagation: () => {},
					type: 'lostpointercapture',
				} as unknown as PointerEvent);
				break;
			case 'textarea blur':
				(terminal.textarea as unknown as FakeElement).dispatch('blur', {
					type: 'blur',
				} as FocusEvent);
				break;
			case 'window blur':
				browser.dispatchWindow('blur');
				break;
			case 'visibility loss':
				browser.setVisibilityState('hidden');
				browser.dispatchDocument('visibilitychange');
				break;
			case 'reset':
				terminal.reset();
				break;
			case 'disposal':
				terminal.dispose();
				break;
		}

		expect(canvas.capturedPointers.has(43)).toBe(false);
		if (repair === 'disposal') return;
		const preventDefault = vi.fn<() => void>();
		canvas.dispatch('pointerup', {
			altKey: false,
			button: 0,
			buttons: 0,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			metaKey: false,
			pointerId: 43,
			preventDefault,
			shiftKey: true,
			stopImmediatePropagation: () => {},
			type: 'pointerup',
		} as unknown as PointerEvent);
		expect(preventDefault).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('repairs read-only scrollbar capture before reset rejects Host control', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n));
		browser.flushFrames();
		commands.length = 0;
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 44,
			preventDefault: () => {},
			shiftKey: false,
			stopImmediatePropagation: () => {},
			type: 'pointerdown',
		} as unknown as PointerEvent);
		expect(canvas.capturedPointers.has(44)).toBe(true);

		terminal.reset();

		expect(canvas.capturedPointers.has(44)).toBe(false);
		expect(commands.some((command) => command.kind === 'reset-terminal')).toBe(false);
		const preventDefault = vi.fn<() => void>();
		canvas.dispatch('pointerup', {
			altKey: false,
			button: 0,
			buttons: 0,
			clientX: 715,
			clientY: 180,
			ctrlKey: false,
			metaKey: false,
			pointerId: 44,
			preventDefault,
			shiftKey: true,
			stopImmediatePropagation: () => {},
			type: 'pointerup',
		} as unknown as PointerEvent);
		expect(preventDefault).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('echoes each distinct HistoryChanged sequence in its HistoryObserved acknowledgement', async () => {
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
		terminal.notifyHistoryChanged({
			anchorStatus: 'valid',
			attachmentId,
			generation: canonicalText('1'),
			revision: canonicalText('1'),
			sequence: canonicalText('1'),
			totalRows: canonicalText('10'),
		});
		expect(commands.at(-1)).toEqual({
			kind: 'history-observed',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '1' },
		});

		terminal.notifyHistoryChanged({
			anchorStatus: 'valid',
			attachmentId,
			generation: canonicalText('1'),
			revision: canonicalText('1'),
			sequence: canonicalText('2'),
			totalRows: canonicalText('10'),
		});
		expect(commands.at(-1)).toEqual({
			kind: 'history-observed',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '2' },
		});

		terminal.dispose();
	});

	it('rejects a text row flight with an over-cap mapped cell without committing buffer rows', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 42n, ['a', 'b'], null, [0, 1], 10n));

		const rows = terminal.readBufferRows(
			{
				detail: 'text',
				generation: 1n,
				maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				revision: 42n,
				rowCount: 1,
				startRow: 0n,
			},
			new AbortController().signal,
		);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('buffer rows request was not sent');
		expect(read.payload.detail).toBe('text');

		expect(terminal.applyRows(textRowWindow(BigInt(read.payload.requestSequence), 16_385))).toBe(
			'rejected',
		);
		await expect(rows).resolves.toEqual({
			generation: 1n,
			kind: 'unavailable',
			revision: 42n,
			totalRows: 12n,
		});
		expect(commands).not.toEqual(
			expect.arrayContaining([expect.objectContaining({ kind: 'rows-applied' })]),
		);
		terminal.dispose();
	});

	it('mirrors a Host selection into the accessible DOM at absolute scrollback rows', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));

		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: CanonicalUint64TextSchema.parse('11') },
			preview: null,
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			state: 'active',
		});

		const selection = browser.document.getSelection();
		expect(selection?.rangeCount).toBe(1);
		const range = selection?.getRangeAt(0);
		const accessibilityRoot = (browser.root as unknown as FakeElement).children[2];
		expect(
			(range?.startContainer as unknown as { readonly parentElement?: FakeElement })?.parentElement,
		).toBe(accessibilityRoot?.children[0]);
		expect(range?.startOffset).toBe(0);
		expect(
			(range?.endContainer as unknown as { readonly parentElement?: FakeElement })?.parentElement,
		).toBe(accessibilityRoot?.children[1]);
		expect(range?.endOffset).toBe(1);
		terminal.dispose();
	});

	it('publishes localized row, cursor, selection, and deletion accessibility updates', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: {
				deleted: (text) => `removed ${text}`,
				input: 'input',
				row: (row, text) => `row ${row}: ${text}`,
				screen: 'screen',
				screenCursor: (row, column) => `cursor ${row}:${column}`,
				selectedText: (text) => `selected ${text}`,
			},
			port: { send: () => {} },
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, interaction, {
				column: 1,
				row: 0,
			}),
		);
		const accessibilityRoot = (browser.root as unknown as FakeElement).children[2];
		const announcement = (browser.root as unknown as FakeElement).children[3];

		expect(accessibilityRoot?.attributes.get('aria-label')).toBe('cursor 1:2');
		expect(accessibilityRoot?.children[0]?.attributes.get('aria-label')).toBe('row 1: a');
		expect(announcement?.attributes.get('role')).toBe('status');
		expect(announcement?.textContent).toBe('a\nb');

		terminal.applyFrame(
			terminalFrame('delta', 2n, 2n, ['', 'b'], 1n, [0], 0n, interaction, {
				column: 0,
				row: 0,
			}),
		);
		expect(announcement?.textContent).toBe('removed a');

		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: CanonicalUint64TextSchema.parse('1') },
			preview: 'chosen',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('1') },
			state: 'active',
		});
		expect(accessibilityRoot?.attributes.get('aria-description')).toBe('selected chosen');
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

	it('routes a plain-text drop through the paste policy without accepting file drops', async () => {
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
		const root = browser.root as unknown as FakeElement;
		const preventDragDefault = vi.fn<() => void>();
		const textTransfer = {
			dropEffect: 'none',
			files: [],
			getData: (type: string) => (type === 'text/plain' ? 'dropped text' : ''),
			types: ['text/plain'],
		};

		root.dispatch('dragover', {
			dataTransfer: textTransfer,
			preventDefault: preventDragDefault,
			type: 'dragover',
		} as unknown as DragEvent);
		expect(preventDragDefault).toHaveBeenCalledOnce();
		expect(textTransfer.dropEffect).toBe('copy');

		const preventDropDefault = vi.fn<() => void>();
		const stopPropagation = vi.fn<() => void>();
		root.dispatch('drop', {
			dataTransfer: textTransfer,
			preventDefault: preventDropDefault,
			stopPropagation,
			type: 'drop',
		} as unknown as DragEvent);
		await vi.waitFor(() =>
			expect(commands.at(-1)).toEqual({
				kind: 'paste-input',
				payload: { attachmentId, text: 'dropped text' },
			}),
		);
		expect(preventDropDefault).toHaveBeenCalledOnce();
		expect(stopPropagation).toHaveBeenCalledOnce();

		const fileDefault = vi.fn<() => void>();
		root.dispatch('drop', {
			dataTransfer: {
				files: [{}],
				getData: () => '/untrusted/browser/path',
				types: ['Files', 'text/plain'],
			},
			preventDefault: fileDefault,
			stopPropagation: vi.fn<() => void>(),
			type: 'drop',
		} as unknown as DragEvent);
		expect(fileDefault).not.toHaveBeenCalled();
		expect(commands.filter((command) => command.kind === 'paste-input')).toHaveLength(1);
		terminal.dispose();
	});

	it('keeps observed key input behind an asynchronous paste decision', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let finishReview: ((approved: boolean) => void) | undefined;
		const reviewPaste = vi.fn<() => Promise<boolean>>(
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
					.filter((command) => command.kind === 'key-input' || command.kind === 'paste-input')
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
		const reviewPaste = vi.fn<() => Promise<boolean>>(
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
		input.dispatch('focus', { type: 'focus' } as FocusEvent);

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
					command.kind === 'key-input'
						? `${command.kind}:${command.payload.action}`
						: `${command.kind}:${command.payload.focused}`,
				),
		).toEqual(['focus-input:true', 'key-input:press', 'key-input:release', 'focus-input:false']);
		terminal.dispose();
	});

	it('repairs pressed input when the window loses focus without a textarea blur', async () => {
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
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
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

		browser.dispatchWindow('blur');

		expect(
			commands
				.filter((command) => command.kind === 'key-input' || command.kind === 'focus-input')
				.map((command) =>
					command.kind === 'key-input'
						? `${command.kind}:${command.payload.action}`
						: `${command.kind}:${command.payload.focused}`,
				),
		).toEqual(['focus-input:true', 'key-input:press', 'key-input:release', 'focus-input:false']);
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

	it('orders reset after accepted input and repairs pressed keys before the control request', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let finishReview: ((approved: boolean) => void) | undefined;
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
			reviewPaste: async () =>
				await new Promise<boolean>((resolve) => {
					finishReview = resolve;
				}),
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

		const paste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(finishReview).toBeTypeOf('function'));
		terminal.reset();
		expect(commands.some((command) => command.kind === 'reset-terminal')).toBe(false);

		finishReview?.(true);
		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'sent' }));
		await vi.waitFor(() =>
			expect(commands.some((command) => command.kind === 'reset-terminal')).toBe(true),
		);
		expect(
			commands
				.filter(
					(command) =>
						command.kind === 'key-input' ||
						command.kind === 'paste-input' ||
						command.kind === 'reset-terminal',
				)
				.map((command) =>
					command.kind === 'key-input' ? `${command.kind}:${command.payload.action}` : command.kind,
				),
		).toEqual(['key-input:press', 'paste-input', 'key-input:release', 'reset-terminal']);
		terminal.dispose();
	});

	it('exposes clear and reset only while the attachment owns input capability', () => {
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
		expect(terminal.canControlTerminal).toBe(true);
		terminal.clear();
		terminal.reset();
		expect(commands.filter((command) => command.kind === 'clear-terminal')).toEqual([
			{ kind: 'clear-terminal', payload: { attachmentId } },
		]);
		expect(commands.filter((command) => command.kind === 'reset-terminal')).toEqual([
			{ kind: 'reset-terminal', payload: { attachmentId } },
		]);
		expect(
			commands.findIndex(
				(command) => command.kind === 'selection-command' && command.payload.command === 'clear',
			),
		).toBeLessThan(commands.findIndex((command) => command.kind === 'reset-terminal'));

		terminal.dispose();
		expect(terminal.canControlTerminal).toBe(false);
		const commandCount = commands.length;
		terminal.clear();
		terminal.reset();
		expect(commands).toHaveLength(commandCount);

		const readOnlyCommands: ProjectedTerminalCommand[] = [];
		const readOnly = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			inputEnabled: false,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					readOnlyCommands.push(command);
				},
			},
			terminalId,
		});
		expect(readOnly.canControlTerminal).toBe(false);
		readOnly.clear();
		readOnly.reset();
		expect(readOnlyCommands).toHaveLength(0);
		readOnly.dispose();
	});

	it('sends an accessibility DOM range as one explicit cell-range command', async () => {
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

		browser.selectAccessibilityText(0, 0, 0, 1);

		expect(commands.at(-1)).toEqual({
			kind: 'selection-command',
			payload: {
				attachmentId,
				command: 'set-range',
				end: { column: 0, row: '0' },
				generation: '1',
				rectangle: false,
				revision: '1',
				start: { column: 0, row: '0' },
			},
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

	it('uses modifier-assisted triple click to select one semantic output block', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			detail: 3,
			metaKey: true,
			pointerId: 17,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);

		expect(commands.at(-1)).toEqual({
			kind: 'selection-command',
			payload: {
				attachmentId,
				command: 'select-output',
				generation: '1',
				point: { column: 0, row: '10' },
				revision: '1',
			},
		});
		expect(canvas.capturedPointers.has(17)).toBe(false);
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: CanonicalUint64TextSchema.parse('11') },
			preview: 'semantic output',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			state: 'active',
		});
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(1);
		terminal.dispose();
	});

	it('copies an active Host selection once its pointer gesture ends', async () => {
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
		const activeSelection = {
			attachmentId,
			end: { column: 1, row: CanonicalUint64TextSchema.parse('0') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active' as const,
		};

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 18,
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
			pointerId: 18,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		browser.flushFrames();
		terminal.notifySelectionChanged(activeSelection);
		canvas.dispatch('pointerup', {
			altKey: false,
			button: 0,
			buttons: 0,
			clientX: 12,
			clientY: 2,
			ctrlKey: false,
			metaKey: false,
			pointerId: 18,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerup',
		} as unknown as PointerEvent);
		browser.flushFrames();
		terminal.notifySelectionChanged(activeSelection);

		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(1);
		terminal.dispose();
	});

	it('cancels an active selection gesture without ending or copying it during reset', async () => {
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
		const activeSelection = {
			attachmentId,
			end: { column: 1, row: CanonicalUint64TextSchema.parse('0') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active' as const,
		};

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 20,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);
		terminal.notifySelectionChanged(activeSelection);
		canvas.dispatch('pointermove', {
			altKey: false,
			buttons: 1,
			clientX: 12,
			clientY: 2,
			ctrlKey: false,
			metaKey: false,
			pointerId: 20,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		browser.flushFrames();
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start', 'update']);

		terminal.reset();
		expect(canvas.capturedPointers.has(20)).toBe(false);
		terminal.notifySelectionChanged(activeSelection);
		browser.flushFrames();
		terminal.notifySelectionChanged(activeSelection);

		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start', 'update']);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(0);
		const clearIndex = commands.findIndex(
			(command) => command.kind === 'selection-command' && command.payload.command === 'clear',
		);
		const resetIndex = commands.findIndex((command) => command.kind === 'reset-terminal');
		expect(clearIndex).toBeGreaterThan(-1);
		expect(resetIndex).toBeGreaterThan(clearIndex);
		terminal.dispose();
	});

	it('continues an edge drag after each bounded history viewport commit', async () => {
		vi.useFakeTimers();
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 18,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 19,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);
		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		canvas.dispatch('pointermove', {
			altKey: false,
			buttons: 1,
			clientX: 2,
			clientY: -1,
			ctrlKey: false,
			metaKey: false,
			pointerId: 19,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		browser.flushFrames();
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			preview: 'a',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			state: 'active',
		});

		vi.advanceTimersByTime(16);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('edge drag did not request history rows');
		expect(read.payload.origin).toEqual({ kind: 'absolute', row: '0' });
		terminal.applyRows(
			renderRowWindow(
				BigInt(read.payload.requestSequence),
				1n,
				0n,
				12n,
				Array.from({ length: 12 }, (_, index) => `row-${index}`),
			),
		);
		browser.flushFrames();

		const gestures = commands.filter((command) => command.kind === 'selection-gesture');
		expect(gestures.at(-1)?.payload).toMatchObject({
			action: 'update',
			point: { column: 0, row: '9' },
		});
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

	it('activates the exact modifier-assisted OSC 8 cell from a Host row window', async () => {
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
		const linkRequests = vi.fn<Parameters<ProjectedTerminal['onLinkOpenRequest']>[0]>();
		terminal.onLinkOpenRequest(linkRequests);
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['linked', '']));
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointermove', {
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		await vi.waitFor(() =>
			expect(commands.some((command) => command.kind === 'read-rows')).toBe(true),
		);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('link lookup did not request Host rows');
		expect(
			terminal.applyRows(
				renderRowWindow(BigInt(read.payload.requestSequence), 1n, 0n, 2n, ['linked', ''], {
					hyperlink: 'https://example.com/from-osc-8',
				}),
			),
		).toBe('committed');
		await vi.waitFor(() => expect(canvas.style['cursor']).toBe('pointer'));

		const preventDefault = vi.fn<() => void>();
		canvas.dispatch('click', {
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			preventDefault,
			type: 'click',
		} as unknown as MouseEvent);

		expect(linkRequests).toHaveBeenCalledWith({
			target: { kind: 'url', url: 'https://example.com/from-osc-8' },
		});
		expect(preventDefault).toHaveBeenCalledOnce();
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

		const resizeCommand = commands.find((command) => command.kind === 'resize-terminal');
		expect(resizeCommand).toEqual({
			kind: 'resize-terminal',
			payload: {
				attachmentId,
				cellPixelHeight: 4_096,
				cellPixelWidth: 4_096,
				columns: 15,
				rows: 15,
			},
		});
		expect(commands.map((command) => command.kind)).toEqual([
			'resize-terminal',
			'set-projection-demand',
		]);
		terminal.dispose();
	});

	it('does not propose a PTY grid when padding consumes the mounted content area', async () => {
		const browser = createFakeBrowser();
		const root = browser.root as unknown as FakeElement;
		root.clientWidth = 32;
		root.clientHeight = 32;
		root.style['paddingBottom'] = '16px';
		root.style['paddingLeft'] = '16px';
		root.style['paddingRight'] = '16px';
		root.style['paddingTop'] = '16px';
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

		expect(commands.filter((command) => command.kind === 'resize-terminal')).toEqual([]);
		terminal.dispose();
	});

	it('retries unchanged measured geometry after Host commits an external grid', async () => {
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
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(1);

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
				{ column: 0, row: 0 },
				1n,
				132,
			),
		);
		browser.dispatchWindow('resize');

		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(2);
		terminal.dispose();
	});

	it('retries unchanged measured geometry after a resize rejection or re-entry window', async () => {
		vi.useFakeTimers();
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

		browser.dispatchWindow('resize');
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(1);
		vi.advanceTimersByTime(51);
		browser.dispatchWindow('resize');
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(2);

		terminal.notifyBrokerFailure({
			code: 'resize-rejected',
			message: 'resize rejected',
			operation: 'resize',
			retryable: true,
		});
		browser.dispatchWindow('resize');
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(3);
		terminal.dispose();
	});

	it('suspends hidden rendering and resumes with one draw of the newest committed frame', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const setSuspended = vi.spyOn(WebGPURenderer.prototype, 'setSuspended');
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		const telemetry = vi.fn<(event: ProjectedTerminalFrameTelemetry) => void>();
		terminal.onFrameTelemetry(telemetry);
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		expect(browser.frames.size).toBe(1);

		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');
		expect(setSuspended).toHaveBeenLastCalledWith(true);
		expect(browser.frames.size).toBe(0);
		terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['c', 'd'], 1n));
		expect(browser.frames.size).toBe(0);

		browser.setVisibilityState('visible');
		browser.dispatchDocument('visibilitychange');
		expect(setSuspended).toHaveBeenLastCalledWith(false);
		expect(browser.frames.size).toBe(1);
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();
		expect(telemetry).toHaveBeenCalledWith(expect.objectContaining({ revision: '2' }));
		terminal.dispose();
	});

	it('applies Host-driven frame geometry without emitting a reverse resize', async () => {
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
		const telemetry = vi.fn<(event: ProjectedTerminalFrameTelemetry) => void>();
		terminal.onFrameTelemetry(telemetry);
		await terminal.open(browser.root);
		const resizeCount = commands.filter((command) => command.kind === 'resize-terminal').length;

		expect(
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
					{ column: 0, row: 0 },
					1n,
					132,
				),
			),
		).toBe('committed');
		expect(terminal.metadata?.columns).toBe(132);
		expect(terminal.metadata?.rows).toBe(2);
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(
			resizeCount,
		);

		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({ cellsVisited: 264, rowsRendered: 2 }),
		);
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(
			resizeCount,
		);
		terminal.dispose();
	});

	it('subtracts surface padding and rebalances every observed container resize', async () => {
		const browser = createFakeBrowser();
		const root = browser.root as unknown as FakeElement;
		root.style['paddingBottom'] = '8px';
		root.style['paddingLeft'] = '8px';
		root.style['paddingRight'] = '8px';
		root.style['paddingTop'] = '8px';
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let notifyResize: (() => void) | undefined;
		const disconnect = vi.fn<() => void>();
		const observe = vi.fn<(target: Element) => void>();
		class FakeResizeObserver {
			constructor(callback: ResizeObserverCallback) {
				notifyResize = () => callback([], this as unknown as ResizeObserver);
			}

			disconnect(): void {
				disconnect();
			}

			observe(target: Element): void {
				observe(target);
			}

			unobserve(): void {}
		}
		vi.stubGlobal('ResizeObserver', FakeResizeObserver);
		const syncSurfacePadding = vi.spyOn(WebGPURenderer.prototype, 'syncSurfacePadding');
		const commands: ProjectedTerminalCommand[] = [];
		let initialResize: ProjectedTerminalCommand | undefined;
		const terminal = new ProjectedTerminal({
			attachmentId,
			canResize: true,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (initialResize === undefined && command.kind === 'resize-terminal') {
						initialResize = command;
					}
				},
			},
			renderer: { paddingBalance: true },
			terminalId,
		});

		await terminal.open(browser.root);
		if (initialResize?.kind !== 'resize-terminal') throw new Error('initial resize is missing');
		expect(initialResize.payload.columns).toBe(
			Math.floor((root.clientWidth - 16) / initialResize.payload.cellPixelWidth),
		);
		expect(initialResize.payload.rows).toBe(
			Math.floor((root.clientHeight - 16) / initialResize.payload.cellPixelHeight),
		);
		expect(observe).toHaveBeenCalledWith(browser.root);

		syncSurfacePadding.mockClear();
		root.clientWidth += 1;
		notifyResize?.();
		expect(syncSurfacePadding).toHaveBeenCalledOnce();
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(1);

		root.clientWidth += initialResize.payload.cellPixelWidth;
		notifyResize?.();
		const resizeCommands = commands.filter((command) => command.kind === 'resize-terminal');
		expect(resizeCommands).toHaveLength(2);
		expect(resizeCommands.at(-1)).toEqual({
			kind: 'resize-terminal',
			payload: {
				...initialResize.payload,
				columns: initialResize.payload.columns + 1,
			},
		});
		terminal.dispose();
		expect(disconnect).toHaveBeenCalledOnce();
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
		const initialResize = commands
			.slice()
			.reverse()
			.find((command) => command.kind === 'resize-terminal');
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
