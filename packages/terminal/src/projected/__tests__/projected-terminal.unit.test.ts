import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	encodeTerminalFrame,
	parseAttachmentId,
	parseCreationId,
	parseHostEpoch,
	parseTerminalId,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	UnderlineKind,
	type TerminalFrame,
	type TerminalExited,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ProjectedTerminal, type ProjectedTerminalCommand } from '../projected-terminal';

interface FakeElement {
	readonly attributes: Map<string, string>;
	readonly children: FakeElement[];
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
	remove(): void;
}

interface FakeBrowser {
	readonly document: Document;
	readonly frames: Map<number, FrameRequestCallback>;
	readonly root: HTMLElement;
	flushFrames(): void;
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
		scrollbackRows: 0n,
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
	const context = createContext();
	const createElement = (tagName: string): FakeElement => {
		const listeners = new Map<string, Set<EventListener>>();
		const element: FakeElement & Record<string, unknown> = {
			attributes: new Map(),
			children: [],
			clientHeight: 360,
			clientWidth: 720,
			height: 0,
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
			removeEventListener: (type: string, listener: EventListener) => {
				listeners.get(type)?.delete(listener);
			},
			setAttribute: (name: string, value: string) => element.attributes.set(name, value),
		};
		Object.defineProperty(element, 'ownerDocument', { get: () => document });
		return element;
	};

	const defaultView = {
		cancelAnimationFrame: (handle: number) => frames.delete(handle),
		devicePixelRatio: 1,
		getComputedStyle: () => ({
			paddingBottom: '0px',
			paddingLeft: '0px',
			paddingRight: '0px',
			paddingTop: '0px',
		}),
		requestAnimationFrame: (callback: FrameRequestCallback) => {
			const handle = nextFrame++;
			frames.set(handle, callback);
			return handle;
		},
	};
	const document = {
		createElement,
		defaultView,
	} as unknown as Document;
	const root = createElement('div') as unknown as HTMLElement;
	return {
		document,
		frames,
		root,
		flushFrames: () => {
			const callbacks = [...frames.values()];
			frames.clear();
			for (const callback of callbacks) callback(16);
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
			labels: { input: 'input', screen: 'screen' },
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
});
