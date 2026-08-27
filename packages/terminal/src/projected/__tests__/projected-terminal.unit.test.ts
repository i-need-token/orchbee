import {
	ActiveScreen,
	CanonicalUint64TextSchema,
	type CanonicalUint64Text,
	CellWidth,
	CursorShape,
	encodeTerminalRowWindow,
	encodeTerminalFrame,
	KeyAction,
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
	TerminalCloseActivityCode,
	TerminalFrameExtensionKind,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	TerminalRowWindowDetail,
	UnderlineKind,
	type TerminalFrame,
	type TerminalExited,
	type TerminalFailed,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { BufferRowsResult, TerminalBufferRow } from '../../buffer-rows';
import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '../../presentation-configuration';
import { WebGPURenderer } from '../../rendering';
import type { ProjectedSearchWorkerLike } from '../../search';
import {
	ProjectedHistoryPhase,
	ProjectedTerminal,
	type ProjectedBrokerFailure,
	type ProjectedPasteReview,
	type ProjectedTerminalCommand,
	type ProjectedTerminalErrorEvent,
	type ProjectedTerminalFrameTelemetry,
} from '../projected-terminal';

const emptyGraphics = {
	animationTick: 0n,
	images: [],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [],
	resourceRevision: 0n,
} as const;

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
	hidden: boolean;
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
	getBoundingClientRect(): DOMRect;
	querySelectorAll<T extends Element>(selector: string): NodeListOf<T>;
	releasePointerCapture(pointerId: number): void;
	replaceChildren(...nodes: FakeElement[]): void;
	removeAttribute(name: string): void;
	remove(): void;
	setPointerCapture(pointerId: number): void;
}

interface FakeBrowser {
	readonly document: Document;
	readonly frames: Map<number, FrameRequestCallback>;
	readonly mediaQueries: FakeMediaQueryList[];
	readonly root: HTMLElement;
	documentListenerCount(type: string): number;
	dispatchDocument(type: string, event?: Event): void;
	dispatchWindow(type: string): void;
	flushFrames(timestamp?: number): void;
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
	readonly listenerCount: number;
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

function resolvedRowRequestStart(
	command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>,
	totalRows: bigint,
): bigint {
	const { origin, rowCount } = command.payload;
	switch (origin.kind) {
		case 'absolute':
			return BigInt(origin.row);
		case 'oldest':
			return BigInt(origin.offset);
		case 'tail': {
			const rowsBefore = BigInt(origin.rowsBefore);
			const end = totalRows - (rowsBefore < totalRows ? rowsBefore : totalRows);
			const count = BigInt(rowCount);
			return end - (count < end ? count : end);
		}
		case 'committed':
			throw new Error('A committed row request requires a Host-owned anchor');
	}
}

const hostEpoch = identifier(parseHostEpoch('11111111111111111111111111111111'));
const attachmentId = identifier(parseAttachmentId('22222222222222222222222222222222'));
const capabilityRevision = canonicalText('1');
const creationId = identifier(parseCreationId('33333333333333333333333333333333'));
const otherCreationId = identifier(parseCreationId('55555555555555555555555555555555'));
const terminalId = identifier(parseTerminalId('44444444444444444444444444444444'));
const otherAttachmentId = identifier(parseAttachmentId('66666666666666666666666666666666'));

function promptNavigationRow(
	index: bigint,
	promptClass: TerminalBufferRow['promptClass'],
): TerminalBufferRow {
	return {
		cells: null,
		identityAscii: true,
		index,
		mappings: [],
		promptClass,
		text: '',
		wrap: false,
		wrapContinuation: false,
	};
}

function terminalHintRow(index: bigint, text: string): TerminalBufferRow {
	return {
		cells: null,
		identityAscii: true,
		index,
		mappings: [],
		promptClass: RowPromptClass.none,
		text,
		wrap: false,
		wrapContinuation: false,
	};
}

afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
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
	activeScreen: TerminalFrame['activeScreen'] = ActiveScreen.primary,
	closeActivity: 'active' | 'idle' | 'unknown' = 'unknown',
	themeRevision = 0n,
	geometry: {
		readonly cellPixelHeight?: number;
		readonly cellPixelWidth?: number;
		readonly rows?: number;
	} = {},
): Uint8Array {
	const frame: TerminalFrame = {
		activeScreen,
		attachmentId,
		baseRevision,
		cellPixelHeight: geometry.cellPixelHeight ?? 18,
		cellPixelWidth: geometry.cellPixelWidth ?? 9,
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
		extensions:
			closeActivity === 'unknown'
				? []
				: [
						{
							data: new Uint8Array([TerminalCloseActivityCode[closeActivity]]),
							kind: TerminalFrameExtensionKind.closeActivity,
							required: false,
						},
					],
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
		rows: geometry.rows ?? 2,
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
		themeRevision,
		title: 'test',
	};
	const encoded = encodeTerminalFrame(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function themedTerminalFrame(
	sequence: bigint,
	revision: bigint,
	themeRevision: bigint,
	text: readonly [string, string],
): Uint8Array {
	return terminalFrame(
		'snapshot',
		sequence,
		revision,
		text,
		null,
		[0, 1],
		0n,
		interaction,
		{ column: 0, row: 0 },
		1n,
		2,
		ActiveScreen.primary,
		'unknown',
		themeRevision,
	);
}

function closeActivityTerminalFrame(
	sequence: bigint,
	revision: bigint,
	closeActivity: 'active' | 'idle' | 'unknown',
	options: {
		readonly scrollbackRows?: bigint;
		readonly themeRevision?: bigint;
	} = {},
): Uint8Array {
	return terminalFrame(
		'snapshot',
		sequence,
		revision,
		['a', 'b'],
		null,
		[0, 1],
		options.scrollbackRows ?? 0n,
		interaction,
		{ column: 0, row: 0 },
		1n,
		2,
		ActiveScreen.primary,
		closeActivity,
		options.themeRevision ?? 0n,
	);
}

const themeOverlays = {
	cursorAccentRgb: 0x01_02_03,
	searchActiveMatchBackgroundRgb: 0x04_05_06,
	searchActiveMatchForegroundRgb: 0x07_08_09,
	searchMatchBackgroundRgb: 0x0a_0b_0c,
	searchMatchForegroundRgb: 0x0d_0e_0f,
	selectionBackgroundRgb: 0x10_11_12,
	selectionForegroundRgb: 0x13_14_15,
} as const;

function renderRowWindow(
	sequence: bigint,
	revision: bigint,
	startRow: bigint,
	totalRows: bigint,
	text: readonly string[],
	options: {
		readonly generation?: bigint;
		readonly hyperlink?: string;
		readonly hyperlinkRowIndex?: number;
		readonly wrapContinuations?: readonly boolean[];
		readonly wraps?: readonly boolean[];
	} = {},
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
		generation: options.generation ?? 1n,
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
			wrap: options.wraps?.[index] ?? false,
			wrapContinuation: options.wrapContinuations?.[index] ?? false,
		})),
		rows: text.length,
		scrollbackRows: startRow,
		sequence,
		styles:
			options.hyperlink === undefined
				? [defaultStyle]
				: [defaultStyle, { ...defaultStyle, hyperlink: options.hyperlink }],
		themeRevision: 0n,
		title: 'history',
	};
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: 2,
		detail: TerminalRowWindowDetail.render,
		frame: decodedFrame,
		graphics: emptyGraphics,
		generation: options.generation ?? 1n,
		hostEpoch,
		revision,
		sequence,
		startRow,
		totalRows,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function encodedTextRowWindow(
	sequence: bigint,
	revision: bigint,
	startRow: bigint,
	totalRows: bigint,
	text: readonly string[],
): Uint8Array {
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: 2,
		detail: TerminalRowWindowDetail.text,
		generation: 1n,
		hostEpoch,
		revision,
		rows: text.map((value, index) => ({
			index: startRow + BigInt(index),
			mappings: [...value].map((_character, column) => ({
				byteOffset: column,
				column,
				utf16Offset: column,
				width: 1,
			})),
			promptClass: RowPromptClass.none,
			text: value,
			wrap: false,
			wrapContinuation: false,
		})),
		sequence,
		startRow,
		totalRows,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function searchWorkerWithMatch(
	row: number,
	onStart?: (
		message: Extract<
			Parameters<ProjectedSearchWorkerLike['postMessage']>[0],
			{ readonly kind: 'start' }
		>,
	) => void,
): ProjectedSearchWorkerLike {
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
				onStart?.(message);
				return;
			}
			if (!message.final || start === undefined) return;
			const response = {
				activeMatch: {
					end: { column: 1, row },
					start: { column: 0, row },
					text: 'needle',
				},
				generation: start.generation,
				kind: 'result',
				matches: [
					{
						end: { column: 1, row },
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
	let activeElement: FakeElement | null = null;
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
			hidden: false,
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
				if (type === 'focus') activeElement = element;
				else if (type === 'blur' && activeElement === element) activeElement = null;
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
			querySelectorAll: <T extends Element>(selector: string): NodeListOf<T> => {
				const matches: FakeElement[] = [];
				const visit = (candidate: FakeElement): void => {
					if (
						selector === '[data-projected-terminal-hint="true"]' &&
						candidate.dataset['projectedTerminalHint'] === 'true'
					) {
						matches.push(candidate);
					}
					for (const child of candidate.children) visit(child);
				};
				for (const child of element.children) visit(child);
				return matches as unknown as NodeListOf<T>;
			},
			remove: () => {
				const parent = element.parentElement;
				if (parent) parent.children.splice(parent.children.indexOf(element), 1);
				element.parentElement = null;
			},
			removeAttribute: (name: string) => element.attributes.delete(name),
			releasePointerCapture: (pointerId: number) => element.capturedPointers.delete(pointerId),
			replaceChildren: (...nodes: FakeElement[]) => {
				for (const child of element.children) child.parentElement = null;
				element.children.length = 0;
				for (const child of nodes) element.appendChild(child);
			},
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
		Node: Object,
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
				get listenerCount() {
					return listeners.size;
				},
				media,
				dispatchChange: () => {
					for (const listener of listeners) listener({ type: 'change' } as Event);
				},
			};
			mediaQueries.push(query);
			return {
				addEventListener: (_type: string, listener: EventListener) => listeners.add(listener),
				matches: media !== '(prefers-reduced-motion: reduce)',
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
		get activeElement() {
			return activeElement;
		},
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
		documentListenerCount: (type) => documentListeners.get(type)?.size ?? 0,
		dispatchDocument: (type, event = { type } as Event) => {
			for (const listener of documentListeners.get(type) ?? []) listener(event);
		},
		dispatchWindow: (type) => {
			for (const listener of windowListeners.get(type) ?? []) listener({ type } as Event);
		},
		frames,
		mediaQueries,
		root,
		flushFrames: (timestamp = 16) => {
			const callbacks = [...frames.values()];
			frames.clear();
			for (const callback of callbacks) callback(timestamp);
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
			const startContainer = startElement.firstChild;
			const endContainer = endElement.firstChild;
			if (startContainer === null || endContainer === null) {
				throw new Error('accessibility row text is unavailable');
			}
			const range = document.createRange();
			range.setStart(startContainer, startOffset);
			range.setEnd(endContainer, endOffset);
			domSelection.removeAllRanges();
			domSelection.addRange(range);
			selection = domSelection;
			for (const listener of documentListeners.get('selectionchange') ?? []) {
				listener({ type: 'selectionchange' } as Event);
			}
		},
	};
}

function historyScrollbarElements(browser: FakeBrowser): {
	readonly scrollbar: FakeElement;
	readonly thumb: FakeElement;
} {
	const scrollbar = (browser.root as unknown as FakeElement).children.find(
		(element) => element.dataset['projectedTerminalHistoryScrollbar'] === 'true',
	);
	const thumb = scrollbar?.children.find(
		(element) => element.dataset['projectedTerminalHistoryScrollbarThumb'] === 'true',
	);
	if (scrollbar === undefined || thumb === undefined) {
		throw new Error('projected history scrollbar is unavailable');
	}
	return { scrollbar, thumb };
}

function exitEvent(finalRevision: string): TerminalExited {
	return {
		exit: { code: 0, kind: 'exit-code' },
		finalRevision: finalRevision as TerminalExited['finalRevision'],
		identity: { creationId, hostEpoch, terminalId },
	};
}

function failureEvent(finalRevision: string): TerminalFailed {
	return {
		failure: { kind: 'pty-read', stage: 'read-output' },
		finalRevision: finalRevision as TerminalFailed['finalRevision'],
		identity: { creationId, hostEpoch, terminalId },
		process: { kind: 'observed', result: { code: 1, kind: 'exit-code' } },
	};
}

function dispatchComposition(
	input: FakeElement,
	type: 'compositionend' | 'compositionstart' | 'compositionupdate',
	data: string,
): void {
	input.dispatch(type, {
		data,
		preventDefault: () => undefined,
		stopPropagation: () => undefined,
		type,
	} as unknown as CompositionEvent);
}

function dispatchBeforeInput(
	input: FakeElement,
	data: string,
	isComposing = false,
	inputType = 'insertText',
): void {
	input.dispatch('beforeinput', {
		data,
		inputType,
		isComposing,
		preventDefault: () => undefined,
		stopPropagation: () => undefined,
		type: 'beforeinput',
	} as unknown as InputEvent);
}

function dispatchInput(input: FakeElement, inputType = 'insertText'): void {
	input.dispatch('input', {
		inputType,
		isComposing: false,
		stopPropagation: () => undefined,
		type: 'input',
	} as unknown as InputEvent);
}

function keyboardEvent(
	type: 'keydown' | 'keyup',
	code: string,
	key: string,
	keyCode: number,
	overrides: Partial<KeyboardEvent> = {},
): KeyboardEvent {
	return {
		altKey: false,
		code,
		ctrlKey: false,
		getModifierState: () => false,
		isComposing: false,
		key,
		keyCode,
		metaKey: false,
		preventDefault: () => undefined,
		repeat: false,
		shiftKey: false,
		stopPropagation: () => undefined,
		type,
		...overrides,
	} as unknown as KeyboardEvent;
}

function pointerEvent(
	type: 'lostpointercapture' | 'pointercancel' | 'pointerdown' | 'pointermove' | 'pointerup',
	button: number,
	buttons: number,
	clientX: number,
	clientY: number,
	overrides: Partial<PointerEvent> = {},
): PointerEvent {
	return {
		altKey: false,
		button,
		buttons,
		clientX,
		clientY,
		ctrlKey: false,
		metaKey: false,
		pointerId: 1,
		preventDefault: () => undefined,
		shiftKey: false,
		type,
		...overrides,
	} as unknown as PointerEvent;
}

function wheelEvent(
	deltaX: number,
	deltaY: number,
	overrides: Partial<WheelEvent> = {},
): WheelEvent {
	return {
		altKey: false,
		buttons: 0,
		clientX: 2,
		clientY: 2,
		ctrlKey: false,
		deltaMode: 0,
		deltaX,
		deltaY,
		metaKey: false,
		preventDefault: () => undefined,
		shiftKey: false,
		type: 'wheel',
		...overrides,
	} as unknown as WheelEvent;
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

	it('emits each scoped terminal clipboard write once and stops after disposal', () => {
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		const requests = vi.fn<Parameters<ProjectedTerminal['onClipboardWriteRequest']>[0]>();
		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		terminal.onClipboardWriteRequest(requests);
		terminal.onError(errors);

		terminal.notifyClipboardWriteRequest({
			attachmentId,
			sequence: canonicalText('2'),
			text: 'one',
		});
		terminal.notifyClipboardWriteRequest({
			attachmentId,
			sequence: canonicalText('2'),
			text: 'one',
		});
		terminal.notifyClipboardWriteRequest({
			attachmentId,
			sequence: canonicalText('3'),
			text: 'two',
		});
		terminal.notifyClipboardWriteRequest({
			attachmentId: otherAttachmentId,
			sequence: canonicalText('4'),
			text: 'wrong attachment',
		});

		expect(requests.mock.calls.map(([event]) => event.text)).toEqual(['one', 'two']);
		expect(errors).toHaveBeenCalledWith(expect.objectContaining({ operation: 'clipboard' }));
		terminal.dispose();
		terminal.notifyClipboardWriteRequest({
			attachmentId,
			sequence: canonicalText('5'),
			text: 'late',
		});
		expect(requests).toHaveBeenCalledTimes(2);
	});

	it('waits for the configured font and keeps the IME textarea font in sync', async () => {
		const browser = createFakeBrowser();
		const finishFontLoads: Array<() => void> = [];
		const fontLoad = vi.fn<() => Promise<FontFace[]>>(
			() =>
				new Promise<FontFace[]>((resolve) => {
					finishFontLoads.push(() => resolve([]));
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
		await vi.waitFor(() => expect(fontLoad).toHaveBeenCalledTimes(4));
		expect(fontLoad).toHaveBeenCalledWith('500 17px "Menlo", monospace', 'MW你🚀\uF120');
		expect(fontLoad).toHaveBeenCalledWith('italic 500 17px "Menlo", monospace', 'MW你🚀\uF120');
		expect(fontLoad).toHaveBeenCalledWith('700 17px "Menlo", monospace', 'MW你🚀\uF120');
		expect(fontLoad).toHaveBeenCalledWith('italic 700 17px "Menlo", monospace', 'MW你🚀\uF120');
		expect(commands).toEqual([]);
		expect(terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['early', '']))).toBe('committed');
		expect(browser.frames.size).toBe(0);
		expect(commands).toEqual([]);
		for (const finishFontLoad of finishFontLoads) finishFontLoad();
		await opening;
		expect(browser.frames.size).toBe(1);
		expect(commands[0]).toEqual({
			kind: 'frame-applied',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '1' },
		});

		expect(terminal.textarea?.style.fontFamily).toBe('"Menlo", monospace');
		expect(terminal.textarea?.style.fontSize).toBe('17px');
		expect(terminal.textarea?.style.fontWeight).toBe('500');
		expect(terminal.textarea?.style.opacity).toBe('0');
		expect(terminal.textarea?.style.caretColor).toBe('transparent');
		expect(terminal.textarea?.style.margin).toBe('0px');
		expect(terminal.textarea?.style.pointerEvents).toBeUndefined();
		expect(terminal.textarea?.style.zIndex).toBe('-5');
		expect((browser.root as unknown as FakeElement).children[1]?.style['zIndex']).toBe('5');
		expect((browser.root as unknown as FakeElement).children[2]?.style['userSelect']).toBe('text');
		terminal.dispose();
	});

	it('publishes IME Unicode input after a composition keydown', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'Period', '.', 229));
		input.value = '。';
		dispatchInput(input);
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands.filter((command) => command.kind === 'text-input').map((command) => command.payload),
		).toEqual([{ attachmentId, capabilityRevision, text: '。' }]);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		expect(input.value).toBe('');
		terminal.dispose();
	});

	it('defers an interim IME beforeinput until the native input settles', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'Period', '.', 229));
		dispatchBeforeInput(input, '.', false, 'insertText');
		expect(commands.filter((command) => command.kind === 'text-input')).toEqual([]);

		input.value = '。';
		dispatchInput(input);
		await vi.advanceTimersByTimeAsync(30);
		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['。']);
		expect(input.value).toBe('');
		terminal.dispose();
	});

	it('falls back to the settled textarea value after a composition keydown without input', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'Period', '.', 229));
		input.value = '。';
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['。']);
		expect(input.value).toBe('');
		terminal.dispose();
	});

	it('lets native IME punctuation settle before publishing its text fallback', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'Period', '.', 190));
		input.value = '。';
		dispatchInput(input);
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['。']);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		terminal.dispose();
	});

	it.each([
		{ code: 'Semicolon', key: ':', keyCode: 186, nativeText: '：' },
		{ code: 'Digit1', key: '!', keyCode: 49, nativeText: '！' },
	])('publishes shifted $code IME punctuation before releasing Shift', async (testCase) => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'ShiftLeft', 'Shift', 16, { shiftKey: true }),
		);
		input.dispatch(
			'keydown',
			keyboardEvent('keydown', testCase.code, testCase.key, testCase.keyCode, { shiftKey: true }),
		);
		input.value = testCase.nativeText;
		dispatchInput(input);
		input.dispatch(
			'keyup',
			keyboardEvent('keyup', testCase.code, testCase.key, testCase.keyCode, { shiftKey: true }),
		);
		input.dispatch('keyup', keyboardEvent('keyup', 'ShiftLeft', 'Shift', 16));
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands.flatMap((command) => {
				if (command.kind === 'text-input') return [`text:${command.payload.text}`];
				if (command.kind === 'key-input') {
					return [`key:${command.payload.action}:${command.payload.key}`];
				}
				return [];
			}),
		).toEqual(['key:press:ShiftLeft', `text:${testCase.nativeText}`, 'key:release:ShiftLeft']);
		terminal.dispose();
	});

	it('falls back to shifted punctuation text when the native IME produces no input', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'Semicolon', ':', 186, { shiftKey: true }));
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual([':']);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		terminal.dispose();
	});

	it('does not duplicate shifted punctuation when input arrives before keydown', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.value = '：';
		dispatchInput(input);
		input.dispatch('keydown', keyboardEvent('keydown', 'Semicolon', ':', 186, { shiftKey: true }));
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['：']);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		terminal.dispose();
	});

	it('keeps shifted letters, Space, and shortcut chords outside native text settlement', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;
		const events = [
			keyboardEvent('keydown', 'KeyA', 'A', 65, { shiftKey: true }),
			keyboardEvent('keydown', 'KeyB', 'Β', 66, { shiftKey: true }),
			keyboardEvent('keydown', 'Space', ' ', 32, { shiftKey: true }),
			keyboardEvent('keydown', 'Semicolon', ':', 186, { ctrlKey: true, shiftKey: true }),
			keyboardEvent('keydown', 'Slash', '?', 191, { altKey: true, shiftKey: true }),
			keyboardEvent('keydown', 'Quote', '"', 222, { metaKey: true, shiftKey: true }),
		];

		for (const event of events) input.dispatch('keydown', event);
		await vi.advanceTimersByTimeAsync(30);

		expect(commands.filter((command) => command.kind === 'text-input')).toEqual([]);
		expect(
			commands.flatMap((command) =>
				command.kind === 'key-input' && command.payload.action === KeyAction.press
					? [command.payload.key]
					: [],
			),
		).toEqual(['KeyA', 'KeyB', 'Space', 'Semicolon', 'Slash', 'Quote']);
		terminal.dispose();
	});

	it('falls back to punctuation text when the native IME produces no input', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'Period', '.', 190));
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['.']);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		terminal.dispose();
	});

	it('settles pending native text before a later physical key can overtake it', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'Digit1', '1', 49));
		input.value = '1';
		dispatchInput(input);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands.flatMap((command) => {
				if (command.kind === 'text-input') return [`text:${command.payload.text}`];
				if (command.kind === 'key-input' && command.payload.action === KeyAction.press) {
					return [`key:${command.payload.text}`];
				}
				return [];
			}),
		).toEqual(['text:1', 'key:a']);
		terminal.dispose();
	});

	it('settles the native fallback before a later physical key can overtake it', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'Digit1', '1', 49));
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands.flatMap((command) => {
				if (command.kind === 'text-input') return [`text:${command.payload.text}`];
				if (command.kind === 'key-input' && command.payload.action === KeyAction.press) {
					return [`key:${command.payload.text}`];
				}
				return [];
			}),
		).toEqual(['text:1', 'key:a']);
		terminal.dispose();
	});

	it('does not duplicate native punctuation when input arrives before keydown', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.value = '。';
		dispatchInput(input);
		input.dispatch('keydown', keyboardEvent('keydown', 'Period', '.', 190));
		await vi.advanceTimersByTimeAsync(30);

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['。']);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		terminal.dispose();
	});

	it('keeps native digit settlement ordered with intervening physical keys', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));
		input.dispatch('keydown', keyboardEvent('keydown', 'Digit1', '1', 49));
		input.value = '1';
		dispatchInput(input);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyB', 'b', 66));
		input.dispatch('keydown', keyboardEvent('keydown', 'Digit2', '2', 50));
		input.value = '2';
		dispatchInput(input);
		await vi.advanceTimersByTimeAsync(30);
		await Promise.resolve();

		expect(
			commands.flatMap((command) => {
				if (command.kind === 'text-input') return [command.payload.text];
				if (command.kind === 'key-input' && command.payload.action === KeyAction.press) {
					return [command.payload.text];
				}
				return [];
			}),
		).toEqual(['a', '1', 'b', '2']);
		terminal.dispose();
	});

	it('uses textarea content when compositionend does not provide committed data', async () => {
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
		dispatchComposition(input, 'compositionstart', '');
		input.value = '中文';
		dispatchComposition(input, 'compositionend', '');

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['中文']);
		expect(input.value).toBe('');
		terminal.dispose();
	});

	it('rolls a failed open back for same-instance retry without taking root ownership', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const root = browser.root as unknown as FakeElement;
		const existingChild = browser.document.createElement('div');
		browser.root.appendChild(existingChild);
		const openError = new Error('renderer initialization failed');
		const initGPU = vi.spyOn(WebGPURenderer.prototype, 'initGPU').mockRejectedValueOnce(openError);
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		terminal.onError(errors);

		await expect(terminal.open(browser.root)).rejects.toBe(openError);
		expect(terminal.phase).toBe('constructed');
		expect(terminal.canvas).toBeUndefined();
		expect(terminal.textarea).toBeUndefined();
		expect(root.children).toEqual([existingChild]);
		expect(browser.documentListenerCount('selectionchange')).toBe(0);
		expect(browser.documentListenerCount('pointermove')).toBe(0);
		expect(root.listeners.get('focusin')?.size ?? 0).toBe(0);
		expect(errors).toHaveBeenCalledWith({ error: openError, operation: 'open' });

		await terminal.open(browser.root);
		expect(terminal.phase).toBe('open');
		expect(root.children).toHaveLength(6);
		await expect(terminal.open(browser.root)).rejects.toThrow(
			'ProjectedTerminal cannot open from phase open',
		);
		expect(root.children).toHaveLength(6);
		expect(initGPU).toHaveBeenCalledTimes(2);

		terminal.dispose();
		expect(root.children).toEqual([existingChild]);
		expect(browser.documentListenerCount('selectionchange')).toBe(0);
		expect(browser.documentListenerCount('pointermove')).toBe(0);
		expect(root.listeners.get('focusin')?.size ?? 0).toBe(0);
	});

	it('publishes one exact theme revision across the staged frame and every renderer overlay', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const rendererPrepare = vi.spyOn(WebGPURenderer.prototype, 'prepareThemePresentation');
		const rendererCommit = vi.spyOn(WebGPURenderer.prototype, 'commitThemePresentation');
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		const publications: Array<{
			readonly rendererCommits: number;
			readonly themeRevision: bigint;
		}> = [];
		terminal.onMetadataChange((metadata) => {
			publications.push({
				rendererCommits: rendererCommit.mock.calls.length,
				themeRevision: metadata.themeRevision,
			});
		});
		await terminal.open(browser.root);
		expect(terminal.applyFrame(themedTerminalFrame(1n, 1n, 0n, ['old', '']))).toBe('committed');

		expect(terminal.prepareThemePresentation('operation-1', themeOverlays)).toEqual({
			kind: 'prepared',
		});
		const readiness = terminal.waitForThemePresentation('operation-1', 1n);
		expect(terminal.applyFrame(themedTerminalFrame(2n, 2n, 1n, ['new', '']))).toBe('staged');
		expect(terminal.metadata?.themeRevision).toBe(0n);
		expect(terminal.getBufferRowsSnapshot().revision).toBe(1n);
		expect(rendererPrepare).toHaveBeenCalledWith(themeOverlays);
		expect(rendererCommit).not.toHaveBeenCalled();

		await expect(readiness).resolves.toEqual({ kind: 'ready' });
		expect(terminal.canCommitThemePresentation('operation-1', 1n)).toBe(true);
		expect(terminal.commitThemePresentation('operation-1', 1n)).toBe(true);
		expect(rendererCommit).toHaveBeenCalledOnce();
		expect(terminal.metadata?.themeRevision).toBe(1n);
		expect(terminal.getBufferRowsSnapshot().revision).toBe(2n);
		expect(publications).toEqual([
			{ rendererCommits: 0, themeRevision: 0n },
			{ rendererCommits: 1, themeRevision: 1n },
		]);
		terminal.dispose();
	});

	it('keeps hidden theme frames staged and resumes only the latest revision', async () => {
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
		const publishedRevisions: bigint[] = [];
		terminal.onMetadataChange((metadata) => publishedRevisions.push(metadata.themeRevision));
		await terminal.open(browser.root);
		terminal.applyFrame(themedTerminalFrame(1n, 1n, 0n, ['visible', '']));
		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');

		terminal.prepareThemePresentation('operation-old', themeOverlays);
		const oldReadiness = terminal.waitForThemePresentation('operation-old', 1n);
		terminal.applyFrame(themedTerminalFrame(2n, 2n, 1n, ['hidden-old', '']));
		expect(terminal.metadata?.themeRevision).toBe(0n);

		terminal.prepareThemePresentation('operation-latest', themeOverlays);
		await expect(oldReadiness).resolves.toEqual({ kind: 'cancelled', reason: 'superseded' });
		const latestReadiness = terminal.waitForThemePresentation('operation-latest', 2n);
		terminal.applyFrame(themedTerminalFrame(3n, 3n, 2n, ['hidden-latest', '']));
		expect(terminal.metadata?.themeRevision).toBe(0n);

		browser.setVisibilityState('visible');
		browser.dispatchDocument('visibilitychange');
		await expect(latestReadiness).resolves.toEqual({ kind: 'ready' });
		expect(terminal.commitThemePresentation('operation-latest', 2n)).toBe(true);
		expect(terminal.metadata?.themeRevision).toBe(2n);
		expect(publishedRevisions).toEqual([0n, 2n]);
		terminal.dispose();
	});

	it('rejects renderer theme preparation without staging or changing the visible frame', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		vi.spyOn(WebGPURenderer.prototype, 'prepareThemePresentation').mockImplementationOnce(() => {
			throw new Error('injected renderer theme failure');
		});
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(themedTerminalFrame(1n, 1n, 0n, ['visible', '']));

		expect(terminal.prepareThemePresentation('operation-failed', themeOverlays)).toEqual({
			code: 'renderer-preparation-failed',
			kind: 'failed',
			retryable: true,
		});
		await expect(terminal.waitForThemePresentation('operation-failed', 1n)).resolves.toEqual({
			kind: 'cancelled',
			reason: 'superseded',
		});
		expect(terminal.metadata?.themeRevision).toBe(0n);
		expect(terminal.getBufferRowsSnapshot().revision).toBe(1n);
		terminal.dispose();
	});

	it('publishes one font-ready runtime presentation transaction on the existing attachment', async () => {
		const browser = createFakeBrowser();
		const fontLoad = vi.fn<(font: string, text?: string) => Promise<FontFace[]>>(async () => []);
		Object.defineProperty(browser.document, 'fonts', {
			configurable: true,
			value: { load: fontLoad },
		});
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const prepare = vi.spyOn(WebGPURenderer.prototype, 'preparePresentationConfiguration');
		const commit = vi.spyOn(WebGPURenderer.prototype, 'commitPresentationConfiguration');
		const setHoveredLinkRange = vi.spyOn(WebGPURenderer.prototype, 'setHoveredLinkRange');
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
			renderer: { devicePixelRatio: 2 },
			searchWorkerFactory: () => searchWorkerWithMatch(9),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
		browser.flushFrames();
		commands.length = 0;
		terminal.scrollLines(-1);
		const historyRequest = commands.find((command) => command.kind === 'read-rows');
		if (historyRequest?.kind !== 'read-rows') {
			throw new Error('runtime presentation fixture did not request history rows');
		}
		expect(historyRequest.payload).toMatchObject({
			origin: { kind: 'tail' },
			revisionPolicy: 'current-view',
		});
		const historyStart = resolvedRowRequestStart(historyRequest, 12n);
		terminal.applyRows(
			renderRowWindow(
				BigInt(historyRequest.payload.requestSequence),
				1n,
				historyStart,
				12n,
				Array.from({ length: historyRequest.payload.rowCount }, (_, index) => {
					const row = historyStart + BigInt(index);
					return row === 9n ? 'needle' : `row-${row}`;
				}),
			),
		);
		browser.flushFrames();
		const rows: TerminalBufferRow[] = Array.from({ length: 12 }, (_, index) => ({
			cells: null,
			identityAscii: true,
			index: BigInt(index),
			mappings: [],
			promptClass: RowPromptClass.none,
			text: index === 9 ? 'needle' : `row-${index}`,
			wrap: false,
			wrapContinuation: false,
		}));
		const readBufferRows = vi
			.spyOn(terminal, 'readBufferRows')
			.mockImplementation(async (request) => ({
				byteLength: rows.length,
				detail: request.detail,
				generation: request.generation,
				kind: 'rows',
				revision: request.revision,
				rows: rows.slice(Number(request.startRow), Number(request.startRow) + request.rowCount),
				startRow: request.startRow,
				totalRows: 12n,
			}));
		const searchEvents = vi.fn<Parameters<ProjectedTerminal['onSearchResults']>[0]>();
		terminal.onSearchResults(searchEvents);
		terminal.updateSearch('needle');
		await vi.waitFor(() =>
			expect(searchEvents.mock.calls.at(-1)?.[0]).toMatchObject({
				activeIndex: 0,
				matches: [{ start: { row: 9 } }],
				query: 'needle',
				state: 'complete',
			}),
		);
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: canonicalText('11') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: canonicalText('10') },
			state: 'active',
		});
		browser.selectAccessibilityText(0, 0, 0, 1);
		const accessibilitySelection = browser.document.getSelection();
		const canvasElement = terminal.canvas as unknown as FakeElement;
		canvasElement.dispatch('pointerdown', {
			altKey: false,
			button: 0,
			buttons: 1,
			clientX: 2,
			clientY: 2,
			ctrlKey: false,
			detail: 1,
			metaKey: false,
			pointerId: 71,
			preventDefault: () => undefined,
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);
		expect(canvasElement.capturedPointers.has(71)).toBe(true);
		const originalCanvas = terminal.canvas;
		const originalTextarea = terminal.textarea;
		const accessibilityRoot = (browser.root as unknown as FakeElement).children[2];
		commands.length = 0;
		setHoveredLinkRange.mockClear();
		fontLoad.mockReset();
		const fontResolvers: Array<() => void> = [];
		fontLoad.mockImplementation(
			() =>
				new Promise<FontFace[]>((resolve) => {
					fontResolvers.push(() => resolve([]));
				}),
		);
		const configuration = {
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			allowTransparency: true,
			fontFamily: 'Orchbee Runtime Mono',
			fontLigatures: false,
			fontMetricModifiers: { cellHeight: 2, cellWidth: 2 },
			fontSize: 18,
			fontWeight: 450,
			fontWeightBold: 700,
			paddingBalance: 'equal',
			terminalPadding: 12,
		} as const;

		const update = terminal.updatePresentationConfiguration(configuration);
		await vi.waitFor(() => expect(fontLoad).toHaveBeenCalledTimes(4));
		expect(prepare).not.toHaveBeenCalled();
		expect(commit).not.toHaveBeenCalled();
		expect(terminal.textarea?.style.fontFamily).toBe('monospace');
		for (const resolveFont of fontResolvers) resolveFont();

		await expect(update).resolves.toEqual({ configuration, kind: 'completed' });
		expect(prepare).toHaveBeenCalledOnce();
		expect(commit).toHaveBeenCalledOnce();
		expect(terminal.canvas).toBe(originalCanvas);
		expect(terminal.textarea).toBe(originalTextarea);
		expect((browser.root as unknown as FakeElement).children[2]).toBe(accessibilityRoot);
		expect(terminal.metadata).toMatchObject({ generation: 1n, revision: 1n });
		expect(terminal.selection).toMatchObject({ state: 'active' });
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.viewingHistory);
		expect(searchEvents.mock.calls.at(-1)?.[0]).toMatchObject({
			activeIndex: 0,
			matches: [{ start: { row: 9 } }],
			query: 'needle',
			state: 'complete',
		});
		expect(browser.document.getSelection()).toBe(accessibilitySelection);
		expect(canvasElement.capturedPointers.has(71)).toBe(false);
		expect(setHoveredLinkRange).toHaveBeenCalledWith(null);
		expect(terminal.textarea?.style).toMatchObject({
			fontFamily: 'Orchbee Runtime Mono',
			fontSize: '18px',
			fontVariantLigatures: 'none',
			fontWeight: '450',
		});
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toEqual([
			expect.objectContaining({
				payload: expect.objectContaining({
					cellPixelHeight: 36,
					cellPixelWidth: 20,
					columns: 69,
					rows: 18,
				}),
			}),
		]);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(0);
		expect(browser.frames.size).toBe(1);
		terminal.dispose();
		readBufferRows.mockRestore();
	});

	it('makes font preparation latest-request-wins and rolls back failure and disposal', async () => {
		const browser = createFakeBrowser();
		const fontLoad = vi.fn<(font: string, text?: string) => Promise<FontFace[]>>(async () => []);
		Object.defineProperty(browser.document, 'fonts', {
			configurable: true,
			value: { load: fontLoad },
		});
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
		fontLoad.mockReset();
		const prepareInvalid = vi.spyOn(WebGPURenderer.prototype, 'preparePresentationConfiguration');
		const commitInvalid = vi.spyOn(WebGPURenderer.prototype, 'commitPresentationConfiguration');
		prepareInvalid.mockClear();
		commitInvalid.mockClear();
		const originalFontFamily = terminal.textarea?.style.fontFamily;
		const originalCanvasWidth = terminal.canvas?.width;
		const originalCanvasHeight = terminal.canvas?.height;
		const throwingCandidate = new Proxy(
			{},
			{
				ownKeys: () => {
					throw new Error('configuration ownKeys failed');
				},
			},
		);
		await expect(terminal.updatePresentationConfiguration(throwingCandidate)).resolves.toEqual({
			code: 'invalid-configuration',
			field: 'configuration',
			kind: 'failed',
		});
		await expect(
			terminal.updatePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontMetricModifiers: { cellWidth: `${'9'.repeat(400)}%` },
			}),
		).resolves.toEqual({
			code: 'invalid-configuration',
			field: 'fontMetricModifiers',
			kind: 'failed',
		});
		expect(fontLoad).not.toHaveBeenCalled();
		expect(prepareInvalid).not.toHaveBeenCalled();
		expect(commitInvalid).not.toHaveBeenCalled();
		expect(terminal.textarea?.style.fontFamily).toBe(originalFontFamily);
		expect(terminal.canvas?.width).toBe(originalCanvasWidth);
		expect(terminal.canvas?.height).toBe(originalCanvasHeight);
		prepareInvalid.mockRestore();
		commitInvalid.mockRestore();
		const pendingFonts: Array<{
			reject(error: unknown): void;
			resolve(value: FontFace[]): void;
		}> = [];
		fontLoad.mockImplementation(
			() =>
				new Promise<FontFace[]>((resolve, reject) => {
					pendingFonts.push({ reject, resolve });
				}),
		);
		const firstConfiguration = {
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'First Runtime Mono',
		} as const;
		const secondConfiguration = {
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'Second Runtime Mono',
		} as const;

		const first = terminal.updatePresentationConfiguration(firstConfiguration);
		await vi.waitFor(() => expect(pendingFonts).toHaveLength(4));
		const second = terminal.updatePresentationConfiguration(secondConfiguration);
		await expect(first).resolves.toEqual({ kind: 'cancelled', reason: 'superseded' });
		await vi.waitFor(() => expect(pendingFonts).toHaveLength(8));
		for (const pending of pendingFonts.slice(4, 8)) pending.resolve([]);
		await expect(second).resolves.toMatchObject({ kind: 'completed' });
		expect(terminal.textarea?.style.fontFamily).toBe('Second Runtime Mono');

		const failed = terminal.updatePresentationConfiguration({
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'Failed Runtime Mono',
		});
		await vi.waitFor(() => expect(pendingFonts).toHaveLength(12));
		pendingFonts[8]?.reject(new Error('font unavailable'));
		for (const pending of pendingFonts.slice(9, 12)) pending.resolve([]);
		await expect(failed).resolves.toEqual({ code: 'font-preparation-failed', kind: 'failed' });
		expect(terminal.textarea?.style.fontFamily).toBe('Second Runtime Mono');

		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		const errorSubscription = terminal.onError(errors);
		const rendererPreparation = vi
			.spyOn(WebGPURenderer.prototype, 'preparePresentationConfiguration')
			.mockImplementationOnce(() => {
				throw new Error('renderer measurement failed');
			});
		const rendererFailed = terminal.updatePresentationConfiguration({
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'Renderer Failed Runtime Mono',
		});
		await vi.waitFor(() => expect(pendingFonts).toHaveLength(16));
		for (const pending of pendingFonts.slice(12, 16)) pending.resolve([]);
		await expect(rendererFailed).resolves.toEqual({
			code: 'renderer-preparation-failed',
			kind: 'failed',
		});
		expect(terminal.textarea?.style.fontFamily).toBe('Second Runtime Mono');
		expect(errors).not.toHaveBeenCalled();
		errorSubscription.dispose();
		rendererPreparation.mockRestore();

		const disposed = terminal.updatePresentationConfiguration({
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'Disposed Runtime Mono',
		});
		await vi.waitFor(() => expect(pendingFonts).toHaveLength(20));
		terminal.dispose();
		await expect(disposed).resolves.toEqual({ kind: 'cancelled', reason: 'disposed' });
	});

	it('retains a committed presentation through post-commit render and resize recovery', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let failResize = false;
		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		const terminal = new ProjectedTerminal({
			attachmentId,
			canResize: true,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					return failResize && command.kind === 'resize-terminal'
						? Promise.reject(new Error('resize transport failed'))
						: undefined;
				},
			},
			terminalId,
		});
		terminal.onError(errors);
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		browser.flushFrames();
		commands.length = 0;
		failResize = true;
		const originalCommit = WebGPURenderer.prototype.commitPresentationConfiguration;
		vi.spyOn(WebGPURenderer.prototype, 'commitPresentationConfiguration').mockImplementation(
			function (this: WebGPURenderer, prepared): void {
				originalCommit.call(this, prepared);
				throw new Error('renderer post-commit failed');
			},
		);
		const configuration = {
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'Committed Recovery Mono',
			fontMetricModifiers: { cellHeight: 3, cellWidth: 3 },
			fontSize: 18,
		} as const;

		await expect(terminal.updatePresentationConfiguration(configuration)).resolves.toEqual({
			configuration,
			kind: 'completed',
		});
		expect(terminal.textarea?.style.fontFamily).toBe('Committed Recovery Mono');
		expect(terminal.textarea?.style.fontSize).toBe('18px');
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(1);
		await vi.waitFor(() => {
			expect(errors.mock.calls.map(([event]) => event.operation)).toEqual(
				expect.arrayContaining(['render', 'resize']),
			);
		});
		expect(browser.frames.size).toBe(1);
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
		input.style['left'] = '0px';
		input.style['top'] = '0px';
		input.style['width'] = '1px';
		input.style['height'] = '1px';
		input.style['lineHeight'] = '1px';
		let anchorBeforePreedit: Record<string, string | undefined> | undefined;

		input.dispatch('compositionstart', {
			get data() {
				anchorBeforePreedit = {
					height: input.style['height'],
					left: input.style['left'],
					lineHeight: input.style['lineHeight'],
					top: input.style['top'],
					width: input.style['width'],
				};
				return '你';
			},
			type: 'compositionstart',
		} as unknown as CompositionEvent);

		expect(anchorBeforePreedit).toEqual({
			height: '16px',
			left: '12px',
			lineHeight: '16px',
			top: '16px',
			width: '8px',
		});
		expect(input.style['left']).toBe('4px');
		expect(input.style['top']).toBe('16px');
		expect(input.style['width']).toBe('16px');
		expect(input.style['height']).toBe('16px');
		expect(input.style['lineHeight']).toBe('16px');
		browser.flushFrames();
		await expect(
			terminal.updatePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontFamily: 'Runtime IME Mono',
				fontMetricModifiers: { cellHeight: 4, cellWidth: 4 },
				fontSize: 19,
			}),
		).resolves.toMatchObject({ kind: 'completed' });
		expect(input.style).toMatchObject({
			fontFamily: 'Runtime IME Mono',
			fontSize: '19px',
			height: '20px',
			left: '10px',
			lineHeight: '20px',
			top: '20px',
			width: '24px',
		});
		expect(browser.frames.size).toBe(1);
		terminal.dispose();
	});

	it('anchors native IME geometry to an injected presentation before a live frame arrives', async () => {
		const sourceBrowser = createFakeBrowser();
		vi.stubGlobal('document', sourceBrowser.document);
		vi.stubGlobal('window', sourceBrowser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const sourceTerminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		await sourceTerminal.open(sourceBrowser.root);
		sourceTerminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, interaction, {
				column: 1,
				row: 1,
			}),
		);
		sourceBrowser.flushFrames();
		const presentationSnapshot = sourceTerminal.capturePresentationSnapshot();
		if (presentationSnapshot === null) throw new Error('Expected a painted presentation snapshot');
		sourceTerminal.dispose();

		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			presentationSnapshot,
			terminalId,
		});
		await terminal.open(browser.root);
		expect(terminal.metadata).toBeNull();
		const expectedAnchor = {
			height: '16px',
			left: '12px',
			lineHeight: '16px',
			top: '16px',
			width: '8px',
		};
		expect(terminal.textarea?.style).toMatchObject(expectedAnchor);
		terminal.focus();
		expect(terminal.textarea?.style).toMatchObject(expectedAnchor);
		terminal.dispose();
	});

	it.each(['ShiftLeft', 'ShiftRight'] as const)(
		'leaves macOS %s switching native while resynchronizing its release anchor',
		async (shiftCode) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', { platform: 'MacIntel' });
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
				terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, interaction, {
					column: 1,
					row: 1,
				}),
			);
			terminal.notifySelectionChanged({
				attachmentId,
				end: { column: 0, row: canonicalText('0') },
				preview: 'a',
				rectangle: false,
				start: { column: 0, row: canonicalText('0') },
				state: 'active',
			});
			expect(browser.document.getSelection()?.rangeCount).toBe(0);
			const input = terminal.textarea as unknown as FakeElement;
			input.style['left'] = '0px';
			input.style['top'] = '0px';
			input.style['width'] = '1px';
			input.style['height'] = '1px';
			input.style['lineHeight'] = '1px';
			const rootBounds = vi.spyOn(browser.root, 'getBoundingClientRect');
			const canvas = terminal.canvas;
			if (canvas === undefined) throw new Error('Projected terminal canvas is unavailable');
			const canvasBounds = vi.spyOn(canvas, 'getBoundingClientRect');
			const inputBounds = vi.spyOn(input, 'getBoundingClientRect');
			let canvasOffset = 0;
			rootBounds.mockImplementation(
				() =>
					({
						bottom: 460,
						height: 360,
						left: 100,
						right: 820,
						top: 100,
						width: 720,
						x: 100,
						y: 100,
						toJSON: () => ({}),
					}) as DOMRect,
			);
			canvasBounds.mockImplementation(
				() =>
					({
						bottom: 460,
						height: 360,
						left: 100 + canvasOffset,
						right: 820 + canvasOffset,
						top: 100,
						width: 720,
						x: 100 + canvasOffset,
						y: 100,
						toJSON: () => ({}),
					}) as DOMRect,
			);
			const keyDownPreventDefault = vi.fn<() => void>();
			const keyDownStopPropagation = vi.fn<() => void>();
			const keyUpPreventDefault = vi.fn<() => void>();
			const keyUpStopPropagation = vi.fn<() => void>();

			input.dispatch(
				'keydown',
				keyboardEvent('keydown', shiftCode, 'Shift', 16, {
					preventDefault: keyDownPreventDefault,
					shiftKey: true,
					stopPropagation: keyDownStopPropagation,
				}),
			);
			expect(input.style).toMatchObject({
				height: '16px',
				left: '12px',
				lineHeight: '16px',
				top: '16px',
				width: '8px',
			});
			canvasOffset = 20;
			input.dispatch(
				'keyup',
				keyboardEvent('keyup', shiftCode, 'Shift', 16, {
					preventDefault: keyUpPreventDefault,
					stopPropagation: keyUpStopPropagation,
				}),
			);

			expect(input.style).toMatchObject({ left: '32px', top: '16px' });
			expect(inputBounds).toHaveBeenCalledTimes(2);
			expect(rootBounds).toHaveBeenCalledTimes(2);
			expect(canvasBounds).toHaveBeenCalledTimes(2);
			expect(keyDownPreventDefault).not.toHaveBeenCalled();
			expect(keyUpPreventDefault).not.toHaveBeenCalled();
			expect(keyDownStopPropagation).toHaveBeenCalledOnce();
			expect(keyUpStopPropagation).toHaveBeenCalledOnce();
			expect(
				commands.flatMap((command) =>
					command.kind === 'key-input' ? [`${command.payload.action}:${command.payload.key}`] : [],
				),
			).toEqual([`press:${shiftCode}`, `release:${shiftCode}`]);
			terminal.dispose();
		},
	);

	it('continues to own Shift defaults outside macOS', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', { platform: 'Linux x86_64' });
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		await terminal.open(browser.root);
		const input = terminal.textarea as unknown as FakeElement;
		const keyDownPreventDefault = vi.fn<() => void>();
		const keyUpPreventDefault = vi.fn<() => void>();

		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'ShiftLeft', 'Shift', 16, {
				preventDefault: keyDownPreventDefault,
				shiftKey: true,
			}),
		);
		input.dispatch(
			'keyup',
			keyboardEvent('keyup', 'ShiftLeft', 'Shift', 16, {
				preventDefault: keyUpPreventDefault,
			}),
		);

		expect(keyDownPreventDefault).toHaveBeenCalledOnce();
		expect(keyUpPreventDefault).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('restores the focused native IME anchor when the window regains focus', async () => {
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
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, interaction, {
				column: 1,
				row: 1,
			}),
		);
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		browser.dispatchWindow('blur');
		input.style['left'] = '0px';
		input.style['top'] = '0px';

		browser.dispatchWindow('focus');

		expect(input.style).toMatchObject({ left: '12px', top: '16px' });
		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false, true]);
		terminal.dispose();
	});

	it('reuses measured IME surface offsets across terminal frames', async () => {
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
		const canvas = terminal.canvas;
		if (canvas === undefined) throw new Error('Projected terminal canvas is unavailable');
		const canvasBounds = vi.spyOn(canvas, 'getBoundingClientRect');

		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		expect(canvasBounds).toHaveBeenCalledOnce();
		browser.flushFrames();
		canvasBounds.mockClear();

		terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['c', 'd'], 1n));
		expect(canvasBounds).not.toHaveBeenCalled();

		browser.dispatchWindow('resize');
		expect(canvasBounds).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('moves the native IME anchor with a wrapped preedit caret', async () => {
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
				{ column: 3, row: 0 },
				1n,
				4,
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

		expect(input.style).toMatchObject({
			height: '16px',
			left: '12px',
			lineHeight: '16px',
			top: '16px',
			width: '16px',
		});
		terminal.dispose();
	});

	it('revokes composition publication when reset repairs active input', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const setPreedit = vi.spyOn(WebGPURenderer.prototype, 'setPreedit');
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
		const input = terminal.textarea as unknown as FakeElement & {
			selectionEnd: number;
			selectionStart: number;
		};
		input.value = '候';
		input.selectionStart = 1;
		input.selectionEnd = 1;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		dispatchComposition(input, 'compositionstart', '候');

		terminal.reset();
		expect(input.value).toBe('');
		expect(setPreedit).toHaveBeenLastCalledWith([]);
		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true]);
		const repairedPreeditCallCount = setPreedit.mock.calls.length;
		dispatchComposition(input, 'compositionupdate', '候选');
		expect(setPreedit).toHaveBeenCalledTimes(repairedPreeditCallCount);
		dispatchComposition(input, 'compositionend', '候选');
		dispatchBeforeInput(input, '候选', false, 'insertFromComposition');
		expect(commands.filter((command) => command.kind === 'text-input')).toHaveLength(0);

		dispatchComposition(input, 'compositionstart', '前');
		terminal.reset();
		dispatchBeforeInput(input, '前', true);
		dispatchComposition(input, 'compositionend', '前');
		expect(commands.filter((command) => command.kind === 'text-input')).toHaveLength(0);

		dispatchComposition(input, 'compositionstart', '新');
		dispatchComposition(input, 'compositionend', '新');
		dispatchComposition(input, 'compositionend', '新');
		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['新']);
		terminal.dispose();
	});

	it('commits composition exactly once in both beforeinput and compositionend orders', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const setPreedit = vi.spyOn(WebGPURenderer.prototype, 'setPreedit');
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
		const input = terminal.textarea as unknown as FakeElement;

		dispatchComposition(input, 'compositionstart', '先');
		dispatchBeforeInput(input, '先');
		expect(setPreedit).toHaveBeenLastCalledWith([]);
		const committedPreeditCallCount = setPreedit.mock.calls.length;
		dispatchComposition(input, 'compositionupdate', '过期先');
		expect(setPreedit).toHaveBeenCalledTimes(committedPreeditCallCount);
		dispatchComposition(input, 'compositionend', '先');
		dispatchComposition(input, 'compositionend', '先');

		dispatchComposition(input, 'compositionstart', '后');
		dispatchComposition(input, 'compositionend', '后');
		dispatchBeforeInput(input, '后', false, 'insertFromComposition');
		dispatchComposition(input, 'compositionend', '后');

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['先', '后']);
		expect(input.value).toBe('');
		terminal.dispose();
	});

	it('preserves composition settlement across repeated repair without suppressing independent input', async () => {
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

		dispatchComposition(input, 'compositionstart', '正常');
		dispatchComposition(input, 'compositionend', '正常');
		terminal.reset();
		dispatchBeforeInput(input, '正常', false, 'insertFromComposition');

		dispatchComposition(input, 'compositionstart', '取消');
		terminal.reset();
		dispatchComposition(input, 'compositionend', '取消');
		terminal.reset();
		dispatchBeforeInput(input, '取消', false, 'insertFromComposition');

		dispatchBeforeInput(input, '取消', false, 'insertText');
		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['正常', '取消']);
		terminal.dispose();
	});

	it('supersedes cancelled native composition and keeps independent input progressing', async () => {
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

		dispatchComposition(input, 'compositionstart', '旧');
		terminal.reset();
		dispatchComposition(input, 'compositionstart', '新');
		dispatchComposition(input, 'compositionupdate', '新的');
		dispatchComposition(input, 'compositionend', '新的');

		dispatchComposition(input, 'compositionstart', '遗留');
		terminal.reset();
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		dispatchBeforeInput(input, 'mobile', false, 'insertText');
		dispatchBeforeInput(input, '遗留', false, 'insertFromComposition');
		dispatchBeforeInput(input, '遗留', false, 'insertText');
		dispatchComposition(input, 'compositionstart', '键盘遗留');
		terminal.reset();
		input.dispatch('keydown', {
			altKey: false,
			code: 'KeyB',
			ctrlKey: false,
			getModifierState: () => false,
			isComposing: false,
			key: 'b',
			keyCode: 66,
			metaKey: false,
			preventDefault: () => undefined,
			repeat: false,
			shiftKey: false,
			stopPropagation: () => undefined,
			type: 'keydown',
		} as unknown as KeyboardEvent);

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['新的', 'mobile', '遗留']);
		expect(
			commands.some(
				(command) =>
					command.kind === 'key-input' &&
					command.payload.action === KeyAction.press &&
					command.payload.key === 'KeyB',
			),
		).toBe(true);
		terminal.dispose();
	});

	it.each(['beforeinput-first', 'compositionend-first'])(
		'publishes composition atomically before reentrant %s settlement',
		async (order) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', {});
			const commands: ProjectedTerminalCommand[] = [];
			let input: FakeElement | undefined;
			let reentered = false;
			const terminal = new ProjectedTerminal({
				attachmentId,
				creationId,
				hostEpoch,
				labels: { input: 'input', screen: 'screen' },
				port: {
					send: (command) => {
						commands.push(command);
						if (command.kind !== 'text-input' || input === undefined || reentered) return;
						reentered = true;
						if (order === 'beforeinput-first') {
							dispatchComposition(input, 'compositionend', '原子');
						} else {
							dispatchBeforeInput(input, '原子', false, 'insertFromComposition');
						}
					},
				},
				terminalId,
			});
			await terminal.open(browser.root);
			input = terminal.textarea as unknown as FakeElement;
			dispatchComposition(input, 'compositionstart', '原子');
			if (order === 'beforeinput-first') dispatchBeforeInput(input, '原子');
			else dispatchComposition(input, 'compositionend', '原子');

			expect(
				commands
					.filter((command) => command.kind === 'text-input')
					.map((command) => command.payload.text),
			).toEqual(['原子']);
			terminal.dispose();
		},
	);

	it.each([
		['insertLineBreak', ''],
		['insertParagraph', ''],
		['deleteContentBackward', ''],
		['deleteContentForward', ''],
		['insertFromPaste', 'blocked paste'],
	])(
		'rejects cancelled composition-origin %s before its special input branch',
		async (inputType, data) => {
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
			dispatchComposition(input, 'compositionstart', '取消');
			terminal.reset();
			input.dispatch('beforeinput', {
				data,
				inputType,
				isComposing: true,
				preventDefault: () => undefined,
				stopPropagation: () => undefined,
				type: 'beforeinput',
			} as unknown as InputEvent);
			await Promise.resolve();

			expect(
				commands.filter(
					(command) => command.kind === 'key-input' || command.kind === 'paste-input',
				),
			).toEqual([]);
			terminal.dispose();
		},
	);

	it('records key correlation before a synchronous press send can reenter beforeinput', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'key-input' ||
						command.payload.action !== KeyAction.press
					) {
						return;
					}
					reentered = true;
					dispatchBeforeInput(input, 'a');
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		expect(commands.filter((command) => command.kind === 'text-input')).toEqual([]);
		expect(
			commands.filter(
				(command) => command.kind === 'key-input' && command.payload.action === KeyAction.press,
			),
		).toHaveLength(1);
		terminal.dispose();
	});

	it('leaves unclaimed Apple Command chords to native application handling', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', { platform: 'MacIntel' });
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
		const preventDefault = vi.fn<() => void>();
		const stopPropagation = vi.fn<() => void>();

		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'KeyQ', 'q', 81, {
				metaKey: true,
				preventDefault,
				stopPropagation,
			}),
		);
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyQ', 'q', 81, { metaKey: true }));

		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		expect(preventDefault).not.toHaveBeenCalled();
		expect(stopPropagation).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('resolves live macOS Option-as-Alt policy from the pressed physical side', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', { platform: 'MacIntel' });
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			inputConfiguration: { optionAsAlt: 'left' },
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
		const sendChord = (side: 'AltLeft' | 'AltRight'): void => {
			input.dispatch('keydown', keyboardEvent('keydown', side, 'Alt', 18, { altKey: true }));
			input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', '∫', 66, { altKey: true }));
			input.dispatch('keyup', keyboardEvent('keyup', 'KeyB', '∫', 66, { altKey: true }));
			input.dispatch('keyup', keyboardEvent('keyup', side, 'Alt', 18));
		};

		sendChord('AltLeft');
		sendChord('AltRight');
		terminal.setOptionAsAltMode('right');
		sendChord('AltRight');
		terminal.setOptionAsAltMode('both');
		sendChord('AltLeft');
		terminal.setOptionAsAltMode('off');
		sendChord('AltRight');

		expect(
			commands.flatMap((command) =>
				command.kind === 'key-input' &&
				command.payload.action === KeyAction.press &&
				command.payload.key === 'KeyB'
					? [command.payload.optionAsAlt]
					: [],
			),
		).toEqual([true, false, true, true, false]);
		terminal.dispose();
	});

	it('retains Meta-modified terminal input on non-Apple platforms', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', { platform: 'Linux x86_64' });
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
		const preventDefault = vi.fn<() => void>();

		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'KeyQ', 'q', 81, { metaKey: true, preventDefault }),
		);

		expect(
			commands.filter(
				(command) => command.kind === 'key-input' && command.payload.action === KeyAction.press,
			),
		).toEqual([
			expect.objectContaining({
				kind: 'key-input',
				payload: expect.objectContaining({
					key: 'KeyQ',
					modifiers: expect.objectContaining({ meta: true }),
				}),
			}),
		]);
		expect(preventDefault).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('normalizes legacy OS modifier codes to Ghostty Meta keys', async () => {
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

		for (const code of ['OSLeft', 'OSRight']) {
			input.dispatch('keydown', keyboardEvent('keydown', code, 'Meta', 91, { metaKey: true }));
			input.dispatch('keyup', keyboardEvent('keyup', code, 'Meta', 91));
		}

		const keys = commands.flatMap((command) =>
			command.kind === 'key-input' ? [command.payload] : [],
		);
		expect(keys).toHaveLength(4);
		expect(keys.map(({ action, key }) => [action, key])).toEqual([
			[KeyAction.press, 'MetaLeft'],
			[KeyAction.release, 'MetaLeft'],
			[KeyAction.press, 'MetaRight'],
			[KeyAction.release, 'MetaRight'],
		]);
		expect(keys[2]?.modifiers).toMatchObject({ meta: true, metaRight: true });
		expect(keys[3]?.modifiers).toMatchObject({ meta: false, metaRight: false });
		terminal.dispose();
	});

	it('keeps Shift-only keypad PageUp and PageDown as local history paging', async () => {
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
		const scrollPages = vi.spyOn(terminal, 'scrollPages');
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'NumpadPageUp', 'PageUp', 33, { shiftKey: true }),
		);
		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'NumpadPageDown', 'PageDown', 34, { shiftKey: true }),
		);

		expect(scrollPages.mock.calls.map(([amount]) => amount)).toEqual([-1, 1]);
		expect(commands.some((command) => command.kind === 'key-input')).toBe(false);
		terminal.dispose();
	});

	it.each([
		{ expectedInput: 'key:KeyA', source: 'physical key' },
		{ expectedInput: 'text', source: 'committed beforeinput text' },
		{ expectedInput: 'key:Enter', source: 'synthetic beforeinput key' },
		{ expectedInput: undefined, source: 'composition activation' },
		{ expectedInput: 'paste', source: 'paste' },
		{ expectedInput: 'mouse', source: 'application mouse' },
	])(
		'returns to bottom and clears selection before $source input',
		async ({ expectedInput, source }) => {
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
				terminalFrame(
					'snapshot',
					1n,
					1n,
					['a', 'b'],
					null,
					[0, 1],
					10n,
					source === 'application mouse' ? { ...interaction, mouseAny: true } : interaction,
				),
			);
			terminal.scrollLines(-1);
			terminal.notifySelectionChanged({
				attachmentId,
				end: { column: 1, row: canonicalText('10') },
				preview: 'ab',
				rectangle: false,
				start: { column: 0, row: canonicalText('10') },
				state: 'active',
			});
			commands.length = 0;
			const input = terminal.textarea as unknown as FakeElement;

			switch (source) {
				case 'physical key': {
					input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
					break;
				}
				case 'committed beforeinput text': {
					dispatchBeforeInput(input, 'a');
					break;
				}
				case 'synthetic beforeinput key': {
					dispatchBeforeInput(input, '', false, 'insertLineBreak');
					break;
				}
				case 'composition activation': {
					dispatchComposition(input, 'compositionstart', 'a');
					break;
				}
				case 'paste': {
					await terminal.paste('a');
					break;
				}
				case 'application mouse': {
					(terminal.canvas as unknown as FakeElement).dispatch(
						'pointerdown',
						pointerEvent('pointerdown', 0, 1, 2, 2),
					);
					break;
				}
			}

			const milestones = commands.flatMap((command) => {
				if (
					command.kind === 'set-history-demand' &&
					command.payload.demand === 'following-bottom'
				) {
					return ['history-bottom'];
				}
				if (command.kind === 'selection-command' && command.payload.command === 'clear') {
					return ['selection-clear'];
				}
				if (command.kind === 'key-input' && command.payload.action !== KeyAction.release) {
					return [`key:${command.payload.key}`];
				}
				if (command.kind === 'text-input') return ['text'];
				if (command.kind === 'paste-input') return ['paste'];
				if (command.kind === 'mouse-input') return ['mouse'];
				return [];
			});
			expect(milestones).toEqual([
				'history-bottom',
				'selection-clear',
				...(expectedInput === undefined ? [] : [expectedInput]),
			]);
			terminal.dispose();
		},
	);

	it('preserves history and selection while publishing physical modifier keys', async () => {
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
		terminal.scrollLines(-1);
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: canonicalText('10') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: canonicalText('10') },
			state: 'active',
		});
		const historyPhaseBeforeModifiers = terminal.historyPhase;
		commands.length = 0;
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'ControlLeft', 'Control', 17, { ctrlKey: true }),
		);
		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'ShiftLeft', 'Shift', 16, {
				ctrlKey: true,
				shiftKey: true,
			}),
		);

		expect(
			commands.flatMap((command) =>
				command.kind === 'key-input' ? [`${command.payload.action}:${command.payload.key}`] : [],
			),
		).toEqual(['press:ControlLeft', 'press:ShiftLeft']);
		expect(
			commands.some(
				(command) =>
					command.kind === 'set-history-demand' && command.payload.demand === 'following-bottom',
			),
		).toBe(false);
		expect(commands.some((command) => command.kind === 'selection-command')).toBe(false);
		expect(terminal.historyPhase).toBe(historyPhaseBeforeModifiers);
		expect(terminal.getSelectionRange()).toEqual({
			end: { column: 1, row: 10n },
			rectangle: false,
			start: { column: 0, row: 10n },
		});
		terminal.dispose();
	});

	it('preserves history and selection while reporting buttonless application mouse motion', async () => {
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
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n, {
				...interaction,
				mouseAny: true,
			}),
		);
		terminal.scrollLines(-1);
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: canonicalText('10') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: canonicalText('10') },
			state: 'active',
		});
		const historyPhaseBeforeMotion = terminal.historyPhase;
		commands.length = 0;

		(terminal.canvas as unknown as FakeElement).dispatch(
			'pointermove',
			pointerEvent('pointermove', 0, 0, 2, 2),
		);

		const milestones = commands.flatMap((command) => {
			if (command.kind === 'set-history-demand' && command.payload.demand === 'following-bottom') {
				return ['history-bottom'];
			}
			if (command.kind === 'selection-command' && command.payload.command === 'clear') {
				return ['selection-clear'];
			}
			if (command.kind === 'mouse-input') {
				return [`mouse:${command.payload.action}:${String(command.payload.button)}`];
			}
			return [];
		});
		expect(milestones).toEqual([`mouse:${MouseAction.motion}:null`]);
		expect(terminal.historyPhase).toBe(historyPhaseBeforeMotion);
		expect(terminal.getSelectionRange()).toEqual({
			end: { column: 1, row: 10n },
			rectangle: false,
			start: { column: 0, row: 10n },
		});
		terminal.dispose();
	});

	it('keeps a newly established selection while releasing an already published modifier', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('keydown', keyboardEvent('keydown', 'MetaLeft', 'Meta', 91, { metaKey: true }));
		terminal.selectAll();
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: canonicalText('1') },
			preview: 'a\nb',
			rectangle: false,
			start: { column: 0, row: canonicalText('0') },
			state: 'active',
		});
		commands.length = 0;

		input.dispatch('keyup', keyboardEvent('keyup', 'MetaLeft', 'Meta', 91));

		expect(commands).toEqual([
			expect.objectContaining({
				kind: 'key-input',
				payload: expect.objectContaining({ action: KeyAction.release, key: 'MetaLeft' }),
			}),
		]);
		expect(terminal.getSelectionRange()).toEqual({
			end: { column: 1, row: 1n },
			rectangle: false,
			start: { column: 0, row: 0n },
		});
		terminal.dispose();
	});

	it('preserves W3C key strings and consumes AltGraph only for printable text', async () => {
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

		for (const [code, key] of [
			['LegacySupplementary', '𨧂'],
			['LegacyCombining', 'ḍ̇'],
		] as const) {
			input.dispatch('keydown', keyboardEvent('keydown', code, key, 0));
			input.dispatch('keyup', keyboardEvent('keyup', code, key, 0));
		}
		const altGraphState = (modifier: string): boolean => modifier === 'AltGraph';
		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'AltRight', 'AltGraph', 18, {
				altKey: true,
				ctrlKey: true,
				getModifierState: altGraphState,
			}),
		);
		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'KeyQ', '@', 81, {
				altKey: true,
				ctrlKey: true,
				getModifierState: altGraphState,
			}),
		);

		const presses = commands.flatMap((command) =>
			command.kind === 'key-input' && command.payload.action === KeyAction.press
				? [command.payload]
				: [],
		);
		expect(presses.slice(0, 2).map(({ key, text }) => [key, text])).toEqual([
			['Unidentified', '𨧂'],
			['Unidentified', 'ḍ̇'],
		]);
		const altGraph = presses.find(({ key }) => key === 'AltRight');
		expect(altGraph?.consumedModifiers).toMatchObject({ alt: false, control: false });
		const printable = presses.find(({ key }) => key === 'KeyQ');
		expect(printable?.consumedModifiers).toMatchObject({
			alt: true,
			altRight: true,
			control: true,
		});
		terminal.dispose();
	});

	it('publishes keyup with its current text and modifier state', async () => {
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
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'A', 65, { shiftKey: true }));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));

		const release = commands.find(
			(command) => command.kind === 'key-input' && command.payload.action === KeyAction.release,
		);
		expect(release?.kind).toBe('key-input');
		if (release?.kind !== 'key-input') throw new Error('missing key release');
		expect(release.payload.text).toBe('a');
		expect(release.payload.modifiers).toMatchObject({ shift: false, shiftRight: false });
		expect(release.payload.consumedModifiers).toMatchObject({
			shift: false,
			shiftRight: false,
		});
		terminal.dispose();
	});

	it('defers a keyup reentered during its keydown publication', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'key-input' ||
						command.payload.action !== KeyAction.press
					) {
						return;
					}
					reentered = true;
					input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		expect(
			commands.flatMap((command) =>
				command.kind === 'key-input' && command.payload.key === 'KeyA'
					? [command.payload.action]
					: [],
			),
		).toEqual([KeyAction.press, KeyAction.release]);
		terminal.dispose();
	});

	it('abandons an off-bottom keydown after history recovery reenters focus repair', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'set-history-demand' ||
						command.payload.demand !== 'following-bottom'
					) {
						return;
					}
					reentered = true;
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
					input.dispatch('focus', { type: 'focus' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		terminal.scrollLines(-1);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		dispatchBeforeInput(input, 'a');

		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false, true]);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['a']);
		terminal.dispose();
	});

	it('does not restore composition preedit after history recovery reenters blur repair', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const setPreedit = vi.spyOn(WebGPURenderer.prototype, 'setPreedit');
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'set-history-demand' ||
						command.payload.demand !== 'following-bottom'
					) {
						return;
					}
					reentered = true;
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		terminal.scrollLines(-1);
		dispatchComposition(input, 'compositionstart', 'stale');
		terminal.applyFrame(terminalFrame('snapshot', 2n, 2n, ['c', 'd'], null, [0, 1], 10n));
		expect(setPreedit).not.toHaveBeenCalled();
		dispatchComposition(input, 'compositionupdate', 'staler');
		dispatchComposition(input, 'compositionend', 'staler');

		expect(setPreedit).not.toHaveBeenCalled();
		expect(input.value).toBe('');
		expect(commands.filter((command) => command.kind === 'text-input')).toEqual([]);
		terminal.dispose();
	});

	it('does not restore composition preedit after selection clearing reenters reset', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const setPreedit = vi.spyOn(WebGPURenderer.prototype, 'setPreedit');
		let terminal: ProjectedTerminal | undefined;
		let reentered = false;
		terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						reentered ||
						command.kind !== 'selection-command' ||
						command.payload.command !== 'clear'
					) {
						return;
					}
					reentered = true;
					terminal?.reset();
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: CanonicalUint64TextSchema.parse('0') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active',
		});
		const input = terminal.textarea as unknown as FakeElement;
		dispatchComposition(input, 'compositionstart', 'stale');
		expect(setPreedit).not.toHaveBeenCalled();
		dispatchComposition(input, 'compositionupdate', 'staler');
		dispatchComposition(input, 'compositionend', 'staler');

		expect(setPreedit).not.toHaveBeenCalled();
		expect(input.value).toBe('');
		expect(commands.filter((command) => command.kind === 'text-input')).toEqual([]);
		terminal.dispose();
	});

	it('removes key ownership before a synchronous keyup send can reenter repair', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'key-input' ||
						command.payload.action !== KeyAction.release
					) {
						return;
					}
					reentered = true;
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));

		expect(
			commands.filter(
				(command) => command.kind === 'key-input' && command.payload.action === KeyAction.release,
			),
		).toHaveLength(1);
		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false]);
		terminal.dispose();
	});

	it.each(['textarea blur', 'window blur', 'visibility hidden', 'dispose'])(
		'repairs active composition on %s before accepting any late native events',
		async (repair) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', {});
			const commands: ProjectedTerminalCommand[] = [];
			const setPreedit = vi.spyOn(WebGPURenderer.prototype, 'setPreedit');
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
			const input = terminal.textarea as unknown as FakeElement & {
				selectionEnd: number;
				selectionStart: number;
			};
			input.dispatch('focus', { type: 'focus' } as FocusEvent);
			input.value = '候';
			input.selectionStart = 1;
			input.selectionEnd = 1;
			dispatchComposition(input, 'compositionstart', '候');
			dispatchComposition(input, 'compositionupdate', '候选');

			switch (repair) {
				case 'textarea blur':
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
					break;
				case 'window blur':
					browser.dispatchWindow('blur');
					break;
				case 'visibility hidden':
					browser.setVisibilityState('hidden');
					browser.dispatchDocument('visibilitychange');
					break;
				case 'dispose':
					terminal.dispose();
					break;
			}

			expect(input.value).toBe('');
			expect(setPreedit).toHaveBeenLastCalledWith([]);
			expect(
				commands
					.filter((command) => command.kind === 'focus-input')
					.map((command) => command.payload.focused),
			).toEqual([true, false]);
			const repairedPreeditCallCount = setPreedit.mock.calls.length;
			dispatchComposition(input, 'compositionupdate', '过期');
			dispatchComposition(input, 'compositionend', '过期');
			dispatchBeforeInput(input, '过期', false, 'insertFromComposition');
			expect(setPreedit).toHaveBeenCalledTimes(repairedPreeditCallCount);
			expect(commands.filter((command) => command.kind === 'text-input')).toHaveLength(0);

			expect((browser.root as unknown as FakeElement).children).toHaveLength(
				repair === 'dispose' ? 0 : 5,
			);
			dispatchComposition(input, 'compositionstart', '恢复');
			dispatchComposition(input, 'compositionend', '恢复');
			expect(
				commands
					.filter((command) => command.kind === 'text-input')
					.map((command) => command.payload.text),
			).toEqual(repair === 'dispose' ? [] : ['恢复']);
			terminal.dispose();
		},
	);

	it('consumes a key correlation on the first matching beforeinput event', async () => {
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
			preventDefault: () => undefined,
			repeat: false,
			shiftKey: false,
			stopPropagation: () => undefined,
			type: 'keydown',
		} as unknown as KeyboardEvent);
		const beforeInput = (): void =>
			input.dispatch('beforeinput', {
				data: 'a',
				inputType: 'insertText',
				isComposing: false,
				preventDefault: () => undefined,
				stopPropagation: () => undefined,
				type: 'beforeinput',
			} as unknown as InputEvent);

		beforeInput();
		beforeInput();

		expect(
			commands
				.filter((command) => command.kind === 'text-input')
				.map((command) => command.payload.text),
		).toEqual(['a']);
		terminal.dispose();
	});

	it('retains a runtime presentation update while hidden and draws it once on resume', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		browser.flushFrames();
		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');

		await expect(
			terminal.updatePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontFamily: 'Hidden Runtime Mono',
				fontMetricModifiers: { cellHeight: 3 },
			}),
		).resolves.toMatchObject({ kind: 'completed' });
		expect(terminal.textarea?.style.fontFamily).toBe('Hidden Runtime Mono');
		expect(browser.frames.size).toBe(0);

		browser.setVisibilityState('visible');
		browser.dispatchDocument('visibilitychange');
		expect(browser.frames.size).toBe(1);
		browser.flushFrames();
		expect(browser.frames.size).toBe(0);
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
		expect(terminal.capturePresentationSnapshot()).toBeNull();
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
		expect(terminal.capturePresentationSnapshot()).toBeNull();
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				backend: 'canvas2d',
				cellsVisited: 4,
				revision: '1',
				rowsRendered: 2,
			}),
		);
		const firstPresentation = terminal.capturePresentationSnapshot();
		if (firstPresentation === null) throw new Error('Expected the rendered presentation.');
		expect(firstPresentation.metadata.revision).toBe(1n);
		expect(firstPresentation.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
		expect('write' in terminal).toBe(false);

		expect(terminal.applyFrame(snapshot)).toBe('duplicate');
		expect(commands.at(-1)).toEqual({
			kind: 'frame-applied',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '1' },
		});
		expect(terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['c', ''], 1n, [0]))).toBe(
			'committed',
		);
		expect(terminal.capturePresentationSnapshot()).toBeNull();
		browser.flushFrames();
		expect(terminal.capturePresentationSnapshot()?.metadata.revision).toBe(2n);
		expect(firstPresentation.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
		terminal.dispose();
	});

	it('keeps focus-only publication fresh and invalidates idle after an accepted held-input focus prelude', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const inputAttempts: ProjectedTerminalCommand[] = [];
		let rejectFirstFocus = true;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (command.kind === 'focus-input' || command.kind === 'key-input') {
						inputAttempts.push(command);
					}
					if (command.kind === 'focus-input' && rejectFirstFocus) {
						rejectFirstFocus = false;
						throw new Error('focus was rejected before publication');
					}
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(closeActivityTerminalFrame(1n, 1n, 'idle'));
		const metadataChanges: Array<{
			readonly closeActivity: 'active' | 'idle' | 'unknown';
			readonly lastInputKind: ProjectedTerminalCommand['kind'] | undefined;
		}> = [];
		terminal.onMetadataChange((metadata) => {
			metadataChanges.push({
				closeActivity: metadata.closeActivity,
				lastInputKind: inputAttempts.at(-1)?.kind,
			});
		});
		const input = terminal.textarea as unknown as FakeElement;

		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		expect(terminal.metadata?.closeActivity).toBe('idle');
		expect(metadataChanges).toEqual([]);

		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		expect(
			inputAttempts.map((command) =>
				command.kind === 'focus-input' ? `focus:${command.payload.focused}` : command.kind,
			),
		).toEqual(['focus:true', 'focus:true', 'key-input']);
		expect(metadataChanges).toEqual([{ closeActivity: 'unknown', lastInputKind: 'key-input' }]);
		expect(terminal.metadata?.closeActivity).toBe('unknown');
		terminal.dispose();
	});

	it('does not invalidate idle when the activity input is rejected synchronously', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (command.kind === 'key-input') {
						throw new Error('key was rejected before publication');
					}
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(closeActivityTerminalFrame(1n, 1n, 'idle'));
		const metadataChanges: Array<'active' | 'idle' | 'unknown'> = [];
		terminal.onMetadataChange((metadata) => metadataChanges.push(metadata.closeActivity));
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);

		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		expect(metadataChanges).toEqual([]);
		expect(terminal.metadata?.closeActivity).toBe('idle');
		terminal.dispose();
	});

	it('keeps a late pre-input idle frame unknown and replaces its acknowledgement with a freshness snapshot request', async () => {
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
		terminal.applyFrame(closeActivityTerminalFrame(1n, 1n, 'idle'));
		commands.length = 0;

		await expect(terminal.paste('safe')).resolves.toMatchObject({ kind: 'sent' });
		expect(terminal.metadata?.closeActivity).toBe('unknown');
		expect(terminal.applyFrame(closeActivityTerminalFrame(2n, 1n, 'idle'))).toBe(
			'snapshot-required',
		);

		expect(terminal.metadata?.closeActivity).toBe('unknown');
		expect(commands.at(-1)).toEqual({
			kind: 'need-snapshot',
			payload: { attachmentId, reason: 'activity-stale', sequence: '2' },
		});
		expect(
			commands.some(
				(command) => command.kind === 'frame-applied' && command.payload.sequence === '2',
			),
		).toBe(false);

		expect(terminal.applyFrame(closeActivityTerminalFrame(3n, 1n, 'idle'))).toBe('committed');
		expect(terminal.metadata?.closeActivity).toBe('idle');
		expect(commands.at(-1)).toEqual({
			kind: 'frame-applied',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '3' },
		});
		terminal.dispose();
	});

	it('cancels an opening acknowledgement when a duplicate idle frame starts the freshness barrier', async () => {
		const browser = createFakeBrowser();
		const finishFontLoads: Array<() => void> = [];
		const fontLoad = vi.fn<() => Promise<FontFace[]>>(
			() =>
				new Promise<FontFace[]>((resolve) => {
					finishFontLoads.push(() => resolve([]));
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
			terminalId,
		});
		const opening = terminal.open(browser.root);
		await vi.waitFor(() => expect(fontLoad).toHaveBeenCalledTimes(4));
		const idleFrame = closeActivityTerminalFrame(1n, 1n, 'idle');

		expect(terminal.applyFrame(idleFrame)).toBe('committed');
		await expect(terminal.paste('safe')).resolves.toMatchObject({ kind: 'sent' });
		expect(terminal.applyFrame(idleFrame)).toBe('snapshot-required');
		expect(
			commands.some(
				(command) => command.kind === 'frame-applied' && command.payload.sequence === '1',
			),
		).toBe(false);
		expect(terminal.applyFrame(closeActivityTerminalFrame(2n, 1n, 'idle'))).toBe('committed');
		expect(commands.some((command) => command.kind === 'frame-applied')).toBe(false);

		for (const finishFontLoad of finishFontLoads) finishFontLoad();
		await opening;
		expect(commands).toContainEqual({
			kind: 'need-snapshot',
			payload: { attachmentId, reason: 'activity-stale', sequence: '1' },
		});
		expect(
			commands.some(
				(command) => command.kind === 'frame-applied' && command.payload.sequence === '1',
			),
		).toBe(false);
		expect(commands).toContainEqual({
			kind: 'frame-applied',
			payload: { attachmentId, generation: '1', revision: '1', sequence: '2' },
		});
		expect(terminal.metadata?.closeActivity).toBe('idle');
		terminal.dispose();
	});

	it('restarts the idle freshness barrier after another input and a rejected snapshot request', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let rejectFirstSnapshotRequest = true;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (command.kind === 'need-snapshot' && rejectFirstSnapshotRequest) {
						rejectFirstSnapshotRequest = false;
						throw new Error('snapshot request was rejected before publication');
					}
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(closeActivityTerminalFrame(1n, 1n, 'idle'));
		commands.length = 0;

		await terminal.paste('first');
		expect(terminal.applyFrame(closeActivityTerminalFrame(2n, 1n, 'idle'))).toBe(
			'snapshot-required',
		);
		expect(terminal.metadata?.closeActivity).toBe('unknown');
		await terminal.paste('second');
		expect(terminal.applyFrame(closeActivityTerminalFrame(3n, 1n, 'idle'))).toBe(
			'snapshot-required',
		);
		expect(terminal.metadata?.closeActivity).toBe('unknown');

		expect(
			commands
				.filter((command) => command.kind === 'need-snapshot')
				.map((command) => command.payload),
		).toEqual([
			{ attachmentId, reason: 'activity-stale', sequence: '2' },
			{ attachmentId, reason: 'activity-stale', sequence: '3' },
		]);
		expect(terminal.applyFrame(closeActivityTerminalFrame(4n, 1n, 'idle'))).toBe('committed');
		expect(terminal.metadata?.closeActivity).toBe('idle');
		terminal.dispose();
	});

	it('keeps invalidated activity unknown through snapshot failure and clears it on a same-revision live snapshot', async () => {
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
		terminal.applyFrame(closeActivityTerminalFrame(1n, 1n, 'idle'));
		const metadataChanges: Array<'active' | 'idle' | 'unknown'> = [];
		terminal.onMetadataChange((metadata) => metadataChanges.push(metadata.closeActivity));

		await expect(terminal.paste('safe')).resolves.toMatchObject({ kind: 'sent' });
		expect(terminal.metadata?.closeActivity).toBe('unknown');
		expect(terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['x', ''], 0n, [0]))).toBe(
			'snapshot-required',
		);
		expect(terminal.metadata?.closeActivity).toBe('unknown');

		expect(terminal.applyFrame(closeActivityTerminalFrame(3n, 1n, 'idle'))).toBe('committed');
		expect(metadataChanges).toEqual(['unknown', 'idle']);
		expect(terminal.metadata?.closeActivity).toBe('idle');
		terminal.dispose();
	});

	it('does not clear invalidated activity when a history row window commits', async () => {
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
		terminal.applyFrame(closeActivityTerminalFrame(1n, 1n, 'idle', { scrollbackRows: 10n }));
		await expect(terminal.paste('safe')).resolves.toMatchObject({ kind: 'sent' });
		expect(terminal.metadata?.closeActivity).toBe('unknown');

		terminal.scrollLines(-1);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('history request was not sent');
		expect(
			terminal.applyRows(
				renderRowWindow(BigInt(read.payload.requestSequence), 1n, 9n, 12n, [
					'history',
					'live',
					'overscan',
				]),
			),
		).toBe('committed');

		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.viewingHistory);
		expect(terminal.metadata?.closeActivity).toBe('unknown');
		terminal.dispose();
	});

	it('quarantines a stale staged idle frame until its causal snapshot is presented', async () => {
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
		terminal.applyFrame(closeActivityTerminalFrame(1n, 1n, 'idle'));
		await expect(terminal.paste('safe')).resolves.toMatchObject({ kind: 'sent' });
		expect(terminal.metadata?.closeActivity).toBe('unknown');

		expect(terminal.prepareThemePresentation('close-activity-theme', themeOverlays)).toEqual({
			kind: 'prepared',
		});
		const readiness = terminal.waitForThemePresentation('close-activity-theme', 1n);
		expect(
			terminal.applyFrame(closeActivityTerminalFrame(2n, 2n, 'idle', { themeRevision: 1n })),
		).toBe('snapshot-required');
		expect(terminal.metadata?.closeActivity).toBe('unknown');
		let readinessSettled = false;
		void readiness.then(() => {
			readinessSettled = true;
		});
		await Promise.resolve();
		expect(readinessSettled).toBe(false);
		expect(commands.at(-1)).toEqual({
			kind: 'need-snapshot',
			payload: { attachmentId, reason: 'activity-stale', sequence: '2' },
		});
		expect(
			commands.some(
				(command) => command.kind === 'frame-applied' && command.payload.sequence === '2',
			),
		).toBe(false);

		expect(
			terminal.applyFrame(closeActivityTerminalFrame(3n, 2n, 'idle', { themeRevision: 1n })),
		).toBe('staged');
		expect(terminal.metadata?.closeActivity).toBe('unknown');
		await expect(readiness).resolves.toEqual({ kind: 'ready' });
		expect(terminal.canCommitThemePresentation('close-activity-theme', 1n)).toBe(true);

		await expect(terminal.paste('newer')).resolves.toMatchObject({ kind: 'sent' });
		expect(terminal.canCommitThemePresentation('close-activity-theme', 1n)).toBe(false);
		expect(
			terminal.applyFrame(closeActivityTerminalFrame(4n, 2n, 'idle', { themeRevision: 1n })),
		).toBe('snapshot-required');
		expect(commands.at(-1)).toEqual({
			kind: 'need-snapshot',
			payload: { attachmentId, reason: 'activity-stale', sequence: '4' },
		});
		expect(
			terminal.applyFrame(closeActivityTerminalFrame(5n, 2n, 'idle', { themeRevision: 1n })),
		).toBe('staged');
		expect(terminal.canCommitThemePresentation('close-activity-theme', 1n)).toBe(true);

		expect(terminal.commitThemePresentation('close-activity-theme', 1n)).toBe(true);
		expect(terminal.metadata?.closeActivity).toBe('idle');
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

	it('commits a current-view history response from a newer revision until a fresh bottom snapshot', async () => {
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
		const metadataChanges: Array<'active' | 'idle' | 'unknown'> = [];
		terminal.onMetadataChange((metadata) => metadataChanges.push(metadata.closeActivity));
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame(
				'snapshot',
				1n,
				1n,
				['live-a', 'live-b'],
				null,
				[0, 1],
				10n,
				interaction,
				{ column: 0, row: 0 },
				1n,
				2,
				ActiveScreen.primary,
				'active',
			),
		);
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
					origin: { kind: 'tail', rowsBefore: 0 },
					purpose: 'viewport',
					revisionPolicy: 'current-view',
					rowCount: 3,
				}),
			}),
		]);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('history request was not sent');
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(read.payload.requestSequence),
					2n,
					9n,
					12n,
					Array.from({ length: 3 }, (_, index) => {
						if (index === 0) return 'history-a';
						if (index === 1) return 'history-b';
						return `overscan-${index}`;
					}),
				),
			),
		).toBe('committed');
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.viewingHistory);
		expect(terminal.metadata?.revision).toBe(2n);
		expect(terminal.metadata?.closeActivity).toBe('active');
		expect(metadataChanges.at(-1)).toBe('active');
		expect(accessibilityAnnouncement?.attributes.get('aria-live')).toBe('off');
		expect(accessibilityRoot?.children[0]?.attributes.get('data-terminal-absolute-row')).toBe('9');
		expect(commands.at(-1)).toEqual({
			kind: 'rows-applied',
			payload: {
				attachmentId,
				generation: '1',
				requestSequence: read.payload.requestSequence,
				revision: '2',
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

	it('admits bounded line, page, top, and bottom history intents only on the primary screen', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 10n));

		expect(terminal.canScrollHistory).toBe(true);
		expect(terminal.scrollLines(0)).toBe(false);
		expect(terminal.scrollLines(1)).toBe(false);
		expect(terminal.scrollPages(-1)).toBe(true);
		const pageRead = commands.at(-1);
		if (pageRead?.kind !== 'read-rows') throw new Error('page history request was not sent');
		expect(pageRead.payload).toMatchObject({
			origin: { kind: 'tail', rowsBefore: 0 },
			revisionPolicy: 'current-view',
		});
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(pageRead.payload.requestSequence),
					1n,
					9n,
					12n,
					Array.from({ length: pageRead.payload.rowCount }, (_, index) => `page-${index}`),
				),
			),
		).toBe('committed');

		expect(terminal.scrollToTop()).toBe(true);
		const topRead = commands.at(-1);
		if (topRead?.kind !== 'read-rows') throw new Error('top history request was not sent');
		expect(topRead.payload).toMatchObject({
			origin: { kind: 'tail', rowsBefore: 0 },
			revisionPolicy: 'current-view',
		});
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(topRead.payload.requestSequence),
					1n,
					0n,
					12n,
					Array.from({ length: topRead.payload.rowCount }, (_, index) => `top-${index}`),
				),
			),
		).toBe('committed');
		expect(terminal.scrollToTop()).toBe(false);
		expect(terminal.scrollToBottom()).toBe(true);

		terminal.applyFrame(
			terminalFrame(
				'snapshot',
				2n,
				2n,
				['alternate-a', 'alternate-b'],
				null,
				[0, 1],
				0n,
				interaction,
				{ column: 0, row: 0 },
				1n,
				2,
				ActiveScreen.alternate,
			),
		);
		expect(terminal.canScrollHistory).toBe(false);
		terminal.dispose();
		expect(terminal.canScrollHistory).toBe(false);
	});

	it('smooths wheel history motion, reverses on a committed target, and cancels when hidden', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 10n));
		browser.flushFrames(0);
		const root = browser.root as unknown as FakeElement;
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('wheel', wheelEvent(0, -1, { deltaMode: 1 }));
		const historyRead = commands.at(-1);
		if (historyRead?.kind !== 'read-rows') throw new Error('wheel history request was not sent');
		expect(
			terminal.applyRows(
				renderRowWindow(BigInt(historyRead.payload.requestSequence), 1n, 9n, 12n, [
					'history-a',
					'history-b',
					'overscan',
				]),
			),
		).toBe('committed');
		browser.flushFrames(16);
		const activeMotion = (): FakeElement | undefined =>
			root.children.find((element) => element.dataset['projectedTerminalHistoryMotion'] === 'true');
		expect(activeMotion()).toBeDefined();
		expect(canvas.style['transform']).toMatch(/^translateY\(-[\d.]+px\)$/u);

		browser.flushFrames(50);
		const upwardOffset = canvas.style['transform'];
		expect(upwardOffset).toMatch(/^translateY\(-[\d.]+px\)$/u);

		canvas.dispatch('wheel', wheelEvent(0, 1, { deltaMode: 1 }));
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.recovering);
		terminal.applyFrame(terminalFrame('snapshot', 2n, 2n, ['live-c', 'live-d'], null, [0, 1], 10n));
		browser.flushFrames(60);
		expect(activeMotion()).toBeDefined();
		expect(canvas.style['transform']).toMatch(/^translateY\([\d.]+px\)$/u);

		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');
		expect(activeMotion()).toBeUndefined();
		expect(canvas.style['transform'] ?? '').toBe('');
		expect(browser.frames.size).toBe(0);
		terminal.dispose();
	});

	it('keeps a later programmatic history intent immediate after queued wheel motion', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 10n));
		browser.flushFrames(0);
		const root = browser.root as unknown as FakeElement;
		const canvas = terminal.canvas as unknown as FakeElement;
		const activeMotion = (): FakeElement | undefined =>
			root.children.find((element) => element.dataset['projectedTerminalHistoryMotion'] === 'true');

		canvas.dispatch('wheel', wheelEvent(0, -1, { deltaMode: 1 }));
		const firstRead = commands.at(-1);
		if (firstRead?.kind !== 'read-rows') throw new Error('first history request was not sent');
		canvas.dispatch('wheel', wheelEvent(0, -1, { deltaMode: 1 }));
		terminal.scrollLines(-1);
		expect(
			terminal.applyRows(
				renderRowWindow(BigInt(firstRead.payload.requestSequence), 1n, 9n, 12n, [
					'first-a',
					'first-b',
					'overscan',
				]),
			),
		).toBe('committed');
		browser.flushFrames(16);
		expect(activeMotion()).toBeUndefined();
		const secondRead = commands.at(-1);
		if (secondRead?.kind !== 'read-rows') throw new Error('queued history request was not sent');
		expect(
			terminal.applyRows(
				renderRowWindow(BigInt(secondRead.payload.requestSequence), 1n, 7n, 12n, [
					'second-a',
					'second-b',
					'overscan',
				]),
			),
		).toBe('committed');
		browser.flushFrames(32);

		expect(activeMotion()).toBeUndefined();
		expect(canvas.style['transform'] ?? '').toBe('');
		terminal.dispose();
	});

	it('preserves the live accessibility baseline while retained history is visible', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['base-a', 'base-b'], null, [0, 1], 10n));
		const announcement = (browser.root as unknown as FakeElement).children[3];
		expect(announcement?.textContent).toBe('base-a\nbase-b');

		terminal.scrollLines(-1);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('history request was not sent');
		expect(
			terminal.applyRows(
				renderRowWindow(BigInt(read.payload.requestSequence), 1n, 9n, 12n, [
					'history-a',
					'history-b',
					'overscan',
				]),
			),
		).toBe('committed');

		terminal.scrollToBottom();
		terminal.applyFrame(
			terminalFrame('snapshot', 2n, 2n, ['base-a', 'fresh-b'], null, [0, 1], 10n),
		);

		expect(announcement?.textContent).toBe('fresh-b');
		terminal.dispose();
	});

	it('uses the Host committed pin as the visible history anchor after reflow', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 18n));
		const accessibilityRoot = (browser.root as unknown as FakeElement).children[2];

		terminal.scrollLines(-3);
		const initialRead = commands.at(-1);
		if (initialRead?.kind !== 'read-rows') throw new Error('initial history request was not sent');
		expect(initialRead.payload.revisionPolicy).toBe('current-view');
		const initialStart = resolvedRowRequestStart(initialRead, 20n);
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(initialRead.payload.requestSequence),
					1n,
					initialStart,
					20n,
					Array.from({ length: initialRead.payload.rowCount }, (_, index) => `before-${index}`),
				),
			),
		).toBe('committed');

		terminal.notifyHistoryChanged({
			anchorStatus: 'valid',
			attachmentId,
			generation: canonicalText('2'),
			revision: canonicalText('2'),
			sequence: canonicalText('1'),
			totalRows: canonicalText('18'),
		});
		const reflowRead = commands.at(-1);
		if (reflowRead?.kind !== 'read-rows') throw new Error('reflow history request was not sent');
		expect(reflowRead.payload.origin).toEqual({ delta: 0, kind: 'committed' });
		const reflowStart = 12n;
		const reflowRowCount = Math.min(reflowRead.payload.rowCount, Number(18n - reflowStart));
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(reflowRead.payload.requestSequence),
					2n,
					reflowStart,
					18n,
					Array.from({ length: reflowRowCount }, (_, index) => `after-${index}`),
					{ generation: 2n },
				),
			),
		).toBe('committed');
		expect(accessibilityRoot?.children[0]?.attributes.get('data-terminal-absolute-row')).toBe('12');

		terminal.scrollLines(-1);
		expect(commands.at(-1)).toEqual(
			expect.objectContaining({
				kind: 'read-rows',
				payload: expect.objectContaining({ origin: { delta: -1, kind: 'committed' } }),
			}),
		);
		terminal.dispose();
	});

	it('refreshes a reflowed history viewport after its old generation request fails', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 18n));

		terminal.scrollLines(-3);
		const initialRead = commands.at(-1);
		if (initialRead?.kind !== 'read-rows') throw new Error('initial history request was not sent');
		const initialStart = resolvedRowRequestStart(initialRead, 20n);
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(initialRead.payload.requestSequence),
					1n,
					initialStart,
					20n,
					Array.from({ length: initialRead.payload.rowCount }, (_, index) => `before-${index}`),
				),
			),
		).toBe('committed');

		terminal.scrollLines(-1);
		const supersededRead = commands.at(-1);
		if (supersededRead?.kind !== 'read-rows') {
			throw new Error('superseded history request was not sent');
		}
		expect(supersededRead.payload).toMatchObject({
			generation: '1',
			origin: { delta: -1, kind: 'committed' },
			revision: '1',
		});

		terminal.notifyHistoryChanged({
			anchorStatus: 'valid',
			attachmentId,
			generation: canonicalText('2'),
			revision: canonicalText('2'),
			sequence: canonicalText('1'),
			totalRows: canonicalText('20'),
		});
		terminal.notifyBrokerFailure({
			code: 'stale-generation',
			message: 'ReadRows failed because the resize advanced the generation',
			operation: 'history',
			requestSequence: supersededRead.payload.requestSequence,
			retryable: true,
		});

		const refreshRead = commands.at(-1);
		if (refreshRead?.kind !== 'read-rows') {
			throw new Error('reflow refresh request was not sent');
		}
		expect(refreshRead.payload).toMatchObject({
			generation: '2',
			origin: { delta: 0, kind: 'committed' },
			revision: '2',
		});
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.requestingHistory);
		expect(errors).not.toHaveBeenCalled();
		expect(
			commands.some(
				(command) =>
					command.kind === 'set-history-demand' && command.payload.demand === 'following-bottom',
			),
		).toBe(false);
		terminal.dispose();
	});

	it('waits for the reflow event when a stale-generation history failure arrives first', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 18n));

		terminal.scrollLines(-3);
		const initialRead = commands.at(-1);
		if (initialRead?.kind !== 'read-rows') throw new Error('initial history request was not sent');
		const initialStart = resolvedRowRequestStart(initialRead, 20n);
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(initialRead.payload.requestSequence),
					1n,
					initialStart,
					20n,
					Array.from({ length: initialRead.payload.rowCount }, (_, index) => `before-${index}`),
				),
			),
		).toBe('committed');

		terminal.scrollLines(-1);
		const read = commands.at(-1);
		if (read?.kind !== 'read-rows') throw new Error('superseded history request was not sent');
		const readCount = commands.filter((command) => command.kind === 'read-rows').length;
		terminal.notifyBrokerFailure({
			code: 'stale-generation',
			message: 'ReadRows failed because the resize advanced the generation',
			operation: 'history',
			requestSequence: read.payload.requestSequence,
			retryable: true,
		});

		expect(commands.filter((command) => command.kind === 'read-rows')).toHaveLength(readCount);
		expect(errors).not.toHaveBeenCalled();
		terminal.notifyHistoryChanged({
			anchorStatus: 'valid',
			attachmentId,
			generation: canonicalText('2'),
			revision: canonicalText('2'),
			sequence: canonicalText('1'),
			totalRows: canonicalText('20'),
		});

		expect(commands.at(-1)).toEqual(
			expect.objectContaining({
				kind: 'read-rows',
				payload: expect.objectContaining({
					generation: '2',
					origin: { delta: 0, kind: 'committed' },
					revision: '2',
				}),
			}),
		);
		expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.requestingHistory);
		expect(errors).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it.each(['evicted', 'none'] as const)(
		'resumes bottom projection when the Host history anchor becomes %s',
		async (anchorStatus) => {
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
				terminalFrame('snapshot', 1n, 1n, ['live-a', 'live-b'], null, [0, 1], 10n),
			);
			terminal.scrollLines(-1);
			const historyRead = commands.at(-1);
			if (historyRead?.kind !== 'read-rows') throw new Error('history request was not sent');
			const historyStart = resolvedRowRequestStart(historyRead, 12n);
			expect(
				terminal.applyRows(
					renderRowWindow(BigInt(historyRead.payload.requestSequence), 1n, historyStart, 12n, [
						'history-a',
						'history-b',
						'overscan',
					]),
				),
			).toBe('committed');
			commands.length = 0;

			terminal.notifyHistoryChanged({
				anchorStatus,
				attachmentId,
				generation: canonicalText('2'),
				revision: canonicalText('2'),
				sequence: canonicalText('1'),
				totalRows: canonicalText('2'),
			});

			expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.recovering);
			expect(commands).toEqual([
				{
					kind: 'history-observed',
					payload: { attachmentId, generation: '2', revision: '2', sequence: '1' },
				},
				{
					kind: 'set-history-demand',
					payload: { attachmentId, demand: 'following-bottom' },
				},
				{
					kind: 'set-projection-demand',
					payload: { attachmentId, demand: 'active' },
				},
			]);
			expect(
				terminal.applyFrame(
					terminalFrame(
						'snapshot',
						2n,
						2n,
						['fresh-a', 'fresh-b'],
						null,
						[0, 1],
						0n,
						interaction,
						{ column: 0, row: 0 },
						2n,
					),
				),
			).toBe('committed');
			expect(terminal.historyPhase).toBe(ProjectedHistoryPhase.followingBottom);
			expect(terminal.metadata).toMatchObject({ generation: 2n, revision: 2n });
			terminal.dispose();
		},
	);

	it('routes fixed scrollbar ownership through current-view requests and latest-intent coalescing', async () => {
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
		const { scrollbar } = historyScrollbarElements(browser);
		canvas.dispatch('pointermove', {
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			shiftKey: true,
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

		scrollbar.dispatch('pointerdown', {
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
		expect(scrollbar.capturedPointers.has(41)).toBe(true);
		expect(canvas.style['cursor']).toBe('default');
		expect(commands.some((command) => command.kind === 'mouse-input')).toBe(false);
		expect(commands.some((command) => command.kind === 'selection-gesture')).toBe(false);
		const firstRead = commands.find((command) => command.kind === 'read-rows');
		if (firstRead?.kind !== 'read-rows') throw new Error('scrollbar request was not sent');
		expect(firstRead.payload).toMatchObject({
			origin: { kind: 'tail' },
			purpose: 'viewport',
			revisionPolicy: 'current-view',
			rowCount: 34,
		});
		expect(resolvedRowRequestStart(firstRead, 100n)).toBe(49n);

		for (const clientY of [100, 260]) {
			scrollbar.dispatch('pointermove', {
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
					49n,
					100n,
					Array.from({ length: 34 }, (_, index) => `row-${index}`),
				),
			),
		).toBe('committed');
		const reads = commands.filter((command) => command.kind === 'read-rows');
		expect(reads).toHaveLength(2);
		const latestRead = reads.at(-1);
		if (latestRead?.kind !== 'read-rows')
			throw new Error('coalesced scrollbar request was not sent');
		expect(latestRead.payload).toMatchObject({
			origin: { kind: 'tail' },
			purpose: 'viewport',
			revisionPolicy: 'current-view',
			rowCount: 27,
		});
		expect(resolvedRowRequestStart(latestRead, 100n)).toBe(73n);
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
		const { scrollbar } = historyScrollbarElements(browser);
		const dispatchOwned = (
			type: 'pointercancel' | 'pointerdown' | 'pointermove' | 'pointerup',
			values: Record<string, unknown>,
		): void => {
			const preventDefault = vi.fn<() => void>();
			const stopImmediatePropagation = vi.fn<() => void>();
			scrollbar.dispatch(type, {
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
		expect(scrollbar.capturedPointers.has(41)).toBe(true);
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

		expect(scrollbar.capturedPointers.has(41)).toBe(true);
		expect(scrollbar.capturedPointers.has(42)).toBe(false);
		expect(scrollbar.capturedPointers.has(43)).toBe(false);
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
					49n,
					100n,
					Array.from({ length: 34 }, (_, index) => `row-${index}`),
				),
			),
		).toBe('committed');
		const latestRead = commands.filter((command) => command.kind === 'read-rows').at(-1);
		if (latestRead?.kind !== 'read-rows')
			throw new Error('coalesced scrollbar request was not sent');
		expect(latestRead.payload).toMatchObject({
			origin: { kind: 'tail' },
			purpose: 'viewport',
			revisionPolicy: 'current-view',
			rowCount: 27,
		});
		expect(resolvedRowRequestStart(latestRead, 100n)).toBe(73n);
		dispatchOwned('pointerup', { buttons: 0, pointerId: 41 });
		expect(scrollbar.capturedPointers.has(41)).toBe(false);
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
			shiftKey: true,
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
		const { scrollbar } = historyScrollbarElements(browser);

		scrollbar.dispatch('pointerdown', {
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
		scrollbar.dispatch('wheel', {
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

		scrollbar.dispatch('pointerup', {
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
		scrollbar.dispatch('click', click);
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
		const { scrollbar, thumb } = historyScrollbarElements(browser);
		const canvas = terminal.canvas as unknown as FakeElement;
		expect(scrollbar.hidden).toBe(false);
		expect(scrollbar.style['right']).toBe('0');
		expect(scrollbar.style['width']).toBe('16px');
		expect(thumb.style['right']).toBe('2px');
		expect(thumb.style['width']).toBe('8px');
		expect(thumb.style['borderRadius']).toBe('9999px');
		expect(Number.parseFloat(canvas.style['width'] ?? '0')).toBeLessThan(
			(browser.root as unknown as FakeElement).clientWidth - 2,
		);

		terminal.scrollLines(-1);
		const visibleOpacity = thumb.style['opacity'];
		expect(Number(visibleOpacity)).toBeGreaterThan(0);
		vi.advanceTimersByTime(1_499);
		expect(thumb.style['opacity']).toBe(visibleOpacity);
		vi.advanceTimersByTime(1);
		expect(thumb.style['opacity']).toBe('0');

		scrollbar.dispatch('pointerdown', {
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
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);
		vi.advanceTimersByTime(2_000);
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);

		scrollbar.dispatch('pointerup', {
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
		expect(scrollbar.capturedPointers.has(42)).toBe(false);
		vi.advanceTimersByTime(1_499);
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);
		vi.advanceTimersByTime(1);
		expect(thumb.style['opacity']).toBe('0');
		expect(canvas.attributes.get('aria-hidden')).toBe('true');
		expect(canvas.getAttribute('role')).toBeNull();
		terminal.dispose();
	});

	it('continues previous-prompt navigation across a revision-only viewport refresh', async () => {
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
		commands.length = 0;
		const readBufferRows = vi
			.spyOn(terminal, 'readBufferRows')
			.mockImplementation(async (request) => ({
				byteLength: 4_096,
				detail: request.detail,
				generation: request.generation,
				kind: 'rows',
				revision: request.revision,
				rows: Array.from({ length: request.rowCount }, (_, offset) => {
					const index = request.startRow + BigInt(offset);
					return promptNavigationRow(
						index,
						index === (request.revision === 1n ? 80n : 40n)
							? RowPromptClass.prompt
							: RowPromptClass.none,
					);
				}),
				startRow: request.startRow,
				totalRows: 100n,
			}));

		expect(terminal.canNavigatePrompts).toBe(true);
		await expect(terminal.scrollToPreviousPrompt(new AbortController().signal)).resolves.toBe(true);
		expect(readBufferRows).toHaveBeenCalledWith(
			expect.objectContaining({
				generation: 1n,
				purpose: 'buffer',
				revision: 1n,
				rowCount: 98,
				startRow: 0n,
			}),
			expect.any(AbortSignal),
		);
		expect(
			commands.find(
				(command) => command.kind === 'read-rows' && command.payload.purpose === 'viewport',
			)?.payload,
		).toMatchObject({
			origin: { kind: 'tail', rowsBefore: 0 },
			purpose: 'viewport',
			revisionPolicy: 'current-view',
		});
		expect(terminal.canNavigatePrompts).toBe(true);
		terminal.notifyHistoryChanged({
			anchorStatus: 'valid',
			attachmentId,
			generation: canonicalText('1'),
			revision: canonicalText('2'),
			sequence: canonicalText('1'),
			totalRows: canonicalText('100'),
		});
		await expect(terminal.scrollToPreviousPrompt(new AbortController().signal)).resolves.toBe(true);
		expect(readBufferRows).toHaveBeenCalledTimes(2);
		expect(readBufferRows).toHaveBeenLastCalledWith(
			expect.objectContaining({
				generation: 1n,
				revision: 2n,
				rowCount: 80,
				startRow: 0n,
			}),
			expect.any(AbortSignal),
		);
		expect(
			commands.filter(
				(command) => command.kind === 'read-rows' && command.payload.purpose === 'viewport',
			),
		).toHaveLength(1);
		terminal.dispose();
	});

	it('retires the transient prompt anchor after another local history intent', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n));
		const readBufferRows = vi
			.spyOn(terminal, 'readBufferRows')
			.mockImplementation(async (request) => ({
				byteLength: 4_096,
				detail: request.detail,
				generation: request.generation,
				kind: 'rows',
				revision: request.revision,
				rows: Array.from({ length: request.rowCount }, (_, offset) => {
					const index = request.startRow + BigInt(offset);
					return promptNavigationRow(
						index,
						index === 80n ? RowPromptClass.prompt : RowPromptClass.none,
					);
				}),
				startRow: request.startRow,
				totalRows: 100n,
			}));

		await expect(terminal.scrollToPreviousPrompt(new AbortController().signal)).resolves.toBe(true);
		terminal.notifyHistoryChanged({
			anchorStatus: 'valid',
			attachmentId,
			generation: canonicalText('1'),
			revision: canonicalText('2'),
			sequence: canonicalText('1'),
			totalRows: canonicalText('100'),
		});
		terminal.scrollLines(-1);

		await expect(terminal.scrollToPreviousPrompt(new AbortController().signal)).resolves.toBe(
			false,
		);
		expect(readBufferRows).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('retires a discovered prompt after a newer local history intent', async () => {
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
		commands.length = 0;
		let resolveRows: ((result: BufferRowsResult) => void) | undefined;
		vi.spyOn(terminal, 'readBufferRows').mockImplementation(
			(request) =>
				new Promise((resolve) => {
					resolveRows = (result) => resolve(result);
					void request;
				}),
		);

		const navigation = terminal.scrollToPreviousPrompt(new AbortController().signal);
		await Promise.resolve();
		terminal.scrollLines(-1);
		resolveRows?.({
			byteLength: 4_096,
			detail: 'text',
			generation: 1n,
			kind: 'rows',
			revision: 1n,
			rows: Array.from({ length: 98 }, (_, index) =>
				promptNavigationRow(
					BigInt(index),
					index === 80 ? RowPromptClass.prompt : RowPromptClass.none,
				),
			),
			startRow: 0n,
			totalRows: 100n,
		});

		await expect(navigation).resolves.toBe(false);
		expect(
			commands.filter(
				(command) => command.kind === 'read-rows' && command.payload.purpose === 'viewport',
			),
		).toHaveLength(1);
		terminal.dispose();
	});

	it('keeps prompt navigation unavailable on the alternate screen', async () => {
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
				2,
				ActiveScreen.alternate,
			),
		);
		const readBufferRows = vi.spyOn(terminal, 'readBufferRows');

		expect(terminal.canNavigatePrompts).toBe(false);
		await expect(terminal.scrollToNextPrompt(new AbortController().signal)).resolves.toBe(false);
		expect(readBufferRows).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('shows and fades the scrollbar when search reveals an offscreen history row', async () => {
		vi.useFakeTimers();
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let searchNavigation: unknown;
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
			searchWorkerFactory: () =>
				searchWorkerWithMatch(1, (message) => {
					searchNavigation = message.navigation;
				}),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 98n));
		browser.flushFrames();
		commands.length = 0;
		const { thumb } = historyScrollbarElements(browser);
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

		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: canonicalText('1') },
			preview: 'needle',
			rectangle: false,
			start: { column: 0, row: canonicalText('1') },
			state: 'active',
		});
		terminal.updateSearch('needle');
		await vi.advanceTimersByTimeAsync(200);

		expect(readBufferRows).toHaveBeenCalledOnce();
		expect(searchNavigation).toEqual({
			preserveStart: true,
			reverse: true,
			start: { column: 0, row: 1 },
		});
		expect(
			commands.find(
				(command) =>
					command.kind === 'selection-command' && command.payload.command === 'set-range',
			)?.payload,
		).toMatchObject({
			command: 'set-range',
			end: { column: 1, row: '1' },
			generation: '1',
			revision: '1',
			start: { column: 0, row: '1' },
		});
		expect(
			commands.find(
				(command) => command.kind === 'read-rows' && command.payload.purpose === 'viewport',
			)?.payload,
		).toMatchObject({
			origin: { kind: 'absolute', row: '0' },
			purpose: 'viewport',
		});
		const visibleOpacity = thumb.style['opacity'];
		expect(Number(visibleOpacity)).toBeGreaterThan(0);
		vi.advanceTimersByTime(1_499);
		expect(thumb.style['opacity']).toBe(visibleOpacity);
		vi.advanceTimersByTime(1);
		expect(thumb.style['opacity']).toBe('0');
		terminal.dispose();
		readBufferRows.mockRestore();
	});

	it('defers search and navigation fade until an active scrollbar drag ends', async () => {
		vi.useFakeTimers();
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
		const { scrollbar, thumb } = historyScrollbarElements(browser);
		scrollbar.dispatch('pointerdown', {
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

		terminal.updateSearch('needle');
		await vi.advanceTimersByTimeAsync(200);
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);
		vi.advanceTimersByTime(2_000);
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);

		terminal.scrollLines(-1);
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);
		vi.advanceTimersByTime(2_000);
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);

		scrollbar.dispatch('pointerup', {
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
		expect(Number(thumb.style['opacity'])).toBeGreaterThan(0);
		vi.advanceTimersByTime(1);
		expect(thumb.style['opacity']).toBe('0');
		terminal.dispose();
		readBufferRows.mockRestore();
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
		const { scrollbar } = historyScrollbarElements(browser);
		scrollbar.dispatch('pointerdown', {
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
		expect(scrollbar.capturedPointers.has(43)).toBe(true);

		switch (repair) {
			case 'buttonless pointer movement':
				scrollbar.dispatch('pointermove', {
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
				scrollbar.dispatch('pointercancel', {
					pointerId: 43,
					preventDefault: () => {},
					stopImmediatePropagation: () => {},
					type: 'pointercancel',
				} as unknown as PointerEvent);
				break;
			case 'lost pointer capture':
				scrollbar.capturedPointers.delete(43);
				scrollbar.dispatch('lostpointercapture', {
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

		expect(scrollbar.capturedPointers.has(43)).toBe(false);
		if (repair === 'disposal') return;
		const preventDefault = vi.fn<() => void>();
		scrollbar.dispatch('pointerup', {
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
		const { scrollbar } = historyScrollbarElements(browser);
		scrollbar.dispatch('pointerdown', {
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
		expect(scrollbar.capturedPointers.has(44)).toBe(true);

		terminal.reset();

		expect(scrollbar.capturedPointers.has(44)).toBe(false);
		expect(commands.some((command) => command.kind === 'reset-terminal')).toBe(false);
		const preventDefault = vi.fn<() => void>();
		scrollbar.dispatch('pointerup', {
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

	it('settles viewport, search, and buffer row lanes independently in response order', async () => {
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
		commands.length = 0;
		const searchRows = terminal.readBufferRows(
			{
				detail: 'text',
				generation: 1n,
				maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				purpose: 'search',
				revision: 42n,
				rowCount: 1,
				startRow: 0n,
			},
			new AbortController().signal,
		);
		const bufferRows = terminal.readBufferRows(
			{
				detail: 'text',
				generation: 1n,
				maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				purpose: 'buffer',
				revision: 42n,
				rowCount: 1,
				startRow: 1n,
			},
			new AbortController().signal,
		);
		terminal.scrollLines(-1);

		try {
			const reads = commands.filter((command) => command.kind === 'read-rows');
			expect(reads.map((command) => command.payload.purpose)).toEqual([
				'search',
				'buffer',
				'viewport',
			]);
			const searchRead = reads[0];
			const bufferRead = reads[1];
			const viewportRead = reads[2];
			if (
				searchRead?.kind !== 'read-rows' ||
				bufferRead?.kind !== 'read-rows' ||
				viewportRead?.kind !== 'read-rows'
			) {
				throw new Error('three row lanes were not admitted');
			}
			expect(
				terminal.applyRows(
					encodedTextRowWindow(BigInt(bufferRead.payload.requestSequence), 42n, 1n, 12n, ['b']),
				),
			).toBe('committed');
			expect(searchRead.payload.revisionPolicy).toBe('exact');
			expect(bufferRead.payload.revisionPolicy).toBe('exact');
			expect(viewportRead.payload.revisionPolicy).toBe('current-view');
			const viewportStart = resolvedRowRequestStart(viewportRead, 12n);
			expect(
				terminal.applyRows(
					renderRowWindow(
						BigInt(viewportRead.payload.requestSequence),
						42n,
						viewportStart,
						12n,
						Array.from({ length: viewportRead.payload.rowCount }, () => 'view'),
					),
				),
			).toBe('committed');
			expect(
				terminal.applyRows(
					encodedTextRowWindow(BigInt(searchRead.payload.requestSequence), 42n, 0n, 12n, ['s']),
				),
			).toBe('committed');
			await expect(bufferRows).resolves.toMatchObject({ kind: 'rows', startRow: 1n });
			await expect(searchRows).resolves.toMatchObject({ kind: 'rows', startRow: 0n });
		} finally {
			terminal.dispose();
		}
	});

	it('settles a stale buffer read without surfacing a history failure', async () => {
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
		terminal.notifyBrokerFailure({
			code: 'stale-revision',
			message: 'ReadRows failed because the revision advanced',
			operation: 'history',
			requestSequence: read.payload.requestSequence,
			retryable: true,
		});

		await expect(rows).resolves.toEqual({
			generation: 1n,
			kind: 'stale',
			revision: 42n,
			totalRows: 12n,
		});
		expect(errors).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('keeps ordinary Host selections out of the document DOM selection', async () => {
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
		terminal.focus();

		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: CanonicalUint64TextSchema.parse('11') },
			preview: null,
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			state: 'active',
		});

		const selection = browser.document.getSelection();
		expect(selection?.rangeCount).toBe(0);
		expect(browser.document.activeElement).toBe(terminal.textarea);

		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			preview: null,
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('11') },
			state: 'active',
		});

		expect(selection?.rangeCount).toBe(0);
		expect(browser.document.activeElement).toBe(terminal.textarea);
		terminal.dispose();
	});

	it('reconciles only an assistive-technology DOM range from Host selection', async () => {
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

		browser.selectAccessibilityText(0, 0, 1, 1);
		expect(commands.at(-1)).toMatchObject({
			kind: 'selection-command',
			payload: { command: 'set-range' },
		});
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
		expect(
			(range?.endContainer as unknown as { readonly parentElement?: FakeElement })?.parentElement,
		).toBe(accessibilityRoot?.children[1]);

		expect(terminal.setSelectionRange({ column: 0, row: 10n }, { column: 1, row: 10n })).toBe(true);
		expect(selection?.rangeCount).toBe(0);
		terminal.dispose();
	});

	it('keeps a search-owned Host selection out of the document DOM selection', async () => {
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
			searchWorkerFactory: () => searchWorkerWithMatch(0),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['needle', '']));
		const rows: TerminalBufferRow[] = [
			{
				cells: null,
				identityAscii: true,
				index: 0n,
				mappings: [],
				promptClass: RowPromptClass.none,
				text: 'needle',
				wrap: false,
				wrapContinuation: false,
			},
			{
				cells: null,
				identityAscii: true,
				index: 1n,
				mappings: [],
				promptClass: RowPromptClass.none,
				text: '',
				wrap: false,
				wrapContinuation: false,
			},
		];
		vi.spyOn(terminal, 'readBufferRows').mockImplementation(async (request) => ({
			byteLength: rows.length,
			detail: request.detail,
			generation: request.generation,
			kind: 'rows',
			revision: request.revision,
			rows,
			startRow: request.startRow,
			totalRows: 2n,
		}));

		terminal.updateSearch('needle');
		await vi.advanceTimersByTimeAsync(200);
		expect(
			commands.some(
				(command) =>
					command.kind === 'selection-command' && command.payload.command === 'set-range',
			),
		).toBe(true);

		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: canonicalText('0') },
			preview: 'ne',
			rectangle: false,
			start: { column: 0, row: canonicalText('0') },
			state: 'active',
		});

		expect(terminal.selection).toMatchObject({ state: 'active' });
		expect(browser.document.getSelection()?.rangeCount).toBe(0);
		terminal.dispose();
	});

	it('retains committed search decorations while an incremental query is pending', async () => {
		vi.useFakeTimers();
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const setSearchMatches = vi.spyOn(WebGPURenderer.prototype, 'setSearchMatches');
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			searchWorkerFactory: () => searchWorkerWithMatch(0),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['12', '']));
		const rows: TerminalBufferRow[] = [
			{
				cells: null,
				identityAscii: true,
				index: 0n,
				mappings: [],
				promptClass: RowPromptClass.none,
				text: '12',
				wrap: false,
				wrapContinuation: false,
			},
			{
				cells: null,
				identityAscii: true,
				index: 1n,
				mappings: [],
				promptClass: RowPromptClass.none,
				text: '',
				wrap: false,
				wrapContinuation: false,
			},
		];
		vi.spyOn(terminal, 'readBufferRows').mockImplementation(async (request) => ({
			byteLength: rows.length,
			detail: request.detail,
			generation: request.generation,
			kind: 'rows',
			revision: request.revision,
			rows,
			startRow: request.startRow,
			totalRows: 2n,
		}));

		terminal.updateSearch('1');
		await vi.advanceTimersByTimeAsync(200);
		expect(setSearchMatches).toHaveBeenLastCalledWith(
			expect.arrayContaining([expect.objectContaining({ text: 'needle' })]),
			expect.objectContaining({ text: 'needle' }),
		);

		setSearchMatches.mockClear();
		terminal.updateSearch('12');
		expect(setSearchMatches).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(199);
		expect(setSearchMatches).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		expect(setSearchMatches).toHaveBeenCalledOnce();
		expect(setSearchMatches).toHaveBeenLastCalledWith(
			expect.arrayContaining([expect.objectContaining({ text: 'needle' })]),
			expect.objectContaining({ text: 'needle' }),
		);
		terminal.dispose();
	});

	it('keeps Host selection when the document selection leaves the terminal surface', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['left', 'selection']));
		const hostSelection = {
			attachmentId,
			end: { column: 8, row: CanonicalUint64TextSchema.parse('1') },
			preview: null,
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active' as const,
		};
		terminal.notifySelectionChanged(hostSelection);

		const sibling = browser.document.createElement('div') as unknown as FakeElement;
		sibling.textContent = 'sibling';
		const siblingText = sibling.firstChild;
		if (siblingText === null) throw new Error('sibling text node is unavailable');
		const siblingRange = browser.document.createRange();
		siblingRange.setStart(siblingText, 0);
		siblingRange.setEnd(siblingText, 1);
		const documentSelection = browser.document.getSelection();
		if (documentSelection === null) throw new Error('document selection is unavailable');
		commands.length = 0;
		documentSelection.removeAllRanges();
		documentSelection.addRange(siblingRange);
		browser.dispatchDocument('selectionchange');

		expect(
			commands.filter(
				(command) => command.kind === 'selection-command' && command.payload.command === 'clear',
			),
		).toHaveLength(0);
		expect(terminal.selection).toEqual(hostSelection);

		terminal.notifySelectionChanged(hostSelection);
		commands.length = 0;
		documentSelection.removeAllRanges();
		browser.dispatchDocument('selectionchange');

		expect(
			commands.filter(
				(command) => command.kind === 'selection-command' && command.payload.command === 'clear',
			),
		).toHaveLength(0);
		expect(terminal.selection).toEqual(hostSelection);
		terminal.dispose();
	});

	it.each(['surface blur', 'window blur'] as const)(
		'keeps Host selection when %s collapses an assistive-technology DOM range',
		async (focusLoss) => {
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
			terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['left', 'selection']));
			const hostSelection = {
				attachmentId,
				end: { column: 8, row: CanonicalUint64TextSchema.parse('1') },
				preview: null,
				rectangle: false,
				start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
				state: 'active' as const,
			};
			terminal.notifySelectionChanged(hostSelection);
			browser.selectAccessibilityText(0, 0, 1, 9);
			terminal.notifySelectionChanged(hostSelection);

			commands.length = 0;
			if (focusLoss === 'surface blur') {
				(terminal.textarea as unknown as FakeElement).dispatch('blur', {
					relatedTarget: null,
					type: 'blur',
				} as unknown as FocusEvent);
			} else {
				browser.dispatchWindow('blur');
			}
			const documentSelection = browser.document.getSelection();
			if (documentSelection === null || documentSelection.rangeCount !== 1) {
				throw new Error('mirrored document selection is unavailable');
			}
			const range = documentSelection.getRangeAt(0);
			range.setEnd(range.startContainer, range.startOffset);
			browser.dispatchDocument('selectionchange');

			expect(
				commands.filter(
					(command) => command.kind === 'selection-command' && command.payload.command === 'clear',
				),
			).toHaveLength(0);
			expect(terminal.selection).toEqual(hostSelection);
			terminal.dispose();
		},
	);

	it('exposes accessible rows through a valid navigable list', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a b', ''], null, [0, 1], 10n));

		const accessibilityRoot = (browser.root as unknown as FakeElement).children[2];
		expect(accessibilityRoot?.attributes.get('role')).toBe('list');
		expect(accessibilityRoot?.attributes.has('aria-rowcount')).toBe(false);
		expect(
			accessibilityRoot?.children.map((row) => ({
				position: row.attributes.get('aria-posinset'),
				role: row.attributes.get('role'),
				size: row.attributes.get('aria-setsize'),
				text: row.textContent,
			})),
		).toEqual([
			{ position: '11', role: 'listitem', size: '12', text: 'a b' },
			{ position: '12', role: 'listitem', size: '12', text: ' ' },
		]);
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
		expect(overscanned.payload.rowCount).toBe(3);

		terminal.notifyBrokerFailure({
			code: 'limit-exceeded',
			message: 'overscan exceeds the negotiated row-window limit',
			operation: 'history',
			requestSequence: overscanned.payload.requestSequence,
			retryable: true,
		});

		expect(commands.at(-1)).toEqual(
			expect.objectContaining({
				kind: 'read-rows',
				payload: expect.objectContaining({
					origin: { kind: 'tail', rowsBefore: 1 },
					revisionPolicy: 'current-view',
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
			payload: { attachmentId, bracketed: false, capabilityRevision, text: 'safe' },
		});
		await expect(terminal.paste('one\ntwo')).resolves.toEqual(
			expect.objectContaining({ kind: 'blocked' }),
		);
		expect(reviewPaste).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('publishes a safe prepared file drop directly but retains review for suspicious input', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const reviewPaste = vi.fn<(review: ProjectedPasteReview) => boolean>(() => false);
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
		const text = '/tmp/safe.txt';

		await expect(terminal.paste(text, 'file-drop')).resolves.toMatchObject({ kind: 'sent' });
		expect(reviewPaste).not.toHaveBeenCalled();
		expect(commands.at(-1)).toEqual({
			kind: 'paste-input',
			payload: { attachmentId, bracketed: false, capabilityRevision, text },
		});

		await expect(terminal.paste('one\ntwo', 'file-drop')).resolves.toMatchObject({
			kind: 'blocked',
		});
		expect(reviewPaste).toHaveBeenCalledExactlyOnceWith(
			expect.objectContaining({ multiline: true, source: 'file-drop' }),
			expect.any(AbortSignal),
		);
		expect(commands.filter((command) => command.kind === 'paste-input')).toHaveLength(1);
		terminal.dispose();
	});

	it('routes native paste once from terminal-owned targets and preserves application editing', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['one', 'two']));
		const root = browser.root as unknown as FakeElement;
		const input = terminal.textarea as unknown as FakeElement;
		const accessibilityRow = root.children[2]?.children[0];
		if (accessibilityRow === undefined) throw new Error('accessibility row is unavailable');

		const dispatchPaste = (target: FakeElement, text: string) => {
			let propagationStopped = false;
			const preventDefault = vi.fn<() => void>();
			const stopPropagation = vi.fn<() => void>(() => {
				propagationStopped = true;
			});
			target.dispatch('paste', {
				bubbles: true,
				clipboardData: {
					getData: (type: string) => (type === 'text/plain' ? text : ''),
					items: [],
				},
				get cancelBubble() {
					return propagationStopped;
				},
				preventDefault,
				stopPropagation,
				target,
				type: 'paste',
			} as unknown as ClipboardEvent);
			return { preventDefault, stopPropagation };
		};

		commands.length = 0;
		const textareaEvent = dispatchPaste(input, 'textarea paste');
		await vi.waitFor(() =>
			expect(commands.filter((command) => command.kind === 'paste-input')).toHaveLength(1),
		);
		expect(textareaEvent.preventDefault).toHaveBeenCalledOnce();
		expect(textareaEvent.stopPropagation).toHaveBeenCalledOnce();

		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: canonicalText('0') },
			preview: 'one',
			rectangle: false,
			start: { column: 0, row: canonicalText('0') },
			state: 'active',
		});
		commands.length = 0;
		const projectionEvent = dispatchPaste(accessibilityRow, 'projection paste');
		await vi.waitFor(() =>
			expect(commands.filter((command) => command.kind === 'paste-input')).toHaveLength(1),
		);
		expect(
			commands
				.filter((command) => command.kind === 'selection-command' || command.kind === 'paste-input')
				.map((command) =>
					command.kind === 'selection-command'
						? `${command.kind}:${command.payload.command}`
						: `${command.kind}:${command.payload.text}`,
				),
		).toEqual(['selection-command:clear', 'paste-input:projection paste']);
		expect(projectionEvent.preventDefault).toHaveBeenCalledOnce();
		expect(projectionEvent.stopPropagation).toHaveBeenCalledOnce();

		const applicationInput = browser.document.createElement('input') as unknown as FakeElement;
		root.appendChild(applicationInput);
		commands.length = 0;
		const applicationEvent = dispatchPaste(applicationInput, 'application paste');
		expect(applicationEvent.preventDefault).not.toHaveBeenCalled();
		expect(applicationEvent.stopPropagation).not.toHaveBeenCalled();
		expect(commands).toEqual([]);
		terminal.dispose();
	});

	it('signals an image-only clipboard paste to the foreground TUI exactly once', async () => {
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
		const preventDefault = vi.fn<() => void>();
		const stopPropagation = vi.fn<() => void>();

		input.dispatch('paste', {
			clipboardData: {
				getData: () => '',
				items: [{ kind: 'file', type: 'image/png' }],
			},
			preventDefault,
			stopPropagation,
			type: 'paste',
		} as unknown as ClipboardEvent);

		expect(
			commands
				.filter((command) => command.kind === 'key-input')
				.map((command) => ({
					action: command.payload.action,
					control: command.payload.modifiers.control,
					key: command.payload.key,
					text: command.payload.text,
					unshiftedCodepoint: command.payload.unshiftedCodepoint,
				})),
		).toEqual([
			{ action: KeyAction.press, control: true, key: 'KeyV', text: 'v', unshiftedCodepoint: 118 },
			{ action: KeyAction.release, control: true, key: 'KeyV', text: 'v', unshiftedCodepoint: 118 },
		]);
		expect(preventDefault).toHaveBeenCalledOnce();
		expect(stopPropagation).toHaveBeenCalledOnce();
		expect(commands.some((command) => command.kind === 'paste-input')).toBe(false);
		terminal.dispose();
	});

	it('keeps text paste authoritative when the clipboard also contains an image', async () => {
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

		input.dispatch('paste', {
			clipboardData: {
				getData: (type: string) => (type === 'text/plain' ? 'caption' : ''),
				items: [
					{ kind: 'string', type: 'text/plain' },
					{ kind: 'file', type: 'image/png' },
				],
			},
			preventDefault: () => undefined,
			stopPropagation: () => undefined,
			type: 'paste',
		} as unknown as ClipboardEvent);

		await vi.waitFor(() =>
			expect(commands.some((command) => command.kind === 'paste-input')).toBe(true),
		);
		expect(commands.filter((command) => command.kind === 'key-input')).toEqual([]);
		terminal.dispose();
	});

	it('reviews the original bracketed-paste terminator and publishes the reviewed mode', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const reviews: unknown[] = [];
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
			reviewPaste: (review) => {
				reviews.push(review);
				return true;
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n, {
				...interaction,
				bracketedPaste: true,
			}),
		);

		const originalText = 'prefix\u001b[201~suffix';
		await expect(terminal.paste(originalText)).resolves.toEqual(
			expect.objectContaining({ kind: 'sent' }),
		);
		expect(reviews).toEqual([
			expect.objectContaining({
				bracketed: true,
				embeddedBracketTerminator: true,
				originalText,
				replacedControlCharacters: 1,
				text: 'prefix [201~suffix',
			}),
		]);
		expect(commands.at(-1)).toEqual({
			kind: 'paste-input',
			payload: {
				attachmentId,
				bracketed: true,
				capabilityRevision,
				text: 'prefix [201~suffix',
			},
		});
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
				payload: { attachmentId, bracketed: false, capabilityRevision, text: 'dropped text' },
			}),
		);
		expect(preventDropDefault).toHaveBeenCalledOnce();
		expect(stopPropagation).toHaveBeenCalledOnce();

		const protectedFileDragDefault = vi.fn<() => void>();
		const protectedFileDropDefault = vi.fn<() => void>();
		const protectedFileStopPropagation = vi.fn<() => void>();
		const protectedFileTransfer = {
			dropEffect: 'none',
			files: [],
			getData: () => '/untrusted/browser/path',
			types: ['Files', 'text/plain'],
		};
		root.dispatch('dragover', {
			dataTransfer: protectedFileTransfer,
			preventDefault: protectedFileDragDefault,
			type: 'dragover',
		} as unknown as DragEvent);
		root.dispatch('drop', {
			dataTransfer: protectedFileTransfer,
			preventDefault: protectedFileDropDefault,
			stopPropagation: protectedFileStopPropagation,
			type: 'drop',
		} as unknown as DragEvent);
		expect(protectedFileDragDefault).not.toHaveBeenCalled();
		expect(protectedFileDropDefault).not.toHaveBeenCalled();
		expect(protectedFileStopPropagation).not.toHaveBeenCalled();

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

	it('immediately releases Host-published key, mouse, and focus ownership when disposal invalidates a review', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let finishReview: ((approved: boolean) => void) | undefined;
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
			reviewPaste: async () =>
				await new Promise<boolean>((resolve) => {
					finishReview = resolve;
				}),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n, {
				...interaction,
				mouseAny: true,
			}),
		);
		const input = terminal.textarea as unknown as FakeElement;
		const canvas = terminal.canvas as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));
		const paste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(finishReview).toBeTypeOf('function'));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 2, 2));

		terminal.dispose();
		const cleanup = commands
			.filter(
				(command) =>
					(command.kind === 'key-input' && command.payload.action === KeyAction.release) ||
					(command.kind === 'mouse-input' && command.payload.action === MouseAction.release) ||
					(command.kind === 'focus-input' && !command.payload.focused),
			)
			.map((command) => {
				if (command.kind === 'key-input') return `key:${command.payload.key}`;
				if (command.kind === 'mouse-input') return `mouse:${command.payload.button}`;
				return 'focus:false';
			});
		expect(cleanup).toEqual(['key:KeyA', `mouse:${MouseButton.left}`, 'focus:false']);

		finishReview?.(true);
		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
		await Promise.resolve();
		expect(
			commands.filter(
				(command) =>
					command.kind === 'paste-input' ||
					command.kind === 'key-input' ||
					command.kind === 'mouse-input',
			),
		).toEqual([
			expect.objectContaining({
				kind: 'key-input',
				payload: expect.objectContaining({ action: KeyAction.press }),
			}),
			expect.objectContaining({
				kind: 'mouse-input',
				payload: expect.objectContaining({ action: MouseAction.press }),
			}),
			expect.objectContaining({
				kind: 'key-input',
				payload: expect.objectContaining({ action: KeyAction.release }),
			}),
			expect.objectContaining({
				kind: 'mouse-input',
				payload: expect.objectContaining({ action: MouseAction.release }),
			}),
		]);
	});

	it('does not release queued key, mouse, or focus ownership that the Host never observed', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let finishReview: ((approved: boolean) => void) | undefined;
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
			reviewPaste: async () =>
				await new Promise<boolean>((resolve) => {
					finishReview = resolve;
				}),
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		const input = terminal.textarea as unknown as FakeElement;
		const canvas = terminal.canvas as unknown as FakeElement;
		const paste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(finishReview).toBeTypeOf('function'));
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));

		terminal.dispose();
		expect(
			commands.filter(
				(command) =>
					command.kind === 'key-input' ||
					command.kind === 'mouse-input' ||
					command.kind === 'focus-input',
			),
		).toEqual([]);
		finishReview?.(true);
		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
		await Promise.resolve();
		expect(
			commands.filter(
				(command) =>
					command.kind === 'paste-input' ||
					command.kind === 'key-input' ||
					command.kind === 'mouse-input' ||
					command.kind === 'focus-input',
			),
		).toEqual([]);
	});

	it('cancels a rejected paste review without an unhandled continuation or stale output', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let rejectReview: ((error: Error) => void) | undefined;
		const commands: ProjectedTerminalCommand[] = [];
		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
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
				await new Promise<boolean>((_resolve, reject) => {
					rejectReview = reject;
				}),
			terminalId,
		});
		terminal.onError(errors);
		await terminal.open(browser.root);
		const paste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(rejectReview).toBeTypeOf('function'));

		terminal.dispose();
		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
		rejectReview?.(new Error('late paste review rejection'));
		await Promise.resolve();
		await Promise.resolve();
		expect(errors).not.toHaveBeenCalled();
		expect(commands.some((command) => command.kind === 'paste-input')).toBe(false);
	});

	it.each(['textarea-blur', 'visibility-hidden'])(
		'lets %s repair bypass a paste review that never settles',
		async (trigger) => {
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
				reviewPaste: async () => await new Promise<boolean>(() => undefined),
				terminalId,
			});
			await terminal.open(browser.root);
			const input = terminal.textarea as unknown as FakeElement;
			input.dispatch('focus', { type: 'focus' } as FocusEvent);
			input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
			const paste = terminal.paste('one\ntwo');
			await Promise.resolve();

			if (trigger === 'visibility-hidden') {
				browser.setVisibilityState('hidden');
				browser.dispatchDocument('visibilitychange');
			} else {
				input.dispatch('blur', { type: 'blur' } as FocusEvent);
			}
			await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
			expect(
				commands
					.filter(
						(command) =>
							(command.kind === 'key-input' && command.payload.action === KeyAction.release) ||
							(command.kind === 'focus-input' && !command.payload.focused),
					)
					.map((command) =>
						command.kind === 'key-input' ? `key:${command.payload.key}` : 'focus:false',
					),
			).toEqual(['key:KeyA', 'focus:false']);
			if (trigger === 'visibility-hidden') {
				browser.setVisibilityState('visible');
				browser.dispatchDocument('visibilitychange');
			}
			input.dispatch('focus', { type: 'focus' } as FocusEvent);
			input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
			expect(
				commands.some(
					(command) =>
						command.kind === 'key-input' &&
						command.payload.action === KeyAction.press &&
						command.payload.key === 'KeyB',
				),
			).toBe(true);
			terminal.dispose();
		},
	);

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
		input.dispatch('keydown', {
			altKey: false,
			code: 'KeyB',
			ctrlKey: false,
			getModifierState: () => false,
			isComposing: false,
			key: 'b',
			keyCode: 66,
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
						? `${command.kind}:${command.payload.action}:${command.payload.key}`
						: `${command.kind}:${command.payload.focused}`,
				),
		).toEqual([
			'focus-input:true',
			'key-input:press:KeyA',
			'key-input:press:KeyB',
			'key-input:release:KeyA',
			'key-input:release:KeyB',
			'focus-input:false',
		]);
		terminal.dispose();
	});

	it('repairs focus only after focus leaves the complete terminal surface', async () => {
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
		const internal = browser.document.createElement('button');
		browser.root.appendChild(internal);
		const outside = browser.document.createElement('button');
		input.dispatch('focus', { relatedTarget: outside, type: 'focus' } as unknown as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		input.dispatch('blur', {
			relatedTarget: internal,
			type: 'blur',
		} as unknown as FocusEvent);
		expect(
			commands
				.filter((command) => command.kind === 'key-input' || command.kind === 'focus-input')
				.map((command) =>
					command.kind === 'key-input'
						? `key:${command.payload.action}`
						: `focus:${command.payload.focused}`,
				),
		).toEqual(['focus:true', 'key:press']);

		(browser.root as unknown as FakeElement).dispatch('focusout', {
			relatedTarget: outside,
			target: internal,
			type: 'focusout',
		} as unknown as FocusEvent);
		expect(
			commands
				.filter((command) => command.kind === 'key-input' || command.kind === 'focus-input')
				.map((command) =>
					command.kind === 'key-input'
						? `key:${command.payload.action}`
						: `focus:${command.payload.focused}`,
				),
		).toEqual(['focus:true', 'key:press', 'key:release', 'focus:false']);

		(browser.root as unknown as FakeElement).dispatch('focusin', {
			relatedTarget: outside,
			target: internal,
			type: 'focusin',
		} as unknown as FocusEvent);
		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false, true]);
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

	it('closes key-release repair against synchronous port reentry', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'key-input' ||
						command.payload.action !== KeyAction.release
					) {
						return;
					}
					reentered = true;
					input.dispatch('keydown', keyboardEvent('keydown', 'KeyC', 'c', 67));
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
		input.dispatch('blur', { type: 'blur' } as FocusEvent);

		expect(
			commands
				.filter((command) => command.kind === 'key-input' || command.kind === 'focus-input')
				.map((command) =>
					command.kind === 'key-input'
						? `${command.payload.action}:${command.payload.key}`
						: `focus:${command.payload.focused}`,
				),
		).toEqual([
			'focus:true',
			`${KeyAction.press}:KeyA`,
			`${KeyAction.press}:KeyB`,
			`${KeyAction.release}:KeyA`,
			`${KeyAction.release}:KeyB`,
			'focus:false',
		]);
		terminal.dispose();
	});

	it('closes focus-out repair against synchronous port reentry', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'focus-input' ||
						command.payload.focused
					) {
						return;
					}
					reentered = true;
					input.dispatch('focus', { type: 'focus' } as FocusEvent);
					input.dispatch('keydown', keyboardEvent('keydown', 'KeyC', 'c', 67));
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('blur', { type: 'blur' } as FocusEvent);

		expect(
			commands
				.filter((command) => command.kind === 'key-input' || command.kind === 'focus-input')
				.map((command) => command.kind),
		).toEqual(['focus-input', 'focus-input']);
		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false]);
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
				mouseAny: true,
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
		(terminal.canvas as unknown as FakeElement).dispatch('pointerdown', {
			altKey: false,
			button: 2,
			buttons: 3,
			clientX: 3,
			clientY: 3,
			ctrlKey: false,
			metaKey: false,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointerdown',
		} as unknown as PointerEvent);
		(terminal.canvas as unknown as FakeElement).dispatch('pointermove', {
			altKey: false,
			button: 0,
			buttons: 3,
			clientX: 4,
			clientY: 4,
			ctrlKey: false,
			metaKey: false,
			preventDefault: () => {},
			shiftKey: false,
			type: 'pointermove',
		} as unknown as PointerEvent);
		(terminal.textarea as unknown as FakeElement).dispatch('blur', {
			type: 'blur',
		} as FocusEvent);

		const mouse = commands.filter((command) => command.kind === 'mouse-input');
		expect(mouse.map((command) => `${command.payload.action}:${command.payload.button}`)).toEqual([
			`${MouseAction.press}:${MouseButton.left}`,
			`${MouseAction.press}:${MouseButton.right}`,
			`${MouseAction.motion}:${MouseButton.left}`,
			`${MouseAction.release}:${MouseButton.left}`,
			`${MouseAction.release}:${MouseButton.right}`,
		]);
		expect(
			mouse
				.filter((command) => command.payload.action === MouseAction.release)
				.map((command) => command.payload.pressedButtons),
		).toEqual([2, 0]);
		expect(mouse.at(-1)?.payload).toMatchObject({
			button: MouseButton.right,
			pressedButtons: 0,
		});
		terminal.dispose();
	});

	it('keeps application mouse ownership through projected mode and modifier changes', async () => {
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
				mouseAny: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));
		terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['a', 'b'], 1n, [0, 1], 0n, interaction));
		canvas.dispatch('pointermove', {
			...pointerEvent('pointermove', 0, 1, 3, 3),
			shiftKey: true,
		} as PointerEvent);
		canvas.dispatch('pointerup', {
			...pointerEvent('pointerup', 0, 0, 3, 3),
			shiftKey: true,
		} as PointerEvent);

		expect(
			commands
				.filter((command) => command.kind === 'mouse-input')
				.map((command) => command.payload.action),
		).toEqual([MouseAction.press, MouseAction.motion, MouseAction.release]);
		terminal.dispose();
	});

	it('coalesces non-pixel application mouse motion within one terminal cell', async () => {
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
				mouseAny: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 2, 2));
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 3, 3));
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 12, 3));

		expect(
			commands.filter(
				(command) =>
					command.kind === 'mouse-input' && command.payload.action === MouseAction.motion,
			),
		).toHaveLength(2);
		terminal.dispose();
	});

	it('retries same-cell hover after synchronous rejection', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let rejectNextHover = true;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (
						rejectNextHover &&
						command.kind === 'mouse-input' &&
						command.payload.action === MouseAction.motion
					) {
						rejectNextHover = false;
						throw new Error('reject hover');
					}
					commands.push(command);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 2, 2));
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 3, 3));

		expect(
			commands.filter(
				(command) =>
					command.kind === 'mouse-input' && command.payload.action === MouseAction.motion,
			),
		).toHaveLength(1);
		terminal.dispose();
	});

	it('publishes the first same-cell hover after mouse tracking is re-enabled', async () => {
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
				mouseAny: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 2, 2));
		terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['a', 'b'], 1n, [0, 1], 0n, interaction));
		terminal.applyFrame(
			terminalFrame('delta', 3n, 3n, ['a', 'b'], 2n, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 3, 3));

		expect(
			commands.filter(
				(command) =>
					command.kind === 'mouse-input' && command.payload.action === MouseAction.motion,
			),
		).toHaveLength(2);
		terminal.dispose();
	});

	it('preserves every same-cell application mouse motion in SGR pixel mode', async () => {
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
				mouseAny: true,
				mouseSgrPixels: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 2, 2));
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 3, 3));

		expect(
			commands.filter(
				(command) =>
					command.kind === 'mouse-input' && command.payload.action === MouseAction.motion,
			),
		).toHaveLength(2);
		terminal.dispose();
	});

	it.each(['buttonless-move', 'pointer-cancel', 'lost-capture'] as const)(
		'repairs application mouse ownership after %s',
		async (repair) => {
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
					mouseAny: true,
				}),
			);
			const canvas = terminal.canvas as unknown as FakeElement;
			const input = terminal.textarea as unknown as FakeElement;
			canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));
			const capturedDuringPress = canvas.capturedPointers.has(1);
			let eventType: 'lostpointercapture' | 'pointercancel' | 'pointermove';
			if (repair === 'buttonless-move') eventType = 'pointermove';
			else if (repair === 'pointer-cancel') eventType = 'pointercancel';
			else eventType = 'lostpointercapture';
			if (repair === 'lost-capture') canvas.capturedPointers.delete(1);
			canvas.dispatch(eventType, pointerEvent(eventType, 0, 0, 3, 3));

			expect(
				commands.filter(
					(command) =>
						command.kind === 'mouse-input' && command.payload.action === MouseAction.release,
				),
			).toHaveLength(1);
			expect(capturedDuringPress).toBe(true);
			expect(canvas.capturedPointers.has(1)).toBe(false);
			input.dispatch('blur', { type: 'blur' } as FocusEvent);
			expect(
				commands.filter(
					(command) =>
						command.kind === 'mouse-input' && command.payload.action === MouseAction.release,
				),
			).toHaveLength(1);
			terminal.dispose();
		},
	);

	it('derives pointer shape from selection, rectangle, and application mouse ownership', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', { platform: 'Linux x86_64' });
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => {} },
			terminalId,
		});
		await terminal.open(browser.root);
		const canvas = terminal.canvas as unknown as FakeElement;
		expect(canvas.style['cursor']).toBe('text');

		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		expect(canvas.style['cursor']).toBe('default');

		canvas.dispatch('pointermove', {
			...pointerEvent('pointermove', 0, 0, -1, -1),
			shiftKey: true,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('text');
		canvas.dispatch('pointermove', {
			...pointerEvent('pointermove', 0, 0, -1, -1),
			altKey: true,
			ctrlKey: true,
			shiftKey: true,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('crosshair');

		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));
		expect(canvas.style['cursor']).toBe('default');
		canvas.dispatch('pointermove', {
			...pointerEvent('pointermove', 0, 1, 3, 3),
			altKey: true,
			ctrlKey: true,
			shiftKey: true,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('default');
		canvas.dispatch('pointerup', {
			...pointerEvent('pointerup', 0, 0, 3, 3),
			altKey: true,
			ctrlKey: true,
			shiftKey: true,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('crosshair');

		canvas.dispatch('pointerdown', {
			...pointerEvent('pointerdown', 0, 1, 2, 2),
			detail: 1,
			pointerId: 2,
			shiftKey: true,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('text');
		canvas.dispatch('pointermove', {
			...pointerEvent('pointermove', 0, 1, 3, 3),
			pointerId: 2,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('text');
		canvas.dispatch('pointermove', {
			...pointerEvent('pointermove', 0, 1, 4, 4),
			altKey: true,
			ctrlKey: true,
			pointerId: 2,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('crosshair');
		canvas.dispatch('pointerup', {
			...pointerEvent('pointerup', 0, 0, 4, 4),
			altKey: true,
			ctrlKey: true,
			pointerId: 2,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('default');
		terminal.dispose();
	});

	it('abandons pointer input when focus reporting reenters focus repair', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'focus-input' ||
						!command.payload.focused
					) {
						return;
					}
					reentered = true;
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
					input.dispatch('focus', { type: 'focus' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		input = terminal.textarea as unknown as FakeElement;
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));

		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false, true]);
		expect(commands.filter((command) => command.kind === 'mouse-input')).toEqual([]);
		terminal.dispose();
	});

	it('closes mouse-release repair against synchronous port reentry', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let canvas: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						canvas === undefined ||
						reentered ||
						command.kind !== 'mouse-input' ||
						command.payload.action !== MouseAction.release
					) {
						return;
					}
					reentered = true;
					canvas.dispatch('pointerdown', pointerEvent('pointerdown', 1, 4, 5, 5));
					input.dispatch('blur', { type: 'blur' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		input = terminal.textarea as unknown as FakeElement;
		canvas = terminal.canvas as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 2, 3, 3, 3));
		input.dispatch('blur', { type: 'blur' } as FocusEvent);

		expect(
			commands
				.filter((command) => command.kind === 'mouse-input')
				.map((command) => `${command.payload.action}:${command.payload.button}`),
		).toEqual([
			`${MouseAction.press}:${MouseButton.left}`,
			`${MouseAction.press}:${MouseButton.right}`,
			`${MouseAction.release}:${MouseButton.left}`,
			`${MouseAction.release}:${MouseButton.right}`,
		]);
		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false]);
		terminal.dispose();
	});

	it('gates disposal before synchronous cleanup sends can admit new input', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let input: FakeElement | undefined;
		let reentered = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (
						input === undefined ||
						reentered ||
						command.kind !== 'key-input' ||
						command.payload.action !== KeyAction.release
					) {
						return;
					}
					reentered = true;
					input.dispatch('keydown', keyboardEvent('keydown', 'KeyC', 'c', 67));
					input.dispatch('focus', { type: 'focus' } as FocusEvent);
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		terminal.dispose();

		expect(
			commands
				.filter((command) => command.kind === 'key-input' || command.kind === 'focus-input')
				.map((command) =>
					command.kind === 'key-input'
						? `${command.payload.action}:${command.payload.key}`
						: `focus:${command.payload.focused}`,
				),
		).toEqual([
			'focus:true',
			`${KeyAction.press}:KeyA`,
			`${KeyAction.release}:KeyA`,
			'focus:false',
		]);
		expect(terminal.phase).toBe('disposed');
	});

	it('disposes after completed blur without publishing a second focus-out', async () => {
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
		input.dispatch('blur', { type: 'blur' } as FocusEvent);
		terminal.dispose();

		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false]);
		expect(terminal.phase).toBe('disposed');
	});

	it('defers reentrant disposal until the active focus-out repair is complete', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let terminal: ProjectedTerminal | undefined;
		let reentered = false;
		terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
					if (reentered || command.kind !== 'focus-input' || command.payload.focused) {
						return;
					}
					reentered = true;
					terminal?.dispose();
				},
			},
			terminalId,
		});
		await terminal.open(browser.root);
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('blur', { type: 'blur' } as FocusEvent);

		expect(
			commands
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false]);
		expect(terminal.phase).toBe('disposed');
		expect((browser.root as unknown as FakeElement).children).toHaveLength(0);
	});

	it('cancels an unresolved review so reset repairs immediately and reopens cleanly', async () => {
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
		expect(commands.some((command) => command.kind === 'reset-terminal')).toBe(true);
		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
		expect(
			commands.some(
				(command) =>
					command.kind === 'key-input' &&
					command.payload.action === KeyAction.press &&
					command.payload.key === 'KeyB',
			),
		).toBe(true);
		const commandCountBeforeLateReview = commands.length;
		finishReview?.(true);
		await Promise.resolve();
		await Promise.resolve();
		expect(commands).toHaveLength(commandCountBeforeLateReview);
		expect(commands.some((command) => command.kind === 'paste-input')).toBe(false);
		expect(
			commands.flatMap((command) =>
				command.kind === 'key-input' && command.payload.key === 'KeyA'
					? [command.payload.action]
					: [],
			),
		).toEqual([KeyAction.press, KeyAction.release]);
		expect(
			commands.findIndex(
				(command) =>
					command.kind === 'key-input' &&
					command.payload.key === 'KeyA' &&
					command.payload.action === KeyAction.release,
			),
		).toBeLessThan(commands.findIndex((command) => command.kind === 'reset-terminal'));
		terminal.dispose();
	});

	it.each(['blur', 'dispose'])(
		'coalesces selection-clear reentrant %s into the active reset cleanup',
		async (repair) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', {});
			const commands: ProjectedTerminalCommand[] = [];
			let input: FakeElement | undefined;
			let terminal: ProjectedTerminal | undefined;
			let reentered = false;
			terminal = new ProjectedTerminal({
				attachmentId,
				creationId,
				hostEpoch,
				labels: { input: 'input', screen: 'screen' },
				port: {
					send: (command) => {
						commands.push(command);
						if (
							input === undefined ||
							reentered ||
							command.kind !== 'selection-command' ||
							command.payload.command !== 'clear'
						) {
							return;
						}
						reentered = true;
						if (repair === 'dispose') terminal?.dispose();
						else input.dispatch('blur', { type: 'blur' } as FocusEvent);
					},
				},
				terminalId,
			});
			await terminal.open(browser.root);
			terminal.applyFrame(
				terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
					...interaction,
					mouseAny: true,
				}),
			);
			input = terminal.textarea as unknown as FakeElement;
			const canvas = terminal.canvas as unknown as FakeElement;
			input.dispatch('focus', { type: 'focus' } as FocusEvent);
			input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
			canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));

			terminal.reset();

			expect(
				commands
					.filter(
						(command) =>
							(command.kind === 'key-input' && command.payload.action === KeyAction.release) ||
							(command.kind === 'mouse-input' && command.payload.action === MouseAction.release) ||
							(command.kind === 'focus-input' && !command.payload.focused),
					)
					.map((command) => {
						if (command.kind === 'key-input') return `key:${command.payload.key}`;
						if (command.kind === 'mouse-input') return `mouse:${command.payload.button}`;
						return 'focus:false';
					}),
			).toEqual(['key:KeyA', `mouse:${MouseButton.left}`, 'focus:false']);
			expect(commands.some((command) => command.kind === 'reset-terminal')).toBe(repair === 'blur');
			expect(terminal.phase).toBe(repair === 'dispose' ? 'disposed' : 'open');
			expect((browser.root as unknown as FakeElement).children.length === 0).toBe(
				repair === 'dispose',
			);
			const focusOutIndex = commands.findIndex(
				(command) => command.kind === 'focus-input' && !command.payload.focused,
			);
			const resetIndex = commands.findIndex((command) => command.kind === 'reset-terminal');
			expect(focusOutIndex).toBeGreaterThanOrEqual(0);
			expect(focusOutIndex < resetIndex).toBe(repair === 'blur');
			if (repair === 'blur') {
				input.dispatch('focus', { type: 'focus' } as FocusEvent);
				input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
			}
			expect(
				commands.some(
					(command) =>
						command.kind === 'key-input' &&
						command.payload.action === KeyAction.press &&
						command.payload.key === 'KeyB',
				),
			).toBe(repair === 'blur');
			terminal.dispose();
		},
	);

	it('cancels pending paste review while reset and blur repair both make progress', async () => {
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
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		const paste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(finishReview).toBeTypeOf('function'));

		terminal.reset();
		input.dispatch('blur', { type: 'blur' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
		expect(
			commands.some((command) => command.kind === 'key-input' && command.payload.key === 'KeyB'),
		).toBe(true);
		await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
		expect(
			commands
				.filter(
					(command) =>
						command.kind === 'paste-input' ||
						(command.kind === 'key-input' && command.payload.action === KeyAction.release) ||
						(command.kind === 'focus-input' && !command.payload.focused) ||
						command.kind === 'reset-terminal',
				)
				.map((command) => {
					if (command.kind === 'key-input') return `key:${command.payload.key}`;
					if (command.kind === 'focus-input') return 'focus:false';
					return command.kind;
				}),
		).toEqual(['key:KeyA', 'reset-terminal', 'focus:false']);
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyC', 'c', 67));
		expect(
			commands.some(
				(command) =>
					command.kind === 'key-input' &&
					command.payload.action === KeyAction.press &&
					command.payload.key === 'KeyC',
			),
		).toBe(true);
		const commandCountBeforeLateReview = commands.length;
		finishReview?.(true);
		await Promise.resolve();
		await Promise.resolve();
		expect(commands).toHaveLength(commandCountBeforeLateReview);
		terminal.dispose();
	});

	it.each([
		{ inputType: 'insertLineBreak', key: 'Enter', repair: 'reset' },
		{ inputType: 'insertLineBreak', key: 'Enter', repair: 'blur' },
		{ inputType: 'insertLineBreak', key: 'Enter', repair: 'dispose' },
		{ inputType: 'deleteContentBackward', key: 'Backspace', repair: 'reset' },
		{ inputType: 'deleteContentBackward', key: 'Backspace', repair: 'blur' },
		{ inputType: 'deleteContentBackward', key: 'Backspace', repair: 'dispose' },
		{ inputType: 'deleteContentForward', key: 'Delete', repair: 'reset' },
		{ inputType: 'deleteContentForward', key: 'Delete', repair: 'blur' },
		{ inputType: 'deleteContentForward', key: 'Delete', repair: 'dispose' },
	])(
		'publishes a matched synthetic $key tap without retaining ownership during $repair',
		async ({ inputType, key, repair }) => {
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
			dispatchBeforeInput(input, '', false, inputType);

			if (repair === 'reset') terminal.reset();
			else if (repair === 'blur') input.dispatch('blur', { type: 'blur' } as FocusEvent);
			else terminal.dispose();

			expect(
				commands.flatMap((command) =>
					command.kind === 'key-input' && command.payload.key === key
						? [command.payload.action]
						: [],
				),
			).toEqual([KeyAction.press, KeyAction.release]);
			if (repair !== 'dispose') terminal.dispose();
		},
	);

	it('rolls back a throwing physical key press before draining reentrant reset', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const pressError = new Error('key press failed before acceptance');
		let terminal: ProjectedTerminal | undefined;
		let failed = false;
		terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (
						!failed &&
						command.kind === 'key-input' &&
						command.payload.action === KeyAction.press &&
						command.payload.key === 'KeyA'
					) {
						failed = true;
						terminal?.reset();
						throw pressError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));

		expect(errors).toEqual([{ error: pressError, operation: 'command' }]);
		expect(
			accepted.filter((command) => command.kind === 'key-input' && command.payload.key === 'KeyA'),
		).toEqual([]);
		expect(accepted.some((command) => command.kind === 'reset-terminal')).toBe(true);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
		terminal.reset();
		expect(
			accepted.flatMap((command) =>
				command.kind === 'key-input' && command.payload.key === 'KeyB'
					? [command.payload.action]
					: [],
			),
		).toEqual([KeyAction.press, KeyAction.release]);
		terminal.dispose();
	});

	it('rolls back a throwing physical key press without admitting an orphan release', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const pressError = new Error('key press failed before acceptance');
		let failed = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (
						!failed &&
						command.kind === 'key-input' &&
						command.payload.action === KeyAction.press &&
						command.payload.key === 'KeyA'
					) {
						failed = true;
						throw pressError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		dispatchBeforeInput(input, 'a');
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));

		expect(errors).toEqual([{ error: pressError, operation: 'command' }]);
		expect(
			accepted.filter((command) => command.kind === 'key-input' && command.payload.key === 'KeyA'),
		).toEqual([]);
		expect(
			accepted.filter((command) => command.kind === 'text-input').map((command) => command.payload),
		).toEqual([{ attachmentId, capabilityRevision, text: 'a' }]);
		terminal.dispose();
	});

	it('keeps a Host-observed key held when repeat throws and repair reenters', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const repeatError = new Error('key repeat failed before acceptance');
		let input: FakeElement | undefined;
		let failed = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (
						!failed &&
						command.kind === 'key-input' &&
						command.payload.action === KeyAction.repeat
					) {
						failed = true;
						input?.dispatch('blur', { type: 'blur' } as FocusEvent);
						throw repeatError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		input.dispatch('keydown', {
			...keyboardEvent('keydown', 'KeyA', 'a', 65),
			repeat: true,
		} as KeyboardEvent);

		expect(errors).toEqual([{ error: repeatError, operation: 'command' }]);
		expect(
			accepted.flatMap((command) =>
				command.kind === 'key-input' && command.payload.key === 'KeyA'
					? [command.payload.action]
					: [],
			),
		).toEqual([KeyAction.press, KeyAction.release]);
		terminal.dispose();
	});

	it('retries a throwing physical key release after reentrant blur owns cleanup', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const releaseError = new Error('key release failed before acceptance');
		let input: FakeElement | undefined;
		let failed = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (
						!failed &&
						command.kind === 'key-input' &&
						command.payload.action === KeyAction.release
					) {
						failed = true;
						input?.dispatch('blur', { type: 'blur' } as FocusEvent);
						throw releaseError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));

		expect(errors).toEqual([{ error: releaseError, operation: 'command' }]);
		expect(
			accepted.flatMap((command) =>
				command.kind === 'key-input' && command.payload.key === 'KeyA'
					? [command.payload.action]
					: [],
			),
		).toEqual([KeyAction.press, KeyAction.release]);
		terminal.dispose();
	});

	it.each(['press', 'release'])(
		'keeps physical mouse ownership exact when %s throws with reentrant blur',
		async (failedAction) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', {});
			const accepted: ProjectedTerminalCommand[] = [];
			const errors: ProjectedTerminalErrorEvent[] = [];
			const mouseError = new Error(`mouse ${failedAction} failed before acceptance`);
			let input: FakeElement | undefined;
			let failed = false;
			const terminal = new ProjectedTerminal({
				attachmentId,
				creationId,
				hostEpoch,
				labels: { input: 'input', screen: 'screen' },
				port: {
					send: (command) => {
						if (
							!failed &&
							command.kind === 'mouse-input' &&
							command.payload.action === failedAction
						) {
							failed = true;
							input?.dispatch('blur', { type: 'blur' } as FocusEvent);
							throw mouseError;
						}
						accepted.push(command);
					},
				},
				terminalId,
			});
			terminal.onError((event) => errors.push(event));
			await terminal.open(browser.root);
			terminal.applyFrame(
				terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
					...interaction,
					mouseAny: true,
				}),
			);
			input = terminal.textarea as unknown as FakeElement;
			const canvas = terminal.canvas as unknown as FakeElement;
			canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));
			if (failedAction === 'release') {
				canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 2, 2));
			}

			expect(errors).toEqual([{ error: mouseError, operation: 'command' }]);
			expect(
				accepted
					.filter((command) => command.kind === 'mouse-input')
					.map((command) => command.payload.action),
			).toEqual(failedAction === 'press' ? [] : [MouseAction.press, MouseAction.release]);
			terminal.dispose();
		},
	);

	it('rolls back a throwing physical mouse press without admitting an orphan release', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const mouseError = new Error('mouse press failed before acceptance');
		let failed = false;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (
						!failed &&
						command.kind === 'mouse-input' &&
						command.payload.action === MouseAction.press
					) {
						failed = true;
						throw mouseError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2));
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 2, 2));

		expect(errors).toEqual([{ error: mouseError, operation: 'command' }]);
		expect(accepted.filter((command) => command.kind === 'mouse-input')).toEqual([]);
		terminal.dispose();
	});

	it('rolls back a throwing focus-in before reentrant reset and reconciles before key input', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const focusError = new Error('focus-in failed before acceptance');
		let terminal: ProjectedTerminal | undefined;
		let failed = false;
		terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (!failed && command.kind === 'focus-input' && command.payload.focused) {
						failed = true;
						terminal?.reset();
						throw focusError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		expect(errors).toEqual([{ error: focusError, operation: 'command' }]);
		expect(
			accepted
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true]);
		const focusIndex = accepted.findIndex((command) => command.kind === 'focus-input');
		const keyIndex = accepted.findIndex(
			(command) => command.kind === 'key-input' && command.payload.key === 'KeyA',
		);
		expect(focusIndex).toBeLessThan(keyIndex);
		terminal.dispose();
	});

	it('retains Host focus after a throwing focus-out until a later repair retries it', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const focusError = new Error('focus-out failed before acceptance');
		let terminal: ProjectedTerminal | undefined;
		let failed = false;
		terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (!failed && command.kind === 'focus-input' && !command.payload.focused) {
						failed = true;
						terminal?.reset();
						throw focusError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('blur', { type: 'blur' } as FocusEvent);
		browser.dispatchWindow('blur');
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		expect(errors).toEqual([{ error: focusError, operation: 'command' }]);
		expect(
			accepted
				.filter((command) => command.kind === 'focus-input')
				.map((command) => command.payload.focused),
		).toEqual([true, false, true]);
		terminal.dispose();
	});

	it('reports one failed disposal cleanup attempt without recursive retry', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const releaseError = new Error('disposal release failed before acceptance');
		let releaseAttempts = 0;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (command.kind === 'key-input' && command.payload.action === KeyAction.release) {
						releaseAttempts++;
						throw releaseError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		(terminal.textarea as unknown as FakeElement).dispatch(
			'keydown',
			keyboardEvent('keydown', 'KeyA', 'a', 65),
		);
		terminal.dispose();

		expect(releaseAttempts).toBe(1);
		expect(errors).toEqual([{ error: releaseError, operation: 'command' }]);
		expect(
			accepted.flatMap((command) => (command.kind === 'key-input' ? [command.payload.action] : [])),
		).toEqual([KeyAction.press]);
		expect(terminal.phase).toBe('disposed');
		expect((browser.root as unknown as FakeElement).children).toHaveLength(0);
	});

	it('does not recursively retry a throwing cleanup release that reenters blur', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const accepted: ProjectedTerminalCommand[] = [];
		const errors: ProjectedTerminalErrorEvent[] = [];
		const releaseError = new Error('cleanup release failed before acceptance');
		let input: FakeElement | undefined;
		let releaseAttempts = 0;
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					if (command.kind === 'key-input' && command.payload.action === KeyAction.release) {
						releaseAttempts++;
						input?.dispatch('blur', { type: 'blur' } as FocusEvent);
						throw releaseError;
					}
					accepted.push(command);
				},
			},
			terminalId,
		});
		terminal.onError((event) => errors.push(event));
		await terminal.open(browser.root);
		input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		terminal.reset();

		expect(releaseAttempts).toBe(1);
		expect(errors).toEqual([{ error: releaseError, operation: 'command' }]);
		expect(accepted.some((command) => command.kind === 'reset-terminal')).toBe(true);
		terminal.dispose();
	});

	it('normalizes local history wheel delta modes and accumulates fractional pixels', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
		const scrollLines = vi.spyOn(terminal, 'scrollLines').mockImplementation(() => false);
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('wheel', wheelEvent(0, 8));
		canvas.dispatch('wheel', wheelEvent(0, 8));
		canvas.dispatch('wheel', wheelEvent(0, -2, { deltaMode: 1 }));
		canvas.dispatch('wheel', wheelEvent(0, 1, { deltaMode: 2 }));

		expect(scrollLines.mock.calls.map(([rows]) => rows)).toEqual([1, -2, 2]);
		terminal.dispose();
	});

	it('normalizes application wheel delta modes across both stateless axes', async () => {
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
				mouseAny: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('wheel', wheelEvent(-4, -8));
		canvas.dispatch('wheel', wheelEvent(-4, -8));
		canvas.dispatch('wheel', wheelEvent(1, 1, { deltaMode: 1 }));
		canvas.dispatch('wheel', wheelEvent(-1, 1, { deltaMode: 2 }));

		expect(
			commands.flatMap((command) =>
				command.kind === 'mouse-input' ? [command.payload.button] : [],
			),
		).toEqual([
			MouseButton.four,
			MouseButton.seven,
			MouseButton.five,
			MouseButton.six,
			MouseButton.five,
			MouseButton.five,
			MouseButton.seven,
			MouseButton.seven,
		]);
		terminal.dispose();
	});

	it('uses semantic arrow fallback for alternate-screen wheel input', async () => {
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
			terminalFrame(
				'snapshot',
				1n,
				1n,
				['a', 'b'],
				null,
				[0, 1],
				0n,
				{ ...interaction, mouseAny: true },
				{ column: 0, row: 0 },
				1n,
				2,
				ActiveScreen.alternate,
			),
		);
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('wheel', wheelEvent(0, -8, { shiftKey: true }));
		canvas.dispatch('wheel', wheelEvent(0, -8, { shiftKey: true }));
		canvas.dispatch('wheel', wheelEvent(0, 1, { deltaMode: 1, shiftKey: true }));

		expect(
			commands.flatMap((command) =>
				command.kind === 'key-input' && command.payload.action === KeyAction.press
					? [command.payload.key]
					: [],
			),
		).toEqual(['ArrowUp', 'ArrowDown']);
		expect(commands.some((command) => command.kind === 'mouse-input')).toBe(false);
		expect(commands.some((command) => command.kind === 'read-rows')).toBe(false);
		terminal.dispose();
	});

	it.each([
		{
			buttons: [MouseButton.four],
			deltaX: 0,
			deltaY: -24,
			direction: 'up',
			repair: 'reset',
		},
		{
			buttons: [MouseButton.five],
			deltaX: 0,
			deltaY: 24,
			direction: 'down',
			repair: 'blur',
		},
		{
			buttons: [MouseButton.four, MouseButton.seven],
			deltaX: -12,
			deltaY: -24,
			direction: 'up with horizontal delta',
			repair: 'dispose',
		},
		{
			buttons: [MouseButton.five, MouseButton.six],
			deltaX: 12,
			deltaY: 24,
			direction: 'down with horizontal delta',
			repair: 'reset',
		},
	])(
		'keeps stateless wheel $direction report out of held ownership during $repair',
		async ({ buttons, deltaX, deltaY, repair }) => {
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
					mouseAny: true,
				}),
			);
			const input = terminal.textarea as unknown as FakeElement;
			const canvas = terminal.canvas as unknown as FakeElement;
			canvas.dispatch('wheel', wheelEvent(deltaX, deltaY));

			if (repair === 'reset') terminal.reset();
			else if (repair === 'blur') input.dispatch('blur', { type: 'blur' } as FocusEvent);
			else terminal.dispose();

			expect(
				commands
					.filter((command) => command.kind === 'mouse-input')
					.map((command) => `${command.payload.action}:${command.payload.button}`),
			).toEqual(buttons.map((button) => `${MouseAction.press}:${button}`));
			if (repair !== 'dispose') terminal.dispose();
		},
	);

	it.each(['never settles', 'rejects after invalidation'])(
		'reconciles queued focus after a paste review that %s',
		async (reviewCase) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', {});
			const commands: ProjectedTerminalCommand[] = [];
			let rejectReview: ((error: Error) => void) | undefined;
			let reviewStarted = false;
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
				reviewPaste: async () => {
					reviewStarted = true;
					return await new Promise<boolean>((_resolve, reject) => {
						rejectReview = reject;
					});
				},
				terminalId,
			});
			await terminal.open(browser.root);
			const input = terminal.textarea as unknown as FakeElement;
			const paste = terminal.paste('one\ntwo');
			await vi.waitFor(() => expect(reviewStarted).toBe(true));
			input.dispatch('focus', { type: 'focus' } as FocusEvent);
			terminal.reset();
			await expect(paste).resolves.toEqual(expect.objectContaining({ kind: 'blocked' }));
			expect(
				commands
					.filter((command) => command.kind === 'focus-input')
					.map((command) => command.payload),
			).toEqual([{ attachmentId, capabilityRevision, focused: true }]);
			input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

			const inputCommands = commands.filter(
				(command) => command.kind === 'focus-input' || command.kind === 'key-input',
			);
			expect(inputCommands.map((command) => command.kind)).toEqual(['focus-input', 'key-input']);
			expect(inputCommands[0]).toEqual({
				kind: 'focus-input',
				payload: { attachmentId, capabilityRevision, focused: true },
			});
			if (reviewCase === 'rejects after invalidation') {
				rejectReview?.(new Error('late review failure'));
				await Promise.resolve();
				await Promise.resolve();
			}
			expect(commands.some((command) => command.kind === 'paste-input')).toBe(false);
			terminal.dispose();
		},
	);

	it.each([
		{ order: 'beforeinput-first', repair: 'reset', throws: false },
		{ order: 'beforeinput-first', repair: 'blur', throws: true },
		{ order: 'compositionend-first', repair: 'reset', throws: false },
		{ order: 'compositionend-first', repair: 'blur', throws: true },
	])(
		'preserves a new composition after $order old commit reenters $repair (throws=$throws)',
		async ({ order, repair, throws }) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', {});
			const setPreedit = vi.spyOn(WebGPURenderer.prototype, 'setPreedit');
			const accepted: ProjectedTerminalCommand[] = [];
			const errors: ProjectedTerminalErrorEvent[] = [];
			let input: FakeElement | undefined;
			let reentered = false;
			const terminal = new ProjectedTerminal({
				attachmentId,
				creationId,
				hostEpoch,
				labels: { input: 'input', screen: 'screen' },
				port: {
					send: (command) => {
						if (command.kind === 'text-input' && !reentered && input !== undefined) {
							reentered = true;
							if (repair === 'reset') terminal.reset();
							else input.dispatch('blur', { type: 'blur' } as FocusEvent);
							input.dispatch('focus', { type: 'focus' } as FocusEvent);
							input.value = '新输入';
							dispatchComposition(input, 'compositionstart', '新');
							dispatchComposition(input, 'compositionupdate', '新输入');
							if (throws) throw new Error('old text send failed');
						}
						accepted.push(command);
					},
				},
				terminalId,
			});
			terminal.onError((event) => errors.push(event));
			await terminal.open(browser.root);
			terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
			input = terminal.textarea as unknown as FakeElement;
			input.value = '旧输入';
			dispatchComposition(input, 'compositionstart', '旧');
			dispatchComposition(input, 'compositionupdate', '旧输入');
			if (order === 'beforeinput-first') dispatchBeforeInput(input, '旧输入');
			else dispatchComposition(input, 'compositionend', '旧输入');

			expect(input.value).toBe('新输入');
			expect(setPreedit).toHaveBeenLastCalledWith(
				expect.arrayContaining([expect.objectContaining({ text: '新' })]),
				expect.any(Object),
			);
			expect(errors).toHaveLength(throws ? 1 : 0);
			expect(accepted.filter((command) => command.kind === 'text-input')).toHaveLength(
				throws ? 0 : 1,
			);
			terminal.dispose();
		},
	);

	it.each(['beforeinput', 'clipboard', 'drop'])(
		'reports a current %s paste review rejection and keeps later input usable',
		async (source) => {
			const browser = createFakeBrowser();
			vi.stubGlobal('document', browser.document);
			vi.stubGlobal('window', browser.document.defaultView);
			vi.stubGlobal('navigator', {});
			const commands: ProjectedTerminalCommand[] = [];
			const errors: ProjectedTerminalErrorEvent[] = [];
			const reviewError = new Error(`${source} review failed`);
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
				reviewPaste: async () => {
					throw reviewError;
				},
				terminalId,
			});
			terminal.onError((event) => errors.push(event));
			await terminal.open(browser.root);
			const input = terminal.textarea as unknown as FakeElement;
			if (source === 'beforeinput') {
				dispatchBeforeInput(input, 'one\ntwo', false, 'insertFromPaste');
			} else if (source === 'clipboard') {
				input.dispatch('paste', {
					clipboardData: { getData: () => 'one\ntwo' },
					preventDefault: () => undefined,
					stopPropagation: () => undefined,
					type: 'paste',
				} as unknown as ClipboardEvent);
			} else {
				(browser.root as unknown as FakeElement).dispatch('drop', {
					dataTransfer: {
						files: [],
						getData: () => 'one\ntwo',
						types: ['text/plain'],
					},
					preventDefault: () => undefined,
					stopPropagation: () => undefined,
					type: 'drop',
				} as unknown as DragEvent);
			}

			await vi.waitFor(() =>
				expect(errors).toEqual([{ error: reviewError, operation: 'command' }]),
			);
			input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
			expect(
				commands.some(
					(command) =>
						command.kind === 'key-input' &&
						command.payload.action === KeyAction.press &&
						command.payload.key === 'KeyA',
				),
			).toBe(true);
			terminal.dispose();
		},
	);

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

	it('repairs live input ownership on downgrade and admits only fresh revision input after upgrade', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		let reviewSignal: AbortSignal | undefined;
		const terminal = new ProjectedTerminal({
			attachmentId,
			capabilityRevision,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: {
				send: (command) => {
					commands.push(command);
				},
			},
			reviewPaste: (_review, signal) => {
				reviewSignal = signal;
				return new Promise<boolean>(() => undefined);
			},
			terminalId,
		});
		await terminal.open(browser.root);
		terminal.applyFrame(
			terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n, {
				...interaction,
				mouseAny: true,
			}),
		);
		const input = terminal.textarea as unknown as FakeElement;
		const canvas = terminal.canvas as unknown as FakeElement;
		input.dispatch('focus', { type: 'focus' } as FocusEvent);
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId: 7 }));
		input.value = '候选';
		dispatchComposition(input, 'compositionstart', '候');
		dispatchComposition(input, 'compositionupdate', '候选');
		const pendingPaste = terminal.paste('one\ntwo');
		await vi.waitFor(() => expect(reviewSignal).toBeDefined());
		expect(canvas.capturedPointers.has(7)).toBe(true);
		expect(
			commands.some(
				(command) => command.kind === 'key-input' && command.payload.action === KeyAction.press,
			),
		).toBe(true);
		expect(
			commands.some(
				(command) => command.kind === 'mouse-input' && command.payload.action === MouseAction.press,
			),
		).toBe(true);

		expect(
			terminal.applyInputCapability({
				attachmentId,
				capabilityRevision: canonicalText('2'),
				input: false,
				terminalId,
			}),
		).toEqual({ kind: 'applied' });
		expect(terminal.canControlTerminal).toBe(false);
		expect(input.getAttribute('readonly')).toBe('');
		expect(input.value).toBe('');
		expect(canvas.capturedPointers.has(7)).toBe(false);
		expect(reviewSignal?.aborted).toBe(true);
		await expect(pendingPaste).resolves.toMatchObject({ kind: 'blocked' });
		const inputCommandCountAfterDowngrade = commands.filter((command) =>
			['focus-input', 'key-input', 'mouse-input', 'paste-input', 'text-input'].includes(
				command.kind,
			),
		).length;

		input.dispatch('keyup', keyboardEvent('keyup', 'KeyA', 'a', 65));
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 2, 2, { pointerId: 7 }));
		dispatchComposition(input, 'compositionend', '候选');
		dispatchBeforeInput(input, '候选', false, 'insertFromComposition');
		terminal.selectAll();
		terminal.scrollLines(-1);
		expect(
			commands.filter((command) =>
				['focus-input', 'key-input', 'mouse-input', 'paste-input', 'text-input'].includes(
					command.kind,
				),
			),
		).toHaveLength(inputCommandCountAfterDowngrade);
		expect(commands.some((command) => command.kind === 'selection-command')).toBe(true);
		expect(commands.some((command) => command.kind === 'read-rows')).toBe(true);

		expect(
			terminal.applyInputCapability({
				attachmentId,
				capabilityRevision: canonicalText('3'),
				input: true,
				terminalId,
			}),
		).toEqual({ kind: 'applied' });
		expect(terminal.canControlTerminal).toBe(true);
		expect(input.getAttribute('readonly')).toBeNull();
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyB', 'b', 66));
		input.dispatch('keyup', keyboardEvent('keyup', 'KeyB', 'b', 66));
		expect(
			commands
				.filter(
					(command): command is Extract<ProjectedTerminalCommand, { kind: 'key-input' }> =>
						command.kind === 'key-input' && command.payload.key === 'KeyB',
				)
				.map((command) => command.payload.capabilityRevision),
		).toEqual(['3', '3']);
		expect(commands.some((command) => command.kind === 'paste-input')).toBe(false);
		expect(commands.some((command) => command.kind === 'text-input')).toBe(false);
		terminal.dispose();
	});

	it('keeps committed capability history distinct from fail-closed local admission', () => {
		const terminal = new ProjectedTerminal({
			attachmentId,
			capabilityRevision,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => undefined },
			terminalId,
		});
		const update = (revision: string, input: boolean, updateAttachmentId = attachmentId) =>
			terminal.applyInputCapability({
				attachmentId: updateAttachmentId,
				capabilityRevision: canonicalText(revision),
				input,
				terminalId,
			});

		expect(update('1', true)).toEqual({ kind: 'replayed' });
		expect(update('2', false)).toEqual({ kind: 'applied' });
		expect(update('1', true)).toEqual({ kind: 'stale' });
		expect(update('2', false)).toEqual({ kind: 'replayed' });
		expect(update('2', true)).toEqual({ kind: 'rejected', reason: 'revision-conflict' });
		expect(update('3', true)).toEqual({ kind: 'applied' });
		expect(terminal.canControlTerminal).toBe(true);

		expect(update('5', false)).toEqual({ kind: 'rejected', reason: 'revision-gap' });
		expect(terminal.canControlTerminal).toBe(false);
		expect(update('3', true)).toEqual({ kind: 'replayed' });
		expect(terminal.canControlTerminal).toBe(false);
		expect(update('4', false)).toEqual({ kind: 'applied' });
		expect(update('5', true)).toEqual({ kind: 'applied' });
		expect(terminal.canControlTerminal).toBe(true);

		expect(update('6', false, otherAttachmentId)).toEqual({
			kind: 'rejected',
			reason: 'identity-mismatch',
		});
		expect(terminal.canControlTerminal).toBe(false);
		expect(update('5', true)).toEqual({ kind: 'replayed' });
		expect(terminal.canControlTerminal).toBe(false);
		expect(update('6', false)).toEqual({ kind: 'applied' });
		expect(update('7', false)).toEqual({
			kind: 'rejected',
			reason: 'revision-conflict',
		});
		expect(update('7', true)).toEqual({ kind: 'applied' });
		expect(terminal.canControlTerminal).toBe(true);
		expect(update('0', false)).toEqual({ kind: 'rejected', reason: 'invalid-revision' });
		expect(terminal.canControlTerminal).toBe(false);

		terminal.dispose();
	});

	it('cancels pending native text settlement at the capability revision boundary', async () => {
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
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'Digit1', '1', 49));
		input.value = '1';

		terminal.applyInputCapability({
			attachmentId,
			capabilityRevision: canonicalText('2'),
			input: false,
			terminalId,
		});
		await vi.advanceTimersByTimeAsync(31);

		expect(commands.some((command) => command.kind === 'text-input')).toBe(false);
		expect(input.value).toBe('');
		terminal.dispose();
	});

	it('installs drop admission for an initially read-only attachment that is later upgraded', async () => {
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

		terminal.applyInputCapability({
			attachmentId,
			capabilityRevision: canonicalText('2'),
			input: true,
			terminalId,
		});
		(browser.root as unknown as FakeElement).dispatch('drop', {
			dataTransfer: {
				files: [],
				getData: () => 'fresh drop',
				types: ['text/plain'],
			},
			preventDefault: () => undefined,
			stopPropagation: () => undefined,
			type: 'drop',
		} as unknown as DragEvent);
		await vi.waitFor(() =>
			expect(commands).toContainEqual({
				kind: 'paste-input',
				payload: {
					attachmentId,
					bracketed: false,
					capabilityRevision: '2',
					text: 'fresh drop',
				},
			}),
		);
		terminal.dispose();
	});

	it('makes a natural exit read-only while preserving local output interaction', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['done', '']));
		terminal.notifyExited(exitEvent('1'));

		expect(terminal.canControlTerminal).toBe(false);
		expect(terminal.textarea?.getAttribute('readonly')).toBe('');
		const commandCount = commands.length;
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		terminal.clear();
		terminal.reset();
		(browser.root as unknown as FakeElement).clientWidth += 80;
		browser.dispatchWindow('resize');
		expect(commands).toHaveLength(commandCount);

		terminal.selectAll();
		expect(commands.at(-1)).toEqual({
			kind: 'selection-command',
			payload: { attachmentId, command: 'select-all', generation: '1', revision: '1' },
		});
		terminal.dispose();
	});

	it('makes a runtime failure read-only while preserving local output interaction', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['failed', '']));
		terminal.notifyFailed(failureEvent('1'));

		expect(terminal.canControlTerminal).toBe(false);
		expect(terminal.textarea?.getAttribute('readonly')).toBe('');
		const commandCount = commands.length;
		const input = terminal.textarea as unknown as FakeElement;
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));
		terminal.clear();
		terminal.reset();
		(browser.root as unknown as FakeElement).clientWidth += 80;
		browser.dispatchWindow('resize');
		expect(commands).toHaveLength(commandCount);

		terminal.selectAll();
		expect(commands.at(-1)).toEqual({
			kind: 'selection-command',
			payload: { attachmentId, command: 'select-all', generation: '1', revision: '1' },
		});
		terminal.dispose();
	});

	it('freezes the last frame until the recovery owner replaces it', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['last readable frame', '']));
		browser.flushFrames();
		const root = browser.root as unknown as FakeElement;
		const canvas = root.children[0];
		const inputHelper = root.children[1];
		const accessibilityRoot = root.children[2];
		const commandCount = commands.length;

		terminal.freeze();

		expect(terminal.phase).toBe('frozen');
		expect(terminal.canControlTerminal).toBe(false);
		expect(root.contains(canvas)).toBe(true);
		expect(root.contains(inputHelper)).toBe(true);
		expect(root.contains(accessibilityRoot)).toBe(true);
		expect(root.children).toEqual([canvas, inputHelper, accessibilityRoot]);
		terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['replacement', ''], 1n));
		terminal.clear();
		terminal.reset();
		terminal.selectAll();
		browser.dispatchWindow('resize');
		expect(commands).toHaveLength(commandCount);

		terminal.dispose();
		expect(terminal.phase).toBe('disposed');
		expect(root.children).toHaveLength(0);
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

	it('preserves absolute endpoints across programmatic selection workflows', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 7n, ['a', 'b'], null, [0, 1], 10n));
		commands.length = 0;

		expect(terminal.setSelectionRange({ column: 1, row: 11n }, { column: 0, row: 10n }, true)).toBe(
			true,
		);
		expect(terminal.selectOutputAt({ column: 1, row: 10n })).toBe(true);
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: canonicalText('10') },
			preview: 'ab',
			rectangle: true,
			start: { column: 1, row: canonicalText('11') },
			state: 'active',
		});
		expect(terminal.getSelectionRange()).toEqual({
			end: { column: 0, row: 10n },
			rectangle: true,
			start: { column: 1, row: 11n },
		});
		expect(terminal.adjustSelectionEndpoint({ column: 1, row: 10n })).toBe(true);

		expect(commands.filter((command) => command.kind === 'selection-command')).toEqual([
			{
				kind: 'selection-command',
				payload: {
					attachmentId,
					command: 'set-range',
					end: { column: 0, row: '10' },
					generation: '1',
					rectangle: true,
					revision: '7',
					start: { column: 1, row: '11' },
				},
			},
			{
				kind: 'selection-command',
				payload: {
					attachmentId,
					command: 'select-output',
					generation: '1',
					point: { column: 1, row: '10' },
					revision: '7',
				},
			},
			{
				kind: 'selection-command',
				payload: {
					attachmentId,
					command: 'set-range',
					end: { column: 1, row: '10' },
					generation: '1',
					rectangle: true,
					revision: '7',
					start: { column: 1, row: '11' },
				},
			},
		]);

		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		expect(terminal.getSelectionRange()).toBeNull();
		expect(terminal.adjustSelectionEndpoint({ column: 0, row: 10n })).toBe(false);
		expect(terminal.setSelectionRange({ column: 2, row: 10n }, { column: 0, row: 10n })).toBe(
			false,
		);
		expect(commands.filter((command) => command.kind === 'selection-command')).toHaveLength(3);
		terminal.dispose();
	});

	it('keeps initial read-only input immutable while preserving local history paging', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b'], null, [0, 1], 10n));
		commands.length = 0;
		const input = terminal.textarea as unknown as FakeElement;
		expect(input.getAttribute('readonly')).toBe('');

		const pagingPreventDefault = vi.fn<() => void>();
		const pagingStopPropagation = vi.fn<() => void>();
		input.dispatch(
			'keydown',
			keyboardEvent('keydown', 'NumpadPageUp', 'PageUp', 33, {
				preventDefault: pagingPreventDefault,
				shiftKey: true,
				stopPropagation: pagingStopPropagation,
			}),
		);
		expect(pagingPreventDefault).toHaveBeenCalledOnce();
		expect(pagingStopPropagation).toHaveBeenCalledOnce();
		expect(commands).toContainEqual({
			kind: 'set-history-demand',
			payload: { attachmentId, demand: 'viewing-history' },
		});

		const dispatchBlockedInput = (type: string, fields: object): void => {
			const preventDefault = vi.fn<() => void>();
			const stopPropagation = vi.fn<() => void>();
			input.value = 'stale';
			input.dispatch(type, {
				...fields,
				preventDefault,
				stopPropagation,
				type,
			} as unknown as Event);
			expect(preventDefault).toHaveBeenCalledOnce();
			expect(stopPropagation).toHaveBeenCalledOnce();
			expect(input.value).toBe('');
		};
		dispatchBlockedInput('beforeinput', {
			data: 'x',
			inputType: 'insertText',
			isComposing: false,
		});
		dispatchBlockedInput('compositionstart', { data: 'x' });
		dispatchBlockedInput('paste', {
			clipboardData: { getData: () => 'x' },
		});
		input.dispatch('keydown', keyboardEvent('keydown', 'KeyA', 'a', 65));

		expect(
			commands.some((command) =>
				['focus-input', 'key-input', 'mouse-input', 'paste-input', 'text-input'].includes(
					command.kind,
				),
			),
		).toBe(false);
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

	it('settles a selection gesture across continuous revision advancement', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId: 71 }));
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 12, 2, { pointerId: 71 }));
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start']);

		terminal.applyFrame(terminalFrame('snapshot', 2n, 2n, ['new-a', 'new-b']));
		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		browser.flushFrames();

		const gestures = commands.filter((command) => command.kind === 'selection-gesture');
		expect(gestures.map((command) => command.payload.action)).toEqual(['start', 'end']);
		expect(gestures.every((command) => !('revision' in command.payload))).toBe(true);
		expect(gestures.at(-1)?.payload).toMatchObject({ generation: '1', point: { row: 0 } });
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: canonicalText('0') },
			preview: 'new',
			rectangle: false,
			start: { column: 0, row: canonicalText('0') },
			state: 'active',
		});
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(1);
		expect(errors).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('retires a gesture when its tracked selection is evicted', async () => {
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

		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId: 72 }));
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 12, 2, { pointerId: 72 }));
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start']);

		terminal.notifySelectionChanged({ attachmentId, state: 'evicted' });
		browser.flushFrames();

		expect(terminal.selection).toEqual({ attachmentId, state: 'evicted' });
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start']);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(0);
		terminal.dispose();
	});

	it('retires a generation-stale active gesture and admits a fresh gesture', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId: 73 }));
		expect(canvas.capturedPointers.has(73)).toBe(true);
		terminal.notifyHistoryChanged({
			anchorStatus: 'none',
			attachmentId,
			generation: canonicalText('2'),
			revision: canonicalText('1'),
			sequence: canonicalText('1'),
			totalRows: canonicalText('2'),
		});
		terminal.notifyBrokerFailure({
			code: 'stale-generation',
			message: 'SelectionGesture failed because the generation advanced',
			operation: 'selection',
			retryable: true,
		});

		expect(canvas.capturedPointers.has(73)).toBe(false);
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 1, 12, 2, { pointerId: 73 }));
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 12, 2, { pointerId: 73 }));
		browser.flushFrames();
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start']);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(0);

		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId: 74 }));
		const restartedGestures = commands.filter((command) => command.kind === 'selection-gesture');
		expect(restartedGestures.map((command) => command.payload.action)).toEqual(['start', 'start']);
		expect(restartedGestures.at(-1)?.payload).toMatchObject({ generation: '2' });
		expect(restartedGestures.at(-1)?.payload).not.toHaveProperty('revision');
		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 12, 2, { pointerId: 74 }));
		browser.flushFrames();
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 0, row: canonicalText('0') },
			preview: 'a',
			rectangle: false,
			start: { column: 0, row: canonicalText('0') },
			state: 'active',
		});

		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start', 'start', 'end']);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(1);
		expect(errors).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('keeps a non-stale selection failure visible', async () => {
		const terminal = new ProjectedTerminal({
			attachmentId,
			creationId,
			hostEpoch,
			labels: { input: 'input', screen: 'screen' },
			port: { send: () => {} },
			terminalId,
		});
		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		terminal.onError(errors);

		terminal.notifyBrokerFailure({
			code: 'invalid-field',
			message: 'SelectionGesture requires an active press',
			operation: 'selection',
			retryable: false,
		});

		expect(errors).toHaveBeenCalledOnce();
		expect(errors.mock.calls[0]?.[0]).toMatchObject({
			code: 'invalid-field',
			operation: 'selection',
			retryable: false,
		});
		terminal.dispose();
	});

	it('activates mobile input on touch end without consuming selection', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = new ProjectedTerminal({
			attachmentId,
			copyOnSelect: false,
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
		commands.length = 0;
		const canvas = terminal.canvas as unknown as FakeElement;
		const input = terminal.textarea as unknown as FakeElement;

		canvas.dispatch(
			'pointerdown',
			pointerEvent('pointerdown', 0, 1, 2, 2, {
				detail: 1,
				pointerId: 27,
				pointerType: 'touch',
			}),
		);
		expect(commands.some((command) => command.kind === 'focus-input')).toBe(false);
		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		canvas.dispatch(
			'pointerup',
			pointerEvent('pointerup', 0, 0, 12, 2, {
				detail: 1,
				pointerId: 27,
				pointerType: 'touch',
			}),
		);
		browser.flushFrames();
		terminal.notifySelectionChanged({
			attachmentId,
			end: { column: 1, row: canonicalText('0') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: canonicalText('0') },
			state: 'active',
		});
		commands.length = 0;

		const firstPreventDefault = vi.fn<() => void>();
		const secondPreventDefault = vi.fn<() => void>();
		canvas.dispatch('touchend', {
			preventDefault: firstPreventDefault,
			type: 'touchend',
		} as unknown as TouchEvent);
		canvas.dispatch('touchend', {
			preventDefault: secondPreventDefault,
			type: 'touchend',
		} as unknown as TouchEvent);

		expect(firstPreventDefault).toHaveBeenCalledOnce();
		expect(secondPreventDefault).toHaveBeenCalledOnce();
		expect(commands.filter((command) => command.kind === 'focus-input')).toHaveLength(1);
		expect(commands.some((command) => command.kind === 'selection-command')).toBe(false);

		dispatchBeforeInput(input, 'mobile');
		expect(
			commands.flatMap((command) => {
				if (command.kind === 'selection-command') return [command.payload.command];
				if (command.kind === 'text-input') return [command.payload.text];
				return [];
			}),
		).toEqual(['clear', 'mobile']);
		terminal.dispose();
	});

	it.each([
		'buttonless-move',
		'input-blur',
		'lost-capture',
		'pointer-cancel',
		'visibility-hidden',
		'window-blur',
	] as const)('cancels an in-flight selection update without copying after %s', async (repair) => {
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
		if (repair === 'buttonless-move') {
			canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 0, 12, 2, { pointerId: 9 }));
		} else if (repair === 'input-blur') {
			(terminal.textarea as unknown as FakeElement).dispatch('blur', {
				relatedTarget: null,
				type: 'blur',
			} as FocusEvent);
		} else if (repair === 'lost-capture') {
			canvas.capturedPointers.delete(9);
			canvas.dispatch('lostpointercapture', {
				pointerId: 9,
				type: 'lostpointercapture',
			} as unknown as PointerEvent);
		} else if (repair === 'pointer-cancel') {
			canvas.dispatch(
				'pointercancel',
				pointerEvent('pointercancel', 0, 0, 12, 2, { pointerId: 9 }),
			);
		} else if (repair === 'visibility-hidden') {
			browser.setVisibilityState('hidden');
			browser.dispatchDocument('visibilitychange');
		} else {
			browser.dispatchWindow('blur');
		}
		terminal.notifySelectionChanged(activeSelection);
		browser.flushFrames();
		terminal.notifySelectionChanged(activeSelection);

		const gestures = commands.filter((command) => command.kind === 'selection-gesture');
		expect(gestures.map((command) => command.payload.action)).toEqual([
			'start',
			'update',
			'cancel',
		]);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(0);
		terminal.dispose();
	});

	it('completes one copy after an outside release when pointer capture is unavailable', async () => {
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
		canvas.setPointerCapture = () => {
			throw new Error('pointer capture unavailable');
		};
		const activeSelection = {
			attachmentId,
			end: { column: 1, row: CanonicalUint64TextSchema.parse('1') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active' as const,
		};

		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId: 10 }));
		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		browser.dispatchDocument(
			'pointermove',
			pointerEvent('pointermove', 0, 1, 12, 361, {
				pointerId: 10,
				target: browser.root,
			}),
		);
		browser.flushFrames();
		terminal.notifySelectionChanged(activeSelection);
		const outsideRelease = pointerEvent('pointerup', 0, 0, 12, 361, {
			pointerId: 10,
			target: browser.root,
		});
		browser.dispatchDocument('pointerup', outsideRelease);
		browser.dispatchDocument('pointerup', outsideRelease);
		canvas.dispatch(
			'lostpointercapture',
			pointerEvent('lostpointercapture', 0, 0, 12, 361, {
				pointerId: 10,
			}),
		);
		browser.flushFrames();
		terminal.notifySelectionChanged(activeSelection);

		const gestures = commands.filter((command) => command.kind === 'selection-gesture');
		expect(gestures.map((command) => command.payload.action)).toEqual(['start', 'update', 'end']);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(1);
		terminal.dispose();
	});

	it('publishes the latest drag update before a rapid release while the press reply is in flight', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['alpha beta', '']));
		const canvas = terminal.canvas as unknown as FakeElement;
		const activeSelection = {
			attachmentId,
			end: { column: 4, row: CanonicalUint64TextSchema.parse('0') },
			preview: 'alpha',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active' as const,
		};

		canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId: 81 }));
		canvas.dispatch('pointermove', pointerEvent('pointermove', 0, 1, 46, 2, { pointerId: 81 }));
		canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 46, 2, { pointerId: 81 }));
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start']);

		terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		browser.flushFrames();
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start', 'update']);

		terminal.notifySelectionChanged(activeSelection);
		browser.flushFrames();
		expect(
			commands
				.filter((command) => command.kind === 'selection-gesture')
				.map((command) => command.payload.action),
		).toEqual(['start', 'update', 'end']);
		terminal.notifySelectionChanged(activeSelection);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(1);
		terminal.dispose();
	});

	it('derives repeated clicks from time and distance instead of event detail', async () => {
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
		const finishPress = (clientX: number, timeStamp: number): void => {
			terminal.notifySelectionChanged({ attachmentId, state: 'none' });
			canvas.dispatch(
				'pointerup',
				pointerEvent('pointerup', 0, 0, clientX, 2, { timeStamp: timeStamp + 10 }),
			);
			browser.flushFrames();
			terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		};
		const press = (clientX: number, timeStamp: number, detail: number, metaKey = false): void => {
			canvas.dispatch(
				'pointerdown',
				pointerEvent('pointerdown', 0, 1, clientX, 2, { detail, metaKey, timeStamp }),
			);
		};

		press(2, 1_000, 3);
		finishPress(2, 1_000);
		press(3, 1_200, 1);
		finishPress(3, 1_200);
		press(4, 1_400, 1, true);
		finishPress(4, 1_400);
		press(20, 1_500, 3);
		finishPress(20, 1_500);
		press(20, 2_100, 2);

		const starts = commands.flatMap((command) =>
			command.kind === 'selection-gesture' && command.payload.action === 'start'
				? [command.payload]
				: [],
		);
		expect(starts.map((payload) => payload.clickCount)).toEqual([1, 2, 3, 1, 1]);
		expect(starts[2]).toMatchObject({ selectOutput: true });
		expect(commands.some((command) => command.kind === 'selection-command')).toBe(false);
		terminal.dispose();
	});

	it('preserves rapid multi-click boundaries while gesture replies are in flight', async () => {
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['alpha beta', ''], null, [0, 1], 10n));
		const canvas = terminal.canvas as unknown as FakeElement;

		for (const timeStamp of [1_000, 1_150, 1_300]) {
			canvas.dispatch(
				'pointerdown',
				pointerEvent('pointerdown', 0, 1, 20, 2, { pointerId: 17, timeStamp }),
			);
			canvas.dispatch(
				'pointerup',
				pointerEvent('pointerup', 0, 0, 20, 2, { pointerId: 17, timeStamp: timeStamp + 10 }),
			);
		}
		for (let index = 0; index < 6; index++) {
			terminal.notifySelectionChanged({ attachmentId, state: 'none' });
			browser.flushFrames();
		}

		const gestures = commands.flatMap((command) =>
			command.kind === 'selection-gesture' ? [command.payload] : [],
		);
		expect(gestures.map((gesture) => gesture.action)).toEqual([
			'start',
			'end',
			'start',
			'end',
			'start',
			'end',
		]);
		expect(
			gestures.flatMap((gesture) => (gesture.action === 'start' ? [gesture.clickCount] : [])),
		).toEqual([1, 2, 3]);
		terminal.dispose();
	});

	it('keeps modifier-assisted semantic output selection in the pointer gesture', async () => {
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

		const activeSelection = {
			attachmentId,
			end: { column: 1, row: CanonicalUint64TextSchema.parse('11') },
			preview: 'semantic output',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('10') },
			state: 'active' as const,
		};
		for (const [index, timeStamp] of [1_000, 1_150].entries()) {
			canvas.dispatch(
				'pointerdown',
				pointerEvent('pointerdown', 0, 1, 2, 2, {
					detail: index + 1,
					pointerId: 17,
					timeStamp,
				}),
			);
			terminal.notifySelectionChanged({ attachmentId, state: 'none' });
			canvas.dispatch(
				'pointerup',
				pointerEvent('pointerup', 0, 0, 2, 2, { pointerId: 17, timeStamp: timeStamp + 10 }),
			);
			browser.flushFrames();
			terminal.notifySelectionChanged({ attachmentId, state: 'none' });
		}
		canvas.dispatch(
			'pointerdown',
			pointerEvent('pointerdown', 0, 1, 2, 2, {
				detail: 1,
				metaKey: true,
				pointerId: 17,
				timeStamp: 1_300,
			}),
		);

		expect(commands.at(-1)).toMatchObject({
			kind: 'selection-gesture',
			payload: { action: 'start', clickCount: 3, selectOutput: true },
		});
		expect(canvas.capturedPointers.has(17)).toBe(true);
		terminal.notifySelectionChanged(activeSelection);
		canvas.dispatch(
			'pointerup',
			pointerEvent('pointerup', 0, 0, 2, 2, { pointerId: 17, timeStamp: 1_310 }),
		);
		browser.flushFrames();
		terminal.notifySelectionChanged(activeSelection);
		expect(canvas.capturedPointers.has(17)).toBe(false);
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
		expect(read.payload).toMatchObject({
			origin: { kind: 'tail' },
			revisionPolicy: 'current-view',
		});
		expect(resolvedRowRequestStart(read, 12n)).toBe(9n);
		terminal.applyRows(
			renderRowWindow(
				BigInt(read.payload.requestSequence),
				1n,
				9n,
				12n,
				Array.from({ length: 3 }, (_, index) => `row-${index + 9}`),
			),
		);
		browser.flushFrames();

		const gestures = commands.filter((command) => command.kind === 'selection-gesture');
		expect(gestures.at(-1)?.payload).toMatchObject({
			action: 'update',
			point: { column: 0, row: 0 },
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

		(terminal.canvas as unknown as FakeElement).dispatch(
			'pointerdown',
			pointerEvent('pointerdown', 0, 1, 8, 18),
		);
		const preventDefault = vi.fn<() => void>();
		(terminal.canvas as unknown as FakeElement).dispatch('pointermove', {
			altKey: false,
			buttons: 1,
			clientX: 8,
			clientY: 18,
			ctrlKey: false,
			metaKey: false,
			pointerId: 1,
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

	it('rejects a terminal failure from a different creation identity', async () => {
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
		terminal.onError(errors);
		await terminal.open(browser.root);

		terminal.notifyFailed({
			...failureEvent('0'),
			identity: { creationId: otherCreationId, hostEpoch, terminalId },
		});

		expect(errors).toHaveBeenCalledOnce();
		expect(terminal.canControlTerminal).toBe(true);
		terminal.dispose();
	});

	it('labels visible URL, file, and hash hints without forwarding label keys', async () => {
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
				hintsActive: (count) => `${count} hints`,
				hintsLoading: 'Finding hints',
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
		terminal.applyFrame(
			terminalFrame(
				'snapshot',
				1n,
				1n,
				['visible output', ''],
				null,
				[0, 1],
				0n,
				interaction,
				{
					column: 0,
					row: 0,
				},
				1n,
				80,
			),
		);
		const visibleRows = [
			terminalHintRow(0n, 'https://orchbee.dev README.md src/main.ts:42:7 deadbee'),
			terminalHintRow(1n, ''),
		];
		vi.spyOn(terminal, 'readBufferRows').mockImplementation(async (request) => ({
			byteLength: visibleRows[0]?.text.length ?? 0,
			detail: request.detail,
			generation: request.generation,
			kind: 'rows',
			revision: request.revision,
			rows: visibleRows,
			startRow: request.startRow,
			totalRows: 2n,
		}));

		const choice = terminal.chooseVisibleHint(new AbortController().signal);
		await vi.waitFor(() => {
			const overlay = (browser.root as unknown as FakeElement).children.find(
				(element) => element.dataset['projectedTerminalHints'] === 'true',
			);
			expect(overlay?.dataset['state']).toBe('active');
		});
		const overlay = (browser.root as unknown as FakeElement).children.find(
			(element) => element.dataset['projectedTerminalHints'] === 'true',
		);
		if (overlay === undefined) throw new Error('terminal hint overlay is unavailable');
		expect(overlay.children.map((element) => element.dataset['projectedTerminalHintKind'])).toEqual(
			['url', 'path', 'file-line', 'hash'],
		);
		const hash = overlay.children.find(
			(element) => element.dataset['projectedTerminalHintKind'] === 'hash',
		);
		const label = hash?.dataset['projectedTerminalHintLabel'];
		if (label === undefined) throw new Error('hash hint label is unavailable');
		const preventDefault = vi.fn<() => void>();
		const stopPropagation = vi.fn<() => void>();
		(terminal.textarea as unknown as FakeElement).dispatch(
			'keydown',
			keyboardEvent('keydown', `Digit${label}`, label, label.codePointAt(0) ?? 0, {
				preventDefault,
				stopPropagation,
			}),
		);

		await expect(choice).resolves.toMatchObject({
			hint: { kind: 'hash', text: 'deadbee' },
			kind: 'selected',
		});
		expect(preventDefault).toHaveBeenCalledOnce();
		expect(stopPropagation).toHaveBeenCalledOnce();
		expect(commands.some((command) => command.kind === 'key-input')).toBe(false);
		expect(overlay.parentElement).toBeNull();

		const staleChoice = terminal.chooseVisibleHint(new AbortController().signal);
		await vi.waitFor(() =>
			expect(
				(browser.root as unknown as FakeElement).children.some(
					(element) => element.dataset['state'] === 'active',
				),
			).toBe(true),
		);
		terminal.applyFrame(
			terminalFrame(
				'snapshot',
				2n,
				2n,
				['changed output', ''],
				null,
				[0, 1],
				0n,
				interaction,
				{
					column: 0,
					row: 0,
				},
				1n,
				80,
			),
		);
		await expect(staleChoice).resolves.toEqual({ kind: 'stale' });
		terminal.dispose();
	});

	it('discovers links across the complete bounded logical line', async () => {
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
		const logicalText = ['https://', 'exa', 'mple', '.com', '/pa', 'th', '?x', 'tail'];
		const wraps = [true, true, true, true, true, true, false, false];
		const wrapContinuations = [false, true, true, true, true, true, true, false];
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['?x', 'tail'], null, [0, 1], 6n));
		terminal.scrollLines(-3);
		const historyRead = commands.at(-1);
		if (historyRead?.kind !== 'read-rows')
			throw new Error('history setup did not request Host rows');
		expect(historyRead.payload.revisionPolicy).toBe('current-view');
		const historyStartRow = Number(resolvedRowRequestStart(historyRead, 8n));
		expect(historyStartRow).toBe(3);
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(historyRead.payload.requestSequence),
					1n,
					BigInt(historyStartRow),
					8n,
					logicalText.slice(historyStartRow),
					{
						wrapContinuations: wrapContinuations.slice(historyStartRow),
						wraps: wraps.slice(historyStartRow),
					},
				),
			),
		).toBe('committed');
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
			expect(commands.filter((command) => command.kind === 'read-rows')).toHaveLength(1),
		);
		const firstRead = commands.at(-1);
		if (firstRead?.kind !== 'read-rows' || firstRead.payload.origin.kind !== 'absolute') {
			throw new Error('link lookup did not request absolute Host rows');
		}
		expect(firstRead.payload).toMatchObject({
			origin: { kind: 'absolute', row: '2' },
			rowCount: 3,
		});
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(firstRead.payload.requestSequence),
					1n,
					2n,
					8n,
					logicalText.slice(2, 5),
					{
						wrapContinuations: wrapContinuations.slice(2, 5),
						wraps: wraps.slice(2, 5),
					},
				),
			),
		).toBe('committed');

		await vi.waitFor(() =>
			expect(commands.filter((command) => command.kind === 'read-rows')).toHaveLength(2),
		);
		const secondRead = commands.at(-1);
		if (secondRead?.kind !== 'read-rows' || secondRead.payload.origin.kind !== 'absolute') {
			throw new Error('link lookup did not continue toward the logical-line start');
		}
		expect(secondRead.payload).toMatchObject({
			origin: { kind: 'absolute', row: '0' },
			rowCount: 2,
		});
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(secondRead.payload.requestSequence),
					1n,
					0n,
					8n,
					logicalText.slice(0, 2),
					{
						wrapContinuations: wrapContinuations.slice(0, 2),
						wraps: wraps.slice(0, 2),
					},
				),
			),
		).toBe('committed');

		await vi.waitFor(() =>
			expect(commands.filter((command) => command.kind === 'read-rows')).toHaveLength(3),
		);
		const thirdRead = commands.at(-1);
		if (thirdRead?.kind !== 'read-rows' || thirdRead.payload.origin.kind !== 'absolute') {
			throw new Error('link lookup did not continue toward the logical-line end');
		}
		expect(thirdRead.payload).toMatchObject({
			origin: { kind: 'absolute', row: '5' },
			rowCount: 3,
		});
		expect(
			terminal.applyRows(
				renderRowWindow(
					BigInt(thirdRead.payload.requestSequence),
					1n,
					5n,
					8n,
					logicalText.slice(5),
					{
						wrapContinuations: wrapContinuations.slice(5),
						wraps: wraps.slice(5),
					},
				),
			),
		).toBe('committed');
		await vi.waitFor(() => expect(canvas.style['cursor']).toBe('pointer'));
		terminal.dispose();
	});

	it.each([
		{
			changeGeometry: (browser: FakeBrowser) => browser.dispatchWindow('resize'),
			name: 'window resize',
		},
		{
			changeGeometry: (browser: FakeBrowser) => {
				browser.setDevicePixelRatio(1.5);
				browser.mediaQueries.at(-1)?.dispatchChange();
			},
			name: 'device-pixel-ratio change',
		},
	])('invalidates hovered links before activation after $name', async ({ changeGeometry }) => {
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
					hyperlink: 'https://example.com/stale-geometry',
				}),
			),
		).toBe('committed');
		await vi.waitFor(() => expect(canvas.style['cursor']).toBe('pointer'));

		changeGeometry(browser);

		expect(canvas.style['cursor']).toBe('text');
		const preventDefault = vi.fn<() => void>();
		canvas.dispatch('click', {
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			preventDefault,
			shiftKey: false,
			type: 'click',
		} as unknown as MouseEvent);
		expect(linkRequests).not.toHaveBeenCalled();
		expect(preventDefault).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('gates modifier-assisted OSC 8 activation at click time during mouse tracking', async () => {
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
			terminalFrame('snapshot', 1n, 1n, ['linked', ''], null, [0, 1], 0n, {
				...interaction,
				mouseAny: true,
			}),
		);
		const canvas = terminal.canvas as unknown as FakeElement;

		canvas.dispatch('pointermove', {
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			shiftKey: true,
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
			shiftKey: false,
			type: 'click',
		} as unknown as MouseEvent);

		expect(linkRequests).not.toHaveBeenCalled();
		expect(preventDefault).not.toHaveBeenCalled();
		expect(canvas.style['cursor']).toBe('default');
		canvas.dispatch('pointermove', {
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			shiftKey: true,
			type: 'pointermove',
		} as unknown as PointerEvent);
		await vi.waitFor(() => expect(canvas.style['cursor']).toBe('pointer'));
		canvas.dispatch('click', {
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			preventDefault,
			shiftKey: true,
			type: 'click',
		} as unknown as MouseEvent);

		expect(linkRequests).toHaveBeenCalledWith({
			context: {
				column: 0,
				generation: '1',
				linkEpoch: '1',
				revision: '1',
				row: '0',
			},
			target: { kind: 'url', url: 'https://example.com/from-osc-8' },
		});
		expect(preventDefault).toHaveBeenCalledOnce();
		expect(terminal.captureActionContext({ clientX: 2, clientY: 2, shiftKey: false })).toEqual({
			kind: 'terminal-owned',
		});
		const capturedContext = terminal.captureActionContext({
			clientX: 2,
			clientY: 2,
			shiftKey: true,
		});
		expect(capturedContext).toEqual({
			kind: 'product',
			link: {
				context: {
					column: 0,
					generation: '1',
					linkEpoch: '1',
					revision: '1',
					row: '0',
				},
				target: { kind: 'url', url: 'https://example.com/from-osc-8' },
			},
		});
		if (capturedContext.kind !== 'product' || capturedContext.link === null) {
			throw new Error('Expected a revision-pinned terminal link context.');
		}
		expect(terminal.linkOpenRequestIsCurrent(capturedContext.link)).toBe(true);
		expect(
			terminal.linkOpenRequestIsCurrent({
				...capturedContext.link,
				context: { ...capturedContext.link.context, revision: '2' },
			}),
		).toBe(false);

		canvas.dispatch('pointerdown', {
			...pointerEvent('pointerdown', 0, 1, 2, 2),
			ctrlKey: true,
			pointerId: 4,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('default');
		canvas.dispatch('pointerup', {
			...pointerEvent('pointerup', 0, 0, 2, 2),
			ctrlKey: true,
			pointerId: 4,
			shiftKey: true,
		} as PointerEvent);
		expect(canvas.style['cursor']).toBe('text');
		canvas.dispatch('pointermove', {
			buttons: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			shiftKey: true,
			type: 'pointermove',
		} as unknown as PointerEvent);
		expect(canvas.style['cursor']).toBe('pointer');
		const suppressedPreventDefault = vi.fn<() => void>();
		canvas.dispatch('click', {
			button: 0,
			clientX: 2,
			clientY: 2,
			ctrlKey: true,
			metaKey: false,
			preventDefault: suppressedPreventDefault,
			shiftKey: true,
			type: 'click',
		} as unknown as MouseEvent);

		expect(linkRequests).toHaveBeenCalledOnce();
		expect(suppressedPreventDefault).not.toHaveBeenCalled();
		expect(canvas.style['cursor']).toBe('text');
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
		const copyResults = vi.fn<Parameters<ProjectedTerminal['onSelectionCopyResult']>[0]>();
		terminal.onSelectionCopyResult(copyResults);

		const copy = terminal.copySelectionAsync();
		const copyCommand = commands.at(-1);
		if (copyCommand?.kind !== 'selection-copy') throw new Error('copy command missing');
		terminal.notifySelectionCopied(8, copyCommand.payload.requestSequence);
		await expect(copy).resolves.toEqual({ byteLength: 8, kind: 'completed' });
		expect(copyResults).toHaveBeenCalledWith({
			requestSequence: copyCommand.payload.requestSequence,
			result: { byteLength: 8, kind: 'completed' },
			source: 'manual',
		});

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

	it('updates copy-on-select and reports one result for automatic and manual failures', async () => {
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
		const copyResults = vi.fn<Parameters<ProjectedTerminal['onSelectionCopyResult']>[0]>();
		terminal.onSelectionCopyResult(copyResults);
		await terminal.open(browser.root);
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']));
		const canvas = terminal.canvas as unknown as FakeElement;
		const activeSelection = {
			attachmentId,
			end: { column: 1, row: canonicalText('0') },
			preview: 'ab',
			rectangle: false,
			start: { column: 0, row: canonicalText('0') },
			state: 'active' as const,
		};
		const completeGesture = (pointerId: number): void => {
			canvas.dispatch('pointerdown', pointerEvent('pointerdown', 0, 1, 2, 2, { pointerId }));
			terminal.notifySelectionChanged({ attachmentId, state: 'none' });
			canvas.dispatch('pointerup', pointerEvent('pointerup', 0, 0, 12, 2, { pointerId }));
			browser.flushFrames();
			terminal.notifySelectionChanged(activeSelection);
		};

		expect(terminal.copyOnSelect).toBe(true);
		terminal.setCopyOnSelect(false);
		expect(terminal.copyOnSelect).toBe(false);
		completeGesture(81);
		expect(commands.filter((command) => command.kind === 'selection-copy')).toHaveLength(0);

		terminal.setCopyOnSelect(true);
		expect(terminal.copyOnSelect).toBe(true);
		completeGesture(82);
		const automaticCopy = commands.at(-1);
		if (automaticCopy?.kind !== 'selection-copy') throw new Error('automatic copy command missing');
		terminal.notifyBrokerFailure({
			code: 'automatic-copy-failed',
			message: 'automatic copy failed',
			operation: 'copy',
			requestSequence: automaticCopy.payload.requestSequence,
			retryable: false,
		});
		terminal.notifyBrokerFailure({
			code: 'automatic-copy-failed',
			message: 'duplicate automatic copy failure',
			operation: 'copy',
			requestSequence: automaticCopy.payload.requestSequence,
			retryable: false,
		});

		const manualResult = terminal.copySelectionAsync();
		const manualCopy = commands.at(-1);
		if (manualCopy?.kind !== 'selection-copy') throw new Error('manual copy command missing');
		terminal.notifyBrokerFailure({
			code: 'manual-copy-failed',
			message: 'manual copy failed',
			operation: 'copy',
			requestSequence: manualCopy.payload.requestSequence,
			retryable: true,
		});

		await expect(manualResult).resolves.toEqual({
			code: 'manual-copy-failed',
			kind: 'failed',
			retryable: true,
		});
		expect(copyResults.mock.calls.map(([event]) => event)).toEqual([
			{
				requestSequence: automaticCopy.payload.requestSequence,
				result: { code: 'automatic-copy-failed', kind: 'failed', retryable: false },
				source: 'automatic',
			},
			{
				requestSequence: manualCopy.payload.requestSequence,
				result: { code: 'manual-copy-failed', kind: 'failed', retryable: true },
				source: 'manual',
			},
		]);
		terminal.dispose();
	});

	it('remeasures a silently resized mounted surface before programmatic focus', async () => {
		const browser = createFakeBrowser();
		const root = browser.root as unknown as FakeElement;
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
		const initialResize = commands.find((command) => command.kind === 'resize-terminal');
		if (initialResize?.kind !== 'resize-terminal') {
			throw new Error('initial resize proposal is unavailable');
		}

		root.clientWidth = 360;
		terminal.focus();

		const resizeCommands = commands.filter((command) => command.kind === 'resize-terminal');
		expect(resizeCommands).toHaveLength(2);
		const focusedResize = resizeCommands.at(-1);
		if (focusedResize?.kind !== 'resize-terminal') {
			throw new Error('focused resize proposal is unavailable');
		}
		expect(focusedResize.payload.columns).toBeLessThan(initialResize.payload.columns);
		expect(browser.document.activeElement).toBe(terminal.textarea);
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

		const errors = vi.fn<(event: ProjectedTerminalErrorEvent) => void>();
		terminal.onError(errors);
		const failure = {
			code: 'resize-rejected',
			correlationId: '55'.repeat(16) as ProjectedBrokerFailure['correlationId'],
			message: 'resize rejected',
			operation: 'resize' as const,
			retryable: true,
		};
		terminal.notifyBrokerFailure(failure);
		expect(errors).toHaveBeenCalledWith({ ...failure, error: expect.any(Error) });
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
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();
		telemetry.mockClear();

		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');
		expect(setSuspended).toHaveBeenLastCalledWith(true);
		expect(browser.frames.size).toBe(0);
		terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['c', 'd'], 1n));
		browser.setDevicePixelRatio(1.5);
		browser.mediaQueries.at(-1)?.dispatchChange();
		expect(browser.frames.size).toBe(0);
		expect(telemetry).not.toHaveBeenCalled();

		browser.setVisibilityState('visible');
		browser.dispatchDocument('visibilitychange');
		expect(setSuspended).toHaveBeenLastCalledWith(false);
		expect(browser.frames.size).toBe(1);
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				cellsVisited: 4,
				forceAll: true,
				reasons: expect.arrayContaining(['surface']),
				revision: '2',
				rowsRendered: 2,
			}),
		);
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();
		terminal.dispose();
	});

	it('coordinates document visibility and projection demand before drawing the newest acknowledged frame', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const setSuspended = vi.spyOn(WebGPURenderer.prototype, 'setSuspended');
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
		const telemetry = vi.fn<(event: ProjectedTerminalFrameTelemetry) => void>();
		terminal.onFrameTelemetry(telemetry);
		await terminal.open(browser.root);

		expect(browser.documentListenerCount('visibilitychange')).toBe(1);
		expect(terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']))).toBe('committed');
		expect(browser.frames.size).toBe(1);
		browser.flushFrames();
		expect(telemetry).toHaveBeenLastCalledWith(
			expect.objectContaining({ generation: '1', revision: '1' }),
		);
		telemetry.mockClear();

		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');
		expect(setSuspended).toHaveBeenLastCalledWith(true);
		expect(browser.frames.size).toBe(0);
		expect(terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['c', 'd'], 1n))).toBe('committed');
		expect(terminal.metadata).toEqual(expect.objectContaining({ generation: 1n, revision: 2n }));
		expect(browser.frames.size).toBe(0);

		const suspensionCallsBeforePause = setSuspended.mock.calls.length;
		terminal.setProjectionDemand('paused');
		expect(setSuspended).toHaveBeenCalledTimes(suspensionCallsBeforePause);
		expect(browser.frames.size).toBe(0);
		expect(terminal.applyFrame(terminalFrame('delta', 3n, 3n, ['e', 'f'], 2n))).toBe('committed');
		expect(
			terminal.applyFrame(
				terminalFrame(
					'snapshot',
					4n,
					4n,
					['g', 'h'],
					null,
					[0, 1],
					0n,
					interaction,
					{ column: 0, row: 0 },
					2n,
				),
			),
		).toBe('committed');
		expect(terminal.metadata).toEqual(expect.objectContaining({ generation: 2n, revision: 4n }));
		expect(browser.frames.size).toBe(0);
		expect(telemetry).not.toHaveBeenCalled();
		expect(commands.filter((command) => command.kind === 'frame-applied')).toEqual([
			{
				kind: 'frame-applied',
				payload: { attachmentId, generation: '1', revision: '1', sequence: '1' },
			},
			{
				kind: 'frame-applied',
				payload: { attachmentId, generation: '1', revision: '2', sequence: '2' },
			},
			{
				kind: 'frame-applied',
				payload: { attachmentId, generation: '1', revision: '3', sequence: '3' },
			},
			{
				kind: 'frame-applied',
				payload: { attachmentId, generation: '2', revision: '4', sequence: '4' },
			},
		]);

		browser.setVisibilityState('visible');
		browser.dispatchDocument('visibilitychange');
		expect(setSuspended).toHaveBeenCalledTimes(suspensionCallsBeforePause);
		expect(browser.frames.size).toBe(0);
		browser.flushFrames();
		expect(telemetry).not.toHaveBeenCalled();

		terminal.setProjectionDemand('active');
		expect(setSuspended).toHaveBeenLastCalledWith(false);
		expect(browser.frames.size).toBe(1);
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();
		expect(telemetry).toHaveBeenLastCalledWith(
			expect.objectContaining({ generation: '2', revision: '4' }),
		);
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();

		terminal.dispose();
		expect(browser.documentListenerCount('visibilitychange')).toBe(0);
		const suspensionCallsAfterDispose = setSuspended.mock.calls.length;
		const commandCountAfterDispose = commands.length;
		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');
		terminal.setProjectionDemand('active');
		expect(terminal.applyFrame(terminalFrame('snapshot', 5n, 5n, ['i', 'j']))).toBe('disposed');
		expect(setSuspended).toHaveBeenCalledTimes(suspensionCallsAfterDispose);
		expect(commands).toHaveLength(commandCountAfterDispose);
		expect(browser.frames.size).toBe(0);
	});

	it('draws one current restoration frame while explicit presentation demand remains paused', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let hiddenObserved = false;
		const setDocumentHidden = vi
			.spyOn(WebGPURenderer.prototype, 'setDocumentHidden')
			.mockImplementation(async (hidden) => {
				if (hidden) {
					hiddenObserved = true;
					return false;
				}
				return hiddenObserved;
			});
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
		terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['initial', '']));
		browser.flushFrames();
		telemetry.mockClear();

		browser.setVisibilityState('hidden');
		browser.dispatchDocument('visibilitychange');
		terminal.setProjectionDemand('paused');
		terminal.applyFrame(terminalFrame('delta', 2n, 2n, ['current', ''], 1n));
		expect(browser.frames.size).toBe(0);

		browser.setVisibilityState('visible');
		browser.dispatchDocument('visibilitychange');
		await Promise.resolve();
		expect(setDocumentHidden).toHaveBeenLastCalledWith(false);
		expect(browser.frames.size).toBe(1);
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({ forceAll: true, revision: '2' }),
		);

		terminal.applyFrame(terminalFrame('delta', 3n, 3n, ['later', ''], 2n));
		expect(browser.frames.size).toBe(0);
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

	it('measures an observer surface and proposes its current geometry only after lease promotion', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		let notifyResize: (() => void) | undefined;
		const observe = vi.fn<(target: Element) => void>();
		class FakeResizeObserver {
			constructor(callback: ResizeObserverCallback) {
				notifyResize = () => callback([], this as unknown as ResizeObserver);
			}

			disconnect(): void {}

			observe(target: Element): void {
				observe(target);
			}

			unobserve(): void {}
		}
		vi.stubGlobal('ResizeObserver', FakeResizeObserver);
		const syncSurfacePadding = vi.spyOn(WebGPURenderer.prototype, 'syncSurfacePadding');
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
		expect(observe).toHaveBeenCalledWith(browser.root);
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toEqual([]);

		syncSurfacePadding.mockClear();
		notifyResize?.();
		expect(syncSurfacePadding).toHaveBeenCalledOnce();
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toEqual([]);

		terminal.setResizeOwnership(true);
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(1);
		terminal.dispose();
	});

	it('forces one current geometry proposal on promotion even when projection metadata is stale', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		vi.stubGlobal(
			'ResizeObserver',
			class {
				disconnect(): void {}

				observe(): void {}

				unobserve(): void {}
			},
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
			terminalId,
		});

		await terminal.open(browser.root);
		terminal.setResizeOwnership(true);
		const measured = commands.find(
			(
				command,
			): command is Extract<ProjectedTerminalCommand, { readonly kind: 'resize-terminal' }> =>
				command.kind === 'resize-terminal',
		);
		if (measured === undefined) throw new Error('promoted geometry is missing');
		terminal.setResizeOwnership(false);
		const rowIndices = Array.from({ length: measured.payload.rows }, (_, index) => index);
		expect(
			terminal.applyFrame(
				terminalFrame(
					'snapshot',
					1n,
					1n,
					['a', 'b'],
					null,
					rowIndices,
					0n,
					interaction,
					{ column: 0, row: 0 },
					1n,
					measured.payload.columns,
					ActiveScreen.primary,
					'unknown',
					0n,
					{
						cellPixelHeight: measured.payload.cellPixelHeight,
						cellPixelWidth: measured.payload.cellPixelWidth,
						rows: measured.payload.rows,
					},
				),
			),
		).toBe('committed');
		commands.length = 0;

		terminal.setResizeOwnership(true);
		expect(commands).toEqual([measured]);
		terminal.dispose();
	});

	it('repaints a committed frame and updates PTY and input geometry for fractional DPR changes', async () => {
		const browser = createFakeBrowser();
		vi.stubGlobal('document', browser.document);
		vi.stubGlobal('window', browser.document.defaultView);
		vi.stubGlobal('navigator', {});
		const commands: ProjectedTerminalCommand[] = [];
		const setDevicePixelRatio = vi.spyOn(WebGPURenderer.prototype, 'setDevicePixelRatio');
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
		expect(terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, ['a', 'b']))).toBe('committed');
		browser.flushFrames();
		telemetry.mockClear();
		const initialResize = commands.reduceRight<
			Extract<ProjectedTerminalCommand, { readonly kind: 'resize-terminal' }> | undefined
		>(
			(found, command) => found ?? (command.kind === 'resize-terminal' ? command : undefined),
			undefined,
		);
		if (initialResize?.kind !== 'resize-terminal') throw new Error('initial resize is missing');
		const initialQuery = browser.mediaQueries.at(-1);
		expect(initialQuery?.media).toBe('(resolution: 1dppx)');
		expect(initialQuery?.listenerCount).toBe(1);
		const input = terminal.textarea as unknown as FakeElement | undefined;
		if (input === undefined) throw new Error('terminal input is missing');
		const initialInputGeometry = {
			height: input.style['height'],
			left: input.style['left'],
			lineHeight: input.style['lineHeight'],
			top: input.style['top'],
			width: input.style['width'],
		};
		const initialCanvas = terminal.canvas;
		if (initialCanvas === undefined) throw new Error('terminal canvas is missing');
		const initialCanvasGeometry = {
			height: initialCanvas.height,
			styleHeight: initialCanvas.style.height,
			styleWidth: initialCanvas.style.width,
			width: initialCanvas.width,
		};

		browser.setDevicePixelRatio(1.5);
		initialQuery?.dispatchChange();

		const resizeCommands = commands.filter((command) => command.kind === 'resize-terminal');
		expect(resizeCommands).toHaveLength(2);
		const changedResize = resizeCommands.at(-1);
		if (changedResize?.kind !== 'resize-terminal') throw new Error('DPR resize is missing');
		expect(changedResize.payload).toEqual({
			...initialResize.payload,
			cellPixelHeight: Math.round((initialResize.payload.cellPixelHeight * 3) / 2),
			cellPixelWidth: Math.round((initialResize.payload.cellPixelWidth * 3) / 2),
		});
		expect(changedResize.payload.cellPixelHeight * changedResize.payload.rows).toBe(
			Math.round((initialResize.payload.cellPixelHeight * 3) / 2) * initialResize.payload.rows,
		);
		expect(changedResize.payload.cellPixelWidth * changedResize.payload.columns).toBe(
			Math.round((initialResize.payload.cellPixelWidth * 3) / 2) * initialResize.payload.columns,
		);
		expect(terminal.metadata).toMatchObject({ columns: 2, rows: 2 });
		expect(terminal.canvas).toBe(initialCanvas);
		expect(initialCanvas.style.height).toBe(initialCanvasGeometry.styleHeight);
		expect(initialCanvas.style.width).toBe(initialCanvasGeometry.styleWidth);
		expect(initialCanvas.height).toBe(Math.round(initialCanvasGeometry.height * 1.5));
		expect(initialCanvas.width).toBe(Math.round(initialCanvasGeometry.width * 1.5));
		expect({
			height: input.style['height'],
			left: input.style['left'],
			lineHeight: input.style['lineHeight'],
			top: input.style['top'],
			width: input.style['width'],
		}).toEqual(initialInputGeometry);
		expect(initialQuery?.listenerCount).toBe(0);
		const fractionalQuery = browser.mediaQueries.at(-1);
		expect(fractionalQuery?.media).toBe('(resolution: 1.5dppx)');
		expect(fractionalQuery?.listenerCount).toBe(1);
		expect(browser.frames.size).toBe(1);
		browser.flushFrames();
		expect(telemetry).toHaveBeenCalledOnce();
		expect(telemetry).toHaveBeenCalledWith(
			expect.objectContaining({
				backend: 'canvas2d',
				cellsVisited: 4,
				forceAll: true,
				reasons: ['surface'],
				revision: '1',
				rowsRendered: 2,
			}),
		);
		terminal.dispose();
		expect(fractionalQuery?.listenerCount).toBe(0);

		setDevicePixelRatio.mockClear();
		browser.setDevicePixelRatio(3);
		fractionalQuery?.dispatchChange();
		browser.dispatchWindow('resize');
		expect(setDevicePixelRatio).not.toHaveBeenCalled();
		expect(commands.filter((command) => command.kind === 'resize-terminal')).toHaveLength(2);
	});
});
