import { beforeAll, describe, expect, it, vi } from 'vitest';
import { createTerminal, initializeWasm } from '../../__tests__/test-helpers';
import type { ContextMenuRequest, TerminalClipboardHost, TerminalHost } from '../../host';
import { AnsiMode, DecMode } from '../../types';
import { Terminal } from '../terminal';

beforeAll(initializeWasm);

function createShiftKeyEvent(code: string): KeyboardEvent {
	return { code, shiftKey: true, ctrlKey: false, altKey: false, metaKey: false } as KeyboardEvent;
}

function createLines(prefix: string, count: number): string {
	return Array.from({ length: count }, (_, index) => `${prefix}-${index}\r\n`).join('');
}

type AbsoluteSelection = {
	start: { x: number; y: number };
	end: { x: number; y: number };
};

function selectionInBuffer(selection: AbsoluteSelection | null): AbsoluteSelection | null {
	return selection;
}

describe('Terminal lifecycle', () => {
	it('creates default and custom dimensions', () => {
		const defaultTerminal = createTerminal();
		const customTerminal = createTerminal(120, 40);
		expect([defaultTerminal.cols, defaultTerminal.rows]).toEqual([80, 24]);
		expect([customTerminal.cols, customTerminal.rows]).toEqual([120, 40]);
		defaultTerminal.dispose();
		customTerminal.dispose();
	});

	it('returns empty accessibility ranges before the DOM surface opens', () => {
		const terminal = createTerminal();
		expect(terminal.getAccessibleText()).toBe('');
		expect(terminal.getAccessibleVisibleRange()).toEqual({ start: 0, end: 0 });
		expect(terminal.getAccessibleSelection()).toBeNull();
		expect(terminal.selectAccessibleRange({ start: 0, end: 1 })).toBe(false);
		terminal.dispose();
	});

	it('supports public terminal options', () => {
		const terminal = new Terminal({
			cols: 12,
			rows: 3,
			scrollback: 7,
			cursorBlink: true,
			fontWeight: 225,
			fontWeightBold: 575,
			fontLigatures: false,
			fontMetricModifiers: { cellHeight: '10%', iconHeight: -1 },
			paddingBalance: 'equal',
			theme: { foreground: '#123456', background: '#010203' },
		});
		terminal.grid.update();
		expect(terminal.options.scrollback).toBe(7);
		expect(terminal.options.fontWeight).toBe(225);
		expect(terminal.options.fontWeightBold).toBe(575);
		expect(terminal.options.fontLigatures).toBe(false);
		expect(terminal.options.fontMetricModifiers).toEqual({ cellHeight: '10%', iconHeight: -1 });
		expect(terminal.options.paddingBalance).toBe('equal');
		expect(terminal.foregroundColor).toEqual({ r: 0x12, g: 0x34, b: 0x56 });
		expect(terminal.backgroundColor).toEqual({ r: 1, g: 2, b: 3 });
		terminal.dispose();
	});

	it('repairs active input state before entering read-only mode', () => {
		const terminal = createTerminal();
		const stateAtRepair: boolean[] = [];
		const recordState = (): void => {
			stateAtRepair.push(terminal.options.disableStdin);
		};
		Reflect.set(terminal, 'inputHandler', {
			releasePressedKeys: recordState,
			releasePressedMouseButtons: recordState,
			cancelComposition: recordState,
		});

		terminal.options.disableStdin = true;

		expect(stateAtRepair).toEqual([false, false, false]);
		Reflect.set(terminal, 'inputHandler', undefined);
		terminal.dispose();
	});

	it('updates the surface cursor immediately when output changes mouse ownership', () => {
		const terminal = createTerminal();
		const canvas = { style: { cursor: 'text' } };
		Reflect.set(terminal, 'canvas', canvas);

		terminal.write('\x1b[?1000h');
		expect(canvas.style.cursor).toBe('default');
		terminal.write('\x1b[?1000l');
		expect(canvas.style.cursor).toBe('text');
		terminal.write('\x1b[?1000h');
		terminal.reset();
		expect(canvas.style.cursor).toBe('text');

		Reflect.set(terminal, 'canvas', undefined);
		terminal.dispose();
	});

	it('discards an unfinished smooth-scroll target when the page becomes hidden', () => {
		const terminal = createTerminal();
		Reflect.set(terminal, 'viewportOffset', 2.5);
		Reflect.set(terminal, 'targetViewportY', 12);
		vi.stubGlobal('document', { visibilityState: 'hidden' });
		const handleVisibilityChange = Reflect.get(terminal, 'handleVisibilityChange') as () => void;

		try {
			handleVisibilityChange.call(terminal);
			expect(Reflect.get(terminal, 'targetViewportY')).toBe(2.5);
		} finally {
			vi.unstubAllGlobals();
			terminal.dispose();
		}
	});

	it('clears and reports a selection when its scrollback endpoint is pruned', () => {
		const terminal = new Terminal({ cols: 80, rows: 24, scrollback: 10_000 });
		const selection = Reflect.get(terminal, 'selectionController') as {
			setRange(start: { x: number; y: number }, end: { x: number; y: number }): boolean;
			getSelection(): string;
		};
		terminal.write(createLines('before', 5_000));
		expect(selection.setRange({ x: 0, y: 0 }, { x: 5, y: 0 })).toBe(true);
		expect(selection.getSelection()).not.toBe('');
		const changes: number[] = [];
		const subscription = terminal.onSelectionChange(() => changes.push(changes.length));

		terminal.write(createLines('after', 5_000));

		expect(selection.getSelection()).toBe('');
		expect(changes).toHaveLength(1);
		subscription.dispose();
		terminal.dispose();
	});

	it('retains history beyond Ghostty minimum page allocation by default', () => {
		const terminal = new Terminal({ cols: 80, rows: 24 });
		for (let index = 0; index < 1500; index++) terminal.writeln(`history-${index}`);

		expect(terminal.options.scrollback).toBe(10_000_000);
		expect(terminal.grid.getScrollbackLength()).toBe(1477);
		terminal.dispose();
	});

	it('clamps the viewport when terminal output clears scrollback', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 10_000_000 });
		for (let index = 0; index < 8; index++) terminal.writeln(`history-${index}`);
		terminal.scrollToTop();
		expect(terminal.viewportY).toBeGreaterThan(0);

		terminal.write('\x1b[3J');

		expect(terminal.grid.getScrollbackLength()).toBe(0);
		expect(terminal.viewportY).toBe(0);
		expect(Reflect.get(terminal, 'targetViewportY')).toBe(0);
		terminal.dispose();
	});

	it('loads the configured font before measuring the terminal surface', async () => {
		const terminal = new Terminal({ fontFamily: 'Test Mono, monospace', fontSize: 17 });
		const load = vi.fn<(font: string, text?: string) => Promise<FontFace[]>>(async () => []);
		vi.stubGlobal('document', { fonts: { load } });
		try {
			const loadConfiguredFont = Reflect.get(terminal, 'loadConfiguredFont') as () => Promise<void>;
			await loadConfiguredFont.call(terminal);

			expect(load).toHaveBeenCalledWith('normal 17px Test Mono, monospace', 'MW你🚀');
		} finally {
			vi.unstubAllGlobals();
			terminal.dispose();
		}
	});

	it('reports font loading failures without rejecting terminal initialization', async () => {
		const terminal = new Terminal({ fontFamily: 'Missing Font', fontSize: 14 });
		const failure = new Error('Font loading failed');
		const errors: unknown[] = [];
		const disposable = terminal.onHostError((event) => errors.push(event));
		const load = vi.fn<(font: string, text?: string) => Promise<FontFace[]>>(async () => {
			throw failure;
		});
		vi.stubGlobal('document', { fonts: { load } });
		try {
			const loadConfiguredFont = Reflect.get(terminal, 'loadConfiguredFont') as () => Promise<void>;
			await expect(loadConfiguredFont.call(terminal)).resolves.toBeUndefined();

			expect(errors).toEqual([{ operation: 'font-load', error: failure }]);
		} finally {
			vi.unstubAllGlobals();
			disposable.dispose();
			terminal.dispose();
		}
	});

	it('exposes a capability snapshot without claiming unimplemented graphics', () => {
		const terminal = new Terminal({
			host: {
				capabilities: { clipboardRead: false },
				clipboard: { read: async () => null, write: async () => {} },
				showContextMenu: async () => null,
			},
		});
		expect(terminal.capabilities).toEqual({
			renderer: 'headless',
			mouseFormats: ['x10', 'utf8', 'sgr', 'urxvt', 'sgr-pixel'],
			graphics: [],
			clipboardRead: false,
			clipboardWrite: true,
			contextMenu: true,
			notifications: false,
			openTarget: false,
			pasteConfirmation: false,
			fileDrop: false,
			accessibility: true,
			semanticPrompts: true,
		});
		expect(terminal.ghosttyBuildCapabilities).toEqual({
			simd: false,
			kittyGraphics: false,
			tmuxControlMode: false,
		});
		terminal.dispose();
	});

	it('rejects opening a terminal after disposal', async () => {
		const terminal = createTerminal();
		terminal.dispose();
		await expect(terminal.open({} as HTMLElement)).rejects.toThrow('disposed terminal');
	});

	it('releases disposed surface component references', () => {
		const terminal = createTerminal();
		const dispose = vi.fn<() => void>();
		Reflect.set(terminal, 'inputHandler', { dispose });
		Reflect.set(terminal, 'selectionManager', { dispose });
		Reflect.set(terminal, 'linkDetector', { dispose });
		Reflect.set(terminal, 'renderer', { dispose });

		terminal.dispose();

		expect(dispose).toHaveBeenCalledTimes(4);
		expect(Reflect.get(terminal, 'inputHandler')).toBeUndefined();
		expect(Reflect.get(terminal, 'selectionManager')).toBeUndefined();
		expect(Reflect.get(terminal, 'linkDetector')).toBeUndefined();
		expect(terminal.renderer).toBeUndefined();
	});

	it('rejects a second open while the first open is pending', async () => {
		const terminal = createTerminal();
		let finishOpen: (() => void) | undefined;
		const pending = new Promise<void>((resolve) => {
			finishOpen = resolve;
		});
		Reflect.set(terminal, 'openInternal', () => pending);
		const first = terminal.open({} as HTMLElement);

		await expect(terminal.open({} as HTMLElement)).rejects.toThrow('already open');
		finishOpen?.();
		await expect(first).resolves.toBeUndefined();
		terminal.dispose();
	});

	it('applies open dimensions as one resize event', async () => {
		const terminal = createTerminal();
		const failure = new Error('stop after options');
		const resizes: { cols: number; rows: number }[] = [];
		terminal.onResize((event) => resizes.push(event));
		Reflect.set(terminal, 'loadConfiguredFont', async () => {
			throw failure;
		});
		vi.stubGlobal('document', { removeEventListener: () => {} });

		try {
			await expect(terminal.open({} as HTMLElement, { cols: 31, rows: 7 })).rejects.toBe(failure);
			expect([terminal.cols, terminal.rows]).toEqual([31, 7]);
			expect(resizes).toEqual([{ cols: 31, rows: 7 }]);
		} finally {
			vi.unstubAllGlobals();
			terminal.dispose();
		}
	});

	it('rolls back parent state when opening fails before renderer initialization', async () => {
		const terminal = createTerminal();
		const parent = {
			style: { position: '' },
			addEventListener: () => {},
			removeEventListener: () => {},
		} as unknown as HTMLElement;
		vi.stubGlobal('window', { getComputedStyle: () => ({ position: 'static' }) });
		vi.stubGlobal('document', {
			createElement: () => {
				throw new Error('DOM allocation failed');
			},
			removeEventListener: () => {},
		});
		try {
			await expect(terminal.open(parent)).rejects.toThrow('DOM allocation failed');
			expect(terminal.element).toBeUndefined();
			expect(parent.style.position).toBe('');
			await expect(terminal.open(parent)).rejects.toThrow('DOM allocation failed');
		} finally {
			vi.unstubAllGlobals();
			terminal.dispose();
		}
	});

	it('rebuilds the renderer and rearms its media query when DPR changes', () => {
		const terminal = createTerminal();
		const setDevicePixelRatio = vi.fn<(ratio: number) => boolean>(() => true);
		const addEventListener = vi.fn<MediaQueryList['addEventListener']>();
		const removeEventListener = vi.fn<MediaQueryList['removeEventListener']>();
		const mediaQuery = { addEventListener, removeEventListener } as unknown as MediaQueryList;
		const matchMedia = vi.fn<(query: string) => MediaQueryList>(() => mediaQuery);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Reflect.set(terminal, 'renderer', { setDevicePixelRatio });
		vi.stubGlobal('window', { devicePixelRatio: 2, matchMedia });
		vi.stubGlobal('requestAnimationFrame', requestAnimationFrame);
		try {
			const syncDevicePixelRatio = Reflect.get(terminal, 'syncDevicePixelRatio') as () => void;
			syncDevicePixelRatio.call(terminal);

			expect(setDevicePixelRatio).toHaveBeenCalledWith(2);
			expect(matchMedia).toHaveBeenCalledWith('(resolution: 2dppx)');
			expect(addEventListener).toHaveBeenCalledWith('change', expect.any(Function));
			expect(requestAnimationFrame).toHaveBeenCalledOnce();
		} finally {
			Reflect.set(terminal, 'renderLoopId', 0);
			Reflect.set(terminal, 'renderer', undefined);
			vi.unstubAllGlobals();
			terminal.dispose();
		}
	});

	it('synchronizes renderer focus for Ghostty-style cursor presentation', () => {
		const terminal = createTerminal();
		const setFocused = vi.fn<(focused: boolean) => void>();
		const cancelActiveGesture = vi.fn<() => void>();
		const releasePressedKeys = vi.fn<() => void>();
		const releasePressedMouseButtons = vi.fn<() => void>();
		const cancelComposition = vi.fn<() => void>();
		vi.stubGlobal('Node', Object);
		Reflect.set(terminal, 'renderer', { setFocused });
		Reflect.set(terminal, 'selectionManager', { cancelActiveGesture });
		Reflect.set(terminal, 'inputHandler', {
			releasePressedKeys,
			releasePressedMouseButtons,
			cancelComposition,
		});
		Reflect.set(terminal, 'draggingScrollbar', true);
		const handleFocusIn = Reflect.get(terminal, 'handleFocusIn') as (
			event: Pick<FocusEvent, 'relatedTarget'>,
		) => void;
		const handleFocusOut = Reflect.get(terminal, 'handleFocusOut') as (
			event: Pick<FocusEvent, 'relatedTarget'>,
		) => void;

		handleFocusIn.call(terminal, { relatedTarget: null });
		handleFocusOut.call(terminal, { relatedTarget: null });

		expect(setFocused.mock.calls).toEqual([[true], [false]]);
		expect(cancelActiveGesture).toHaveBeenCalledOnce();
		expect(releasePressedKeys).toHaveBeenCalledOnce();
		expect(releasePressedMouseButtons).toHaveBeenCalledOnce();
		expect(cancelComposition).toHaveBeenCalledOnce();
		expect(Reflect.get(terminal, 'draggingScrollbar')).toBe(false);
		Reflect.set(terminal, 'renderer', undefined);
		Reflect.set(terminal, 'selectionManager', undefined);
		Reflect.set(terminal, 'inputHandler', undefined);
		terminal.dispose();
	});

	it('reports focus changes without leaving the viewed history', () => {
		const terminal = new Terminal({ cols: 20, rows: 3, scrollback: 10_000 });
		terminal.write(createLines('history', 20));
		terminal.write('\x1b[?1004h');
		terminal.scrollToTop();
		const viewport = terminal.viewportY;
		const data: string[] = [];
		terminal.onData((value) => data.push(value));
		const handleFocusIn = Reflect.get(terminal, 'handleFocusIn') as (
			event: Pick<FocusEvent, 'relatedTarget'>,
		) => void;
		const handleFocusOut = Reflect.get(terminal, 'handleFocusOut') as (
			event: Pick<FocusEvent, 'relatedTarget'>,
		) => void;
		vi.stubGlobal('Node', Object);

		try {
			handleFocusOut.call(terminal, { relatedTarget: null });
			handleFocusIn.call(terminal, { relatedTarget: null });

			expect(data).toEqual(['\x1b[O', '\x1b[I']);
			expect(terminal.viewportY).toBe(viewport);
		} finally {
			vi.unstubAllGlobals();
			terminal.dispose();
		}
	});

	it('does not report focus again when focus moves within the terminal surface', () => {
		const terminal = createTerminal();
		terminal.write('\x1b[?1004h');
		const data: string[] = [];
		terminal.onData((value) => data.push(value));
		const inside = {};
		Reflect.set(terminal, 'hostElement', { contains: (target: unknown) => target === inside });
		const handleFocusIn = Reflect.get(terminal, 'handleFocusIn') as (
			event: Pick<FocusEvent, 'relatedTarget'>,
		) => void;
		vi.stubGlobal('Node', Object);

		try {
			handleFocusIn.call(terminal, { relatedTarget: null });
			handleFocusIn.call(terminal, { relatedTarget: inside as EventTarget });
			expect(data).toEqual(['\x1b[I']);
		} finally {
			vi.unstubAllGlobals();
			Reflect.set(terminal, 'hostElement', undefined);
			terminal.dispose();
		}
	});

	it('writes strings, bytes, and lines', () => {
		const terminal = createTerminal();
		terminal.write('A');
		terminal.write(new TextEncoder().encode('B'));
		terminal.writeln('C');
		terminal.grid.update();
		expect(terminal.cursorY).toBe(1);
		expect(
			terminal.grid
				.readViewport()
				.slice(0, 3)
				.map((cell) => cell.text),
		).toEqual(['A', 'B', 'C']);
		terminal.dispose();
	});

	it('searches both scrollback history and the active screen', () => {
		const terminal = createTerminal(12, 2, { scrollbackLimit: 10 });
		terminal.write('first match\r\nsecond\r\nlast MATCH');
		const expected = [
			{
				text: 'match',
				start: { column: 6, row: 0 },
				end: { column: 10, row: 0 },
			},
			{
				text: 'MATCH',
				start: { column: 5, row: 2 },
				end: { column: 9, row: 2 },
			},
		];
		expect(terminal.findAll('match')).toEqual(expected);
		expect(terminal.highlightSearch('match')).toEqual(expected);
		terminal.clearSearch();
		terminal.dispose();
	});

	it('navigates matches in Ghostty order and wraps in both directions', () => {
		const terminal = createTerminal(10, 2, { scrollbackLimit: 10 });
		terminal.write('Fizz\r\nBuzz\r\nFizz\r\nBang');
		let selection: AbsoluteSelection | null = null;
		const selectAbsoluteRange = vi.fn<
			(start: { x: number; y: number }, end: { x: number; y: number }) => boolean
		>((start: { x: number; y: number }, end: { x: number; y: number }): boolean => {
			selection = { start, end };
			return true;
		});
		const clearSelection = vi.fn<() => void>(() => {
			selection = null;
		});
		Reflect.set(terminal, 'selectionManager', {
			getSelectionPosition: () => selectionInBuffer(selection),
			selectAbsoluteRange,
			clearSelection,
		});

		expect(terminal.findNext('Fizz')).toBe(true);
		expect(selection).toMatchObject({ start: { x: 0, y: 2 } });
		expect(terminal.findNext('Fizz')).toBe(true);
		expect(selection).toMatchObject({ start: { x: 0, y: 0 } });
		expect(terminal.findNext('Fizz')).toBe(true);
		expect(selection).toMatchObject({ start: { x: 0, y: 2 } });
		expect(terminal.findPrevious('Fizz')).toBe(true);
		expect(selection).toMatchObject({ start: { x: 0, y: 0 } });
		expect(terminal.findPrevious('Fizz')).toBe(true);
		expect(selection).toMatchObject({ start: { x: 0, y: 2 } });
		expect(clearSelection).not.toHaveBeenCalled();

		Reflect.set(terminal, 'selectionManager', undefined);
		terminal.dispose();
	});

	it('traverses every scrollback match in buffer order', () => {
		const terminal = createTerminal(12, 2, { scrollbackLimit: 40 });
		terminal.write('match-0\r\nfiller\r\nmatch-1\r\nfiller\r\nmatch-2\r\nfiller\r\nmatch-3');
		const matches = terminal.findAll('match');
		let selection: AbsoluteSelection | null = null;
		Reflect.set(terminal, 'selectionManager', {
			getSelectionPosition: () => selectionInBuffer(selection),
			selectAbsoluteRange: (start: { x: number; y: number }, end: { x: number; y: number }) => {
				selection = { start, end };
				return true;
			},
			clearSelection: () => {
				selection = null;
			},
		});
		const visited: number[] = [];

		for (let index = 0; index < matches.length; index++) {
			expect(terminal.findNext('match')).toBe(true);
			const current = selection as AbsoluteSelection | null;
			if (current) visited.push(current.start.y);
		}

		expect(visited).toEqual(
			matches.map((_match, index) => matches[matches.length - index - 1]?.start.row),
		);
		Reflect.set(terminal, 'selectionManager', undefined);
		terminal.dispose();
	});

	it('drops the search selection after all matches disappear', () => {
		const terminal = createTerminal(10, 2);
		terminal.write('Fizz');
		const clearSelection = vi.fn<() => void>();
		const setSearchMatches = vi.fn<() => void>();
		Reflect.set(terminal, 'selectionManager', {
			getSelectionPosition: () => ({ start: { x: 0, y: 0 }, end: { x: 3, y: 0 } }),
			selectAbsoluteRange: () => true,
			clearSelection,
		});
		Reflect.set(terminal, 'renderer', { setSearchMatches });

		expect(terminal.findNext('missing')).toBe(false);
		expect(clearSelection).toHaveBeenCalledOnce();
		expect(setSearchMatches).toHaveBeenCalledWith([]);

		Reflect.set(terminal, 'selectionManager', undefined);
		Reflect.set(terminal, 'renderer', undefined);
		terminal.dispose();
	});

	it('recomputes search results after active-screen contents change', () => {
		const terminal = createTerminal(10, 3);
		terminal.write('Fizz\r\nBuzz');
		expect(terminal.findAll('Fizz')).toMatchObject([
			{ start: { column: 0, row: 0 }, end: { column: 3, row: 0 } },
		]);

		terminal.write('\x1b[2J\x1b[HBang\r\nFizz');
		expect(terminal.findAll('Fizz')).toMatchObject([
			{ start: { column: 0, row: 1 }, end: { column: 3, row: 1 } },
		]);
		terminal.dispose();
	});

	it('invalidates rendered search highlights when terminal content changes', () => {
		const terminal = createTerminal(10, 3);
		terminal.write('search');
		const setSearchMatches = vi.fn<() => void>();
		Reflect.set(terminal, 'renderer', { setSearchMatches });
		Reflect.set(terminal, 'renderLoopId', 1);

		expect(terminal.highlightSearch('search')).toHaveLength(1);
		terminal.write('\r\x1b[2Kreplacement');

		expect(setSearchMatches).toHaveBeenLastCalledWith([]);
		expect(Reflect.get(terminal, 'searchHighlightsActive')).toBe(false);
		Reflect.set(terminal, 'renderLoopId', 0);
		Reflect.set(terminal, 'renderer', undefined);
		terminal.dispose();
	});

	it('wraps previous search when every match is in scrollback', () => {
		const terminal = createTerminal(10, 2, { scrollbackLimit: 20 });
		terminal.write('Fizz\r\nblank\r\nblank\r\nblank');
		const matches = terminal.findAll('Fizz');
		expect(matches).toHaveLength(1);
		expect(matches[0]?.start.row).toBeLessThan(terminal.grid.getScrollbackLength());

		let selection: AbsoluteSelection | null = null;
		Reflect.set(terminal, 'selectionManager', {
			getSelectionPosition: () => selectionInBuffer(selection),
			selectAbsoluteRange: (start: { x: number; y: number }, end: { x: number; y: number }) => {
				selection = { start, end };
				return true;
			},
			clearSelection: () => {
				selection = null;
			},
		});

		expect(terminal.findNext('Fizz')).toBe(true);
		expect(terminal.findPrevious('Fizz')).toBe(true);
		expect(selection).toMatchObject({ start: { x: 0, y: matches[0]?.start.row } });

		Reflect.set(terminal, 'selectionManager', undefined);
		terminal.dispose();
	});

	it('searches a soft-wrapped match across the scrollback boundary', () => {
		const terminal = createTerminal(4, 2, { scrollbackLimit: 10 });
		terminal.write('A\r\nxxboo!\r\nC');

		expect(terminal.findAll('boo!')).toEqual([
			{
				text: 'boo!',
				start: { column: 2, row: 1 },
				end: { column: 1, row: 2 },
			},
		]);
		terminal.dispose();
	});

	it('searches only the active screen and never primary-screen history from alt screen', () => {
		const terminal = createTerminal(10, 2, { scrollbackLimit: 10 });
		terminal.write('Fizz\r\nblank\r\nblank');
		expect(terminal.findAll('Fizz')).toHaveLength(1);

		terminal.write('\x1b[?1049hAlt only');
		expect(terminal.findAll('Fizz')).toEqual([]);
		expect(terminal.findAll('Alt')).toHaveLength(1);

		terminal.write('\x1b[?1049l');
		expect(terminal.findAll('Fizz')).toHaveLength(1);
		terminal.dispose();
	});

	it('keeps search navigation valid after clearing scrollback', () => {
		const terminal = createTerminal(10, 2, { scrollbackLimit: 20 });
		terminal.write('error\r\nerror\r\nerror\r\nerror.');
		let selection: AbsoluteSelection | null = null;
		Reflect.set(terminal, 'selectionManager', {
			getSelectionPosition: () => selectionInBuffer(selection),
			selectAbsoluteRange: (start: { x: number; y: number }, end: { x: number; y: number }) => {
				selection = { start, end };
				return true;
			},
			clearSelection: () => {
				selection = null;
			},
		});

		expect(terminal.findNext('error')).toBe(true);
		terminal.write('\x1b[3J');
		expect(() => terminal.findNext('error')).not.toThrow();
		expect(() => terminal.findPrevious('error')).not.toThrow();

		Reflect.set(terminal, 'selectionManager', undefined);
		terminal.dispose();
	});

	it('reports the primary-buffer transition immediately when reset leaves alternate screen', () => {
		const terminal = createTerminal();
		terminal.write('\x1b[?1049hAlternate');
		expect(terminal.buffer.active.type).toBe('alternate');
		const changes: string[] = [];
		terminal.buffer.onBufferChange((buffer) => changes.push(buffer.type));

		terminal.reset();

		expect(terminal.buffer.active.type).toBe('normal');
		expect(changes).toEqual(['normal']);
		expect(Reflect.get(terminal, 'lastAlternateScreen')).toBe(false);
		terminal.write('next');
		expect(changes).toEqual(['normal']);
		terminal.dispose();
	});

	it('resizes and safely disposes twice', () => {
		const terminal = createTerminal();
		const resizes: { cols: number; rows: number }[] = [];
		terminal.onResize((event) => resizes.push(event));
		terminal.resize(20, 5);
		expect([terminal.cols, terminal.rows]).toEqual([20, 5]);
		expect([terminal.options.cols, terminal.options.rows]).toEqual([20, 5]);
		expect(resizes).toEqual([{ cols: 20, rows: 5 }]);
		expect(() => terminal.dispose()).not.toThrow();
		expect(() => terminal.dispose()).not.toThrow();
	});

	it('keeps the visible history row anchored across resize reflow', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 10_000 });
		const scrollEvents: number[] = [];
		terminal.onScroll((viewportY) => scrollEvents.push(viewportY));
		for (let index = 0; index < 12; index++) {
			terminal.writeln(`ROW${String(index).padStart(2, '0')}-abc`);
		}
		terminal.scrollToTop();
		terminal.scrollLines(2);
		const beforeTop = terminal.getScrollbackLength() - Math.floor(terminal.viewportY);
		const before = terminal.grid
			.readScrollbackLine(beforeTop)
			?.cells.map((cell) => cell.text)
			.join('')
			.trimEnd();
		scrollEvents.length = 0;

		terminal.resize(6, 3);

		const afterTop = terminal.getScrollbackLength() - Math.floor(terminal.viewportY);
		const after = terminal.grid
			.readScrollbackLine(afterTop)
			?.cells.map((cell) => cell.text)
			.join('')
			.trimEnd();
		expect(before).toBe('ROW02-abc');
		expect(after).toBe('ROW02-');
		expect(scrollEvents).toEqual([terminal.viewportY]);
		terminal.dispose();
	});

	it('disposes loaded addons', () => {
		const terminal = createTerminal();
		let activated = false;
		let disposed = false;
		terminal.loadAddon({
			activate: () => {
				activated = true;
			},
			dispose: () => {
				disposed = true;
			},
		});
		terminal.dispose();
		expect(activated).toBe(true);
		expect(disposed).toBe(true);
	});
});

describe('Terminal callbacks and state', () => {
	it('emits device status responses', () => {
		const terminal = createTerminal();
		const responses: string[] = [];
		const disposable = terminal.onData((data) => responses.push(data));
		terminal.write('\x1b[5n\x1b[6n');
		expect(responses).toEqual(['\x1b[0n', '\x1b[1;1R']);
		disposable.dispose();
		terminal.dispose();
	});

	it('answers XTWINOPS queries from cached terminal geometry', () => {
		const terminal = createTerminal(10, 2);
		const responses: string[] = [];
		terminal.onData((data) => responses.push(data));

		terminal.write('\x1b[14t\x1b[16t\x1b[18t');

		expect(responses).toEqual(['\x1b[4;32;80t', '\x1b[6;16;8t', '\x1b[8;2;10t']);
		terminal.dispose();
	});

	it('answers color-scheme queries from the active terminal background', () => {
		const dark = new Terminal({ cols: 10, rows: 2, theme: { background: '#111111' } });
		const light = new Terminal({ cols: 10, rows: 2, theme: { background: '#f5f5f5' } });
		const darkResponses: string[] = [];
		const lightResponses: string[] = [];
		dark.onData((data) => darkResponses.push(data));
		light.onData((data) => lightResponses.push(data));

		dark.write('\x1b[?996n');
		light.write('\x1b[?996n');

		expect(darkResponses).toEqual(['\x1b[?997;1n']);
		expect(lightResponses).toEqual(['\x1b[?997;2n']);
		dark.dispose();
		light.dispose();
	});

	it('reports the configured product identity through XTVERSION', () => {
		const terminal = new Terminal({ identity: { name: 'orchbee-test', version: '2.4.0' } });
		const responses: string[] = [];
		terminal.onData((data) => responses.push(data));

		terminal.write('\x1b[>q');

		expect(responses).toEqual(['\x1bP>|orchbee-test 2.4.0\x1b\\']);
		terminal.dispose();
	});

	it('uses a cached configurable ENQ answerback', () => {
		const terminal = new Terminal({ enquiryResponse: 'orchbee-ready' });
		const responses: string[] = [];
		terminal.onData((data) => responses.push(data));

		terminal.write('\x05');

		expect(responses).toEqual(['orchbee-ready']);
		terminal.dispose();
	});

	it('reports Ghostty device attributes without claiming disabled graphics', () => {
		const terminal = createTerminal();
		const responses: string[] = [];
		terminal.onData((data) => responses.push(data));

		terminal.write('\x1b[c\x1b[>c\x1b[=c');

		expect(responses).toEqual(['\x1b[?62;22c', '\x1b[>1;0;0c', '\x1bP!|00000000\x1b\\']);
		terminal.dispose();
	});

	it('emits bell and title changes', () => {
		const terminal = createTerminal();
		let bells = 0;
		const titles: string[] = [];
		terminal.onBell(() => bells++);
		terminal.onTitleChange((title) => titles.push(title));
		terminal.write('\x07\x1b]2;Orchbee Agent\x07');
		expect(bells).toBe(1);
		expect(titles).toEqual(['Orchbee Agent']);
		terminal.dispose();
	});

	it('emits working-directory changes with the raw Ghostty value', () => {
		const terminal = createTerminal();
		const directories: string[] = [];
		terminal.onPwdChange((directory) => directories.push(directory));

		terminal.write('\x1b]7;file://host/tmp/project\x07');

		expect(directories).toEqual(['file://host/tmp/project']);
		expect(terminal.getPwd()).toBe('file://host/tmp/project');
		terminal.dispose();
	});

	it('tracks cursor visibility, style, and position', () => {
		const terminal = new Terminal({ cursorStyle: 'underline', cursorBlink: false });
		terminal.write('AB\x1b[?25l\x1b[5 q');
		terminal.grid.update();
		expect(terminal.getCursor()).toEqual({ x: 2, y: 0, visible: false });
		expect(terminal.cursorStyle).toBe('bar');
		expect(terminal.cursorBlinking).toBe(true);
		terminal.dispose();
	});

	it('toggles terminal modes', () => {
		const terminal = createTerminal();
		terminal.write('\x1b[?2004h\x1b[?1004h');
		expect(terminal.getMode(DecMode.BRACKETED_PASTE)).toBe(true);
		expect(terminal.getMode(DecMode.FOCUS_EVENTS)).toBe(true);
		expect(terminal.getMode(AnsiMode.INSERT, true)).toBe(false);
		terminal.dispose();
	});

	it('restores primary content after alternate-screen output', () => {
		const terminal = createTerminal(20, 4);
		terminal.write('primary');
		terminal.write('\x1b[?1049h\x1b[2J\x1b[HALT');
		expect(terminal.isAlternateScreen()).toBe(true);
		terminal.write('\x1b[?1049l');
		terminal.grid.update();
		expect(terminal.grid.readViewport()[0]?.text).toBe('p');
		terminal.dispose();
	});

	it('updates Ghostty palette colors from theme options', () => {
		const terminal = new Terminal({ cols: 8, rows: 2, theme: { red: '#010203' } });
		terminal.write('\x1b[31mR');
		terminal.grid.update();
		expect(terminal.grid.readViewport()[0]?.fg).toEqual({ r: 1, g: 2, b: 3 });
		terminal.dispose();
	});

	it('restores omitted runtime theme colors to defaults', () => {
		const terminal = createTerminal(8, 2);
		terminal.options.theme = {
			foreground: '#010203',
			background: '#fefdfc',
			cursor: '#112233',
		};
		terminal.grid.update();
		expect(terminal.getColors()).toEqual({
			foreground: { r: 1, g: 2, b: 3 },
			background: { r: 254, g: 253, b: 252 },
			cursor: { r: 17, g: 34, b: 51 },
		});

		terminal.options.theme = {};
		terminal.grid.update();

		expect(terminal.getColors()).toEqual({
			foreground: { r: 212, g: 212, b: 212 },
			background: { r: 30, g: 30, b: 30 },
			cursor: { r: 255, g: 255, b: 255 },
		});
		terminal.dispose();
	});

	it('exposes OSC default-color overrides and restores configured defaults', () => {
		const terminal = new Terminal({ cols: 8, rows: 2, theme: { background: '#010203' } });
		terminal.write('\x1b]11;rgb:12/34/56\x1b\\');
		terminal.grid.update();
		expect(terminal.getColors().background).toEqual({ r: 18, g: 52, b: 86 });
		terminal.write('\x1b]111\x1b\\');
		terminal.grid.update();
		expect(terminal.getColors().background).toEqual({ r: 1, g: 2, b: 3 });
		terminal.dispose();
	});

	it('keeps a scrolled viewport anchored when history grows', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 20 });
		for (let index = 0; index < 6; index++) terminal.writeln(`line-${index}`);
		terminal.scrollLines(-2);
		const before = terminal.buffer.active.viewportY;
		terminal.writeln('new-line');
		expect(terminal.buffer.active.viewportY).toBe(before);
		terminal.dispose();
	});

	it('clears a stale hovered link when the viewport moves', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 20 });
		for (let index = 0; index < 8; index++) terminal.writeln(`line-${index}`);
		const hover = vi.fn<(active: boolean) => void>();
		Reflect.set(terminal, 'hoveredLink', {
			text: 'https://example.com/stale',
			range: { start: { x: 0, y: 7 }, end: { x: 24, y: 7 } },
			hover,
		});

		terminal.scrollToTop();

		expect(hover).toHaveBeenCalledWith(false);
		expect(Reflect.get(terminal, 'hoveredLink')).toBeUndefined();
		terminal.dispose();
	});

	it('clears a stale hovered link when terminal content changes', () => {
		const terminal = createTerminal();
		const hover = vi.fn<(active: boolean) => void>();
		Reflect.set(terminal, 'hoveredLink', {
			text: 'https://example.com/stale',
			range: { start: { x: 0, y: 0 }, end: { x: 24, y: 0 } },
			hover,
		});

		terminal.write('replacement');

		expect(hover).toHaveBeenCalledWith(false);
		expect(Reflect.get(terminal, 'hoveredLink')).toBeUndefined();
		terminal.dispose();
	});

	it('repairs a lost scrollbar mouseup from buttonless pointer movement', () => {
		const terminal = createTerminal(80, 24, { scrollbackLimit: 10_000_000 });
		for (let index = 0; index < 200; index++) terminal.writeln(`line-${index}`);
		const canvas = {
			getBoundingClientRect: () => ({ left: 0, top: 0, width: 640, height: 408 }),
		};
		Reflect.set(terminal, 'canvas', canvas);
		const handleMouseDown = Reflect.get(terminal, 'handleScrollbarMouseDown') as (
			event: MouseEvent,
		) => void;
		const handleMouseMove = Reflect.get(terminal, 'handleScrollbarMouseMove') as (
			event: MouseEvent,
		) => void;
		handleMouseDown.call(terminal, {
			button: 0,
			clientX: 639,
			clientY: 300,
			preventDefault: () => {},
			stopImmediatePropagation: () => {},
		} as unknown as MouseEvent);
		const viewportBeforeMove = terminal.viewportY;

		expect(Reflect.get(terminal, 'draggingScrollbar')).toBe(true);
		handleMouseMove.call(terminal, { buttons: 0, clientY: 100 } as MouseEvent);

		expect(Reflect.get(terminal, 'draggingScrollbar')).toBe(false);
		expect(terminal.viewportY).toBe(viewportBeforeMove);
		Reflect.set(terminal, 'canvas', undefined);
		terminal.dispose();
	});

	it('keeps Shift+PageUp and Shift+PageDown local to scrollback navigation', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 20 });
		for (let index = 0; index < 8; index++) terminal.writeln(`line-${index}`);
		const data: string[] = [];
		terminal.onData((value) => data.push(value));
		const dispatch = Reflect.get(terminal, 'dispatchCustomKeyEvent') as (
			event: KeyboardEvent,
		) => boolean;

		expect(dispatch.call(terminal, createShiftKeyEvent('PageUp'))).toBe(true);
		expect(terminal.viewportY).toBeGreaterThan(0);
		expect(dispatch.call(terminal, createShiftKeyEvent('PageDown'))).toBe(true);
		expect(terminal.viewportY).toBe(0);
		expect(data).toEqual([]);
		terminal.dispose();
	});

	it('leaves modified scrollback keys available to application key handlers', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 20 });
		for (let index = 0; index < 8; index++) terminal.writeln(`line-${index}`);
		const handle = vi.fn<(event: KeyboardEvent) => boolean>(() => true);
		terminal.registerKeyEventHandler({
			id: 'application.shift-page-up',
			priority: 100,
			scope: 'application',
			handle,
		});
		const dispatch = Reflect.get(terminal, 'dispatchCustomKeyEvent') as (
			event: KeyboardEvent,
		) => boolean;
		const event = createShiftKeyEvent('PageUp');

		expect(dispatch.call(terminal, event)).toBe(true);
		expect(handle).toHaveBeenCalledWith(event, {
			alternateScreen: false,
			hasSelection: false,
			readOnly: false,
		});
		expect(terminal.viewportY).toBe(0);
		terminal.dispose();
	});

	it('reveals the active prompt before emitting user input from history', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 20 });
		for (let index = 0; index < 8; index++) terminal.writeln(`line-${index}`);
		terminal.scrollPages(-1);
		expect(terminal.viewportY).toBeGreaterThan(0);
		const data: string[] = [];
		terminal.onData((value) => data.push(value));

		terminal.input('typed', true);

		expect(terminal.viewportY).toBe(0);
		expect(data).toEqual(['typed']);
		terminal.dispose();
	});

	it('reveals the active prompt when IME composition starts in history', () => {
		const terminal = createTerminal(12, 3, { scrollbackLimit: 20 });
		for (let index = 0; index < 8; index++) terminal.writeln(`line-${index}`);
		terminal.scrollPages(-1);
		expect(terminal.viewportY).toBeGreaterThan(0);
		const handleComposition = Reflect.get(terminal, 'handleComposition') as (
			data: string | null,
			selection?: { readonly start: number; readonly end: number },
		) => void;

		handleComposition.call(terminal, '', { start: 0, end: 0 });

		expect(terminal.viewportY).toBe(0);
		terminal.scrollPages(-1);
		handleComposition.call(terminal, null);
		expect(terminal.viewportY).toBeGreaterThan(0);
		terminal.dispose();
	});

	it('confirms, sanitizes, and encodes unsafe paste', async () => {
		const confirmPaste = vi.fn<NonNullable<TerminalHost['confirmPaste']>>(async () => true);
		const terminal = new Terminal({ cols: 10, rows: 2, host: { confirmPaste } });
		const data: string[] = [];
		terminal.onData((value) => data.push(value));

		const result = await terminal.pasteAsync('first\nsec\x1bond', 'clipboard');

		expect(result).toMatchObject({
			kind: 'emitted',
			source: 'clipboard',
			inspection: {
				classification: 'confirm',
				sanitized: 'first\nsec ond',
				replacedControlBytes: 1,
			},
		});
		expect(confirmPaste).toHaveBeenCalledOnce();
		expect(data).toEqual(['first\rsec ond']);
		terminal.dispose();
	});

	it('treats ordinary bracketed multiline paste as safe', async () => {
		const confirmPaste = vi.fn<NonNullable<TerminalHost['confirmPaste']>>(async () => true);
		const terminal = new Terminal({ cols: 10, rows: 2, host: { confirmPaste } });
		const data: string[] = [];
		terminal.onData((value) => data.push(value));
		terminal.write('\x1b[?2004h');

		await expect(terminal.pasteAsync('first\nsecond')).resolves.toMatchObject({
			kind: 'emitted',
			inspection: { bracketed: true, classification: 'safe' },
		});
		expect(confirmPaste).not.toHaveBeenCalled();
		expect(data).toEqual(['\x1b[200~first\nsecond\x1b[201~']);
		terminal.dispose();
	});

	it('does not emit paste data when confirmation is rejected', async () => {
		const terminal = new Terminal({
			cols: 10,
			rows: 2,
			host: { confirmPaste: async () => false },
		});
		const data: string[] = [];
		terminal.onData((value) => data.push(value));

		await expect(terminal.pasteAsync('first\nsecond')).resolves.toMatchObject({
			kind: 'cancelled',
		});
		expect(data).toEqual([]);
		terminal.dispose();
	});

	it('preserves input order while an asynchronous paste confirmation is pending', async () => {
		let resolveConfirmation: ((confirmed: boolean) => void) | undefined;
		const confirmation = new Promise<boolean>((resolve) => {
			resolveConfirmation = resolve;
		});
		const terminal = new Terminal({
			cols: 10,
			rows: 2,
			host: { confirmPaste: () => confirmation },
		});
		const data: string[] = [];
		terminal.onData((value) => data.push(value));

		const paste = terminal.pasteAsync('first\nsecond', 'clipboard');
		terminal.input('typed', true);
		const barrier = terminal.pasteAsync('');
		await Promise.resolve();
		expect(data).toEqual([]);

		resolveConfirmation?.(true);
		await Promise.all([paste, barrier]);
		expect(data).toEqual(['first\rsecond', 'typed']);
		terminal.dispose();
	});

	it('reports a paste as blocked when stdin is disabled during confirmation', async () => {
		let resolveConfirmation: ((confirmed: boolean) => void) | undefined;
		const confirmation = new Promise<boolean>((resolve) => {
			resolveConfirmation = resolve;
		});
		const terminal = new Terminal({
			cols: 10,
			rows: 2,
			host: { confirmPaste: () => confirmation },
		});
		const data: string[] = [];
		terminal.onData((value) => data.push(value));

		const paste = terminal.pasteAsync('first\nsecond', 'clipboard');
		await Promise.resolve();
		terminal.options.disableStdin = true;
		resolveConfirmation?.(true);

		await expect(paste).resolves.toMatchObject({
			kind: 'blocked',
			reason: 'stdin-disabled',
			source: 'clipboard',
		});
		expect(data).toEqual([]);
		terminal.dispose();
	});

	it('delivers queued state-repair input without changing the viewed history', async () => {
		let resolveConfirmation: ((confirmed: boolean) => void) | undefined;
		const confirmation = new Promise<boolean>((resolve) => {
			resolveConfirmation = resolve;
		});
		const terminal = new Terminal({
			cols: 10,
			rows: 2,
			scrollback: 10_000,
			host: { confirmPaste: () => confirmation },
		});
		const data: string[] = [];
		const binary: string[] = [];
		terminal.onData((value) => data.push(value));
		terminal.onBinary((value) => binary.push(value));
		terminal.write(createLines('history', 20));
		terminal.scrollToTop();
		const viewport = terminal.viewportY;
		const paste = terminal.pasteAsync('first\nsecond', 'clipboard');
		await Promise.resolve();
		terminal.options.disableStdin = true;
		const emitStateRepairInput = Reflect.get(terminal, 'emitStateRepairInput') as (
			data: string,
		) => void;
		const createInputDeps = Reflect.get(terminal, 'createInputDeps') as () => {
			emitStateRepairBinary?: (data: string) => void;
		};

		emitStateRepairInput.call(terminal, 'mouse-release');
		createInputDeps.call(terminal).emitStateRepairBinary?.('binary-release');
		resolveConfirmation?.(true);
		await paste;
		await terminal.pasteAsync('');

		expect(data).toEqual(['mouse-release']);
		expect(binary).toEqual(['binary-release']);
		expect(terminal.viewportY).toBe(viewport);
		terminal.dispose();
	});

	it('routes dropped files through host path policy before paste policy', async () => {
		const file = {
			name: 'report final.txt',
			size: 12,
			type: 'text/plain',
			lastModified: 1,
		} as File;
		const resolveDrop = vi.fn<NonNullable<TerminalHost['resolveDrop']>>(async () => ({
			text: "'/tmp/report final.txt'",
		}));
		const terminal = new Terminal({ cols: 10, rows: 2, host: { resolveDrop } });
		const data: string[] = [];
		terminal.onData((value) => data.push(value));
		terminal.write('\x1b]7;file://localhost/tmp/project\x07');

		await expect(terminal.dropAsync('', [file])).resolves.toMatchObject({
			kind: 'emitted',
			source: 'drop',
		});

		expect(resolveDrop).toHaveBeenCalledWith({
			text: '',
			files: [file],
			cwd: 'file://localhost/tmp/project',
		});
		expect(data).toEqual(["'/tmp/report final.txt'"]);
		terminal.dispose();
	});

	it('accepts dropped text but blocks files without a host path policy', async () => {
		const terminal = createTerminal();
		const data: string[] = [];
		terminal.onData((value) => data.push(value));

		await expect(terminal.dropAsync('dropped text')).resolves.toMatchObject({
			kind: 'emitted',
			source: 'drop',
		});
		await expect(terminal.dropAsync('', [{} as File])).resolves.toMatchObject({
			kind: 'blocked',
			reason: 'file-drop-unsupported',
		});
		expect(data).toEqual(['dropped text']);
		terminal.dispose();
	});

	it('executes built-in paste through the action registry', async () => {
		const read = vi.fn<TerminalClipboardHost['read']>(async () => ({ text: 'from clipboard' }));
		const terminal = new Terminal({
			cols: 10,
			rows: 2,
			host: {
				clipboard: {
					read,
					write: async () => {},
				},
			},
		});
		const data: string[] = [];
		terminal.onData((value) => data.push(value));

		await expect(terminal.executeAction('terminal.paste', 'keyboard')).resolves.toEqual({
			kind: 'consumed',
		});
		expect(read).toHaveBeenCalledWith({ target: 'standard', source: 'paste' });
		expect(data).toEqual(['from clipboard']);
		terminal.dispose();
	});

	it('exposes deterministic built-in action state', () => {
		const terminal = createTerminal();
		const context = { source: 'api' as const, signal: new AbortController().signal };
		expect(terminal.actions.getState('terminal.copy', context)).toEqual({
			enabled: false,
			checked: false,
		});
		expect(terminal.actions.list().map((action) => action.id)).toEqual([
			'terminal.open-link',
			'terminal.copy',
			'terminal.paste',
			'terminal.select-all',
			'terminal.select-output',
			'terminal.clear-selection',
			'terminal.reset',
		]);
		terminal.dispose();
	});

	it('requires the Ghostty link modifier before enabling hover activation', () => {
		const terminal = createTerminal();
		const isLinkActivationEnabled = Reflect.get(terminal, 'isLinkActivationEnabled') as (
			link: { readonly activation?: 'direct' | 'modifier' },
			modifiers: {
				readonly alt: boolean;
				readonly control: boolean;
				readonly meta: boolean;
				readonly shift: boolean;
			},
		) => boolean;
		const modifierLink = { activation: 'modifier' as const };
		const noModifiers = { alt: false, control: false, meta: false, shift: false };

		expect(isLinkActivationEnabled.call(terminal, modifierLink, noModifiers)).toBe(false);
		expect(
			isLinkActivationEnabled.call(terminal, modifierLink, {
				...noModifiers,
				control: true,
			}),
		).toBe(true);
		expect(isLinkActivationEnabled.call(terminal, { activation: 'direct' }, noModifiers)).toBe(
			true,
		);
		terminal.dispose();
	});

	it('sends serializable interaction context to the host and executes its selected action', async () => {
		const requests: ContextMenuRequest[] = [];
		const terminal = new Terminal({
			cols: 10,
			rows: 3,
			host: {
				showContextMenu: async (request) => {
					requests.push(request);
					return 'terminal.reset';
				},
			},
		});
		(terminal as unknown as { canvas: HTMLCanvasElement }).canvas = {
			getBoundingClientRect: () => ({ left: 4, top: 8 }),
			removeEventListener: () => {},
			remove: () => {},
		} as unknown as HTMLCanvasElement;
		terminal.write('abc');
		terminal.grid.update();
		expect(terminal.cursorX).toBe(3);

		await expect(
			terminal.requestContextMenuAt(14, 28, {
				alt: false,
				control: true,
				meta: false,
				shift: true,
			}),
		).resolves.toEqual({ kind: 'consumed' });

		expect(requests).toHaveLength(1);
		expect(requests[0]).toMatchObject({
			x: 14,
			y: 28,
			context: {
				cell: { column: 1, row: 1, bufferRow: 1 },
				selection: null,
				link: null,
				semanticZone: 'output',
				cwd: null,
				modifiers: { alt: false, control: true, meta: false, shift: true },
				mouseReporting: false,
			},
		});
		expect(requests[0]?.actionIds).toEqual([
			'terminal.copy',
			'terminal.paste',
			'terminal.select-all',
			'terminal.select-output',
			'terminal.clear-selection',
			'terminal.reset',
		]);
		expect(requests[0]?.actions.find((action) => action.id === 'terminal.copy')?.enabled).toBe(
			false,
		);
		terminal.grid.update();
		expect(terminal.cursorX).toBe(0);
		(terminal as unknown as { canvas: HTMLCanvasElement | undefined }).canvas = undefined;
		terminal.dispose();
	});

	it('reads context-menu semantic zones from the visible scrollback row', () => {
		const terminal = createTerminal(10, 2, { scrollbackLimit: 20 });
		terminal.write('\x1b]133;A\x07PROMPT\r\n\x1b]133;C\x07output-1\r\noutput-2\r\noutput-3');
		terminal.scrollToTop();
		Reflect.set(terminal, 'canvas', {
			getBoundingClientRect: () => ({ left: 0, top: 0 }),
		});
		const createInteractionContext = Reflect.get(terminal, 'createInteractionContext') as (
			x: number,
			y: number,
			modifiers: { alt: boolean; control: boolean; meta: boolean; shift: boolean },
		) => { semanticZone: string | null; cell: { bufferRow: number } | null };

		const context = createInteractionContext.call(terminal, 4, 8, {
			alt: false,
			control: false,
			meta: false,
			shift: false,
		});

		expect(context.cell?.bufferRow).toBe(0);
		expect(context.semanticZone).toBe('prompt');
		Reflect.set(terminal, 'canvas', undefined);
		terminal.dispose();
	});

	it('excludes a hovered link from context requested over another buffer cell', () => {
		const terminal = createTerminal(40, 3);
		terminal.write('https://example.com/context\r\nPLAIN');
		Reflect.set(terminal, 'canvas', {
			getBoundingClientRect: () => ({ left: 0, top: 0 }),
		});
		Reflect.set(terminal, 'hoveredLink', {
			text: 'https://example.com/context',
			target: { kind: 'url', url: 'https://example.com/context' },
			range: { start: { x: 0, y: 0 }, end: { x: 26, y: 0 } },
		});
		const createInteractionContext = Reflect.get(terminal, 'createInteractionContext') as (
			x: number,
			y: number,
			modifiers: { alt: boolean; control: boolean; meta: boolean; shift: boolean },
		) => { link: { text: string } | null };
		const modifiers = { alt: false, control: false, meta: false, shift: false };

		expect(createInteractionContext.call(terminal, 4, 8, modifiers).link?.text).toBe(
			'https://example.com/context',
		);
		expect(createInteractionContext.call(terminal, 4, 24, modifiers).link).toBeNull();
		Reflect.set(terminal, 'hoveredLink', undefined);
		Reflect.set(terminal, 'canvas', undefined);
		terminal.dispose();
	});

	it('does not activate a hovered link from another clicked buffer cell', () => {
		const openTarget = vi.fn<NonNullable<TerminalHost['openTarget']>>(async () => {});
		const terminal = new Terminal({ cols: 40, rows: 3, host: { openTarget } });
		terminal.write('https://example.com/click\r\nPLAIN');
		Reflect.set(terminal, 'canvas', {
			getBoundingClientRect: () => ({ left: 0, top: 0 }),
		});
		Reflect.set(terminal, 'hoveredLink', {
			text: 'https://example.com/click',
			target: { kind: 'url', url: 'https://example.com/click' },
			activation: 'modifier',
			range: { start: { x: 0, y: 0 }, end: { x: 24, y: 0 } },
		});
		const handleCanvasClick = Reflect.get(terminal, 'handleCanvasClick') as (
			event: MouseEvent,
		) => void;
		const event = {
			clientX: 4,
			clientY: 24,
			altKey: false,
			ctrlKey: false,
			metaKey: true,
			shiftKey: false,
		} as MouseEvent;

		handleCanvasClick.call(terminal, event);
		expect(openTarget).not.toHaveBeenCalled();
		handleCanvasClick.call(terminal, { ...event, clientY: 8 });
		expect(openTarget).toHaveBeenCalledOnce();
		Reflect.set(terminal, 'hoveredLink', undefined);
		Reflect.set(terminal, 'canvas', undefined);
		terminal.dispose();
	});

	it('does not activate a direct link from the click synthesized after a selection drag', () => {
		const openTarget = vi.fn<NonNullable<TerminalHost['openTarget']>>(async () => {});
		const terminal = new Terminal({ cols: 40, rows: 3, host: { openTarget } });
		const consumeSelectionDragClick = vi.fn<(event: MouseEvent) => boolean>(() => true);
		Reflect.set(terminal, 'selectionManager', { consumeSelectionDragClick });
		Reflect.set(terminal, 'canvas', {
			getBoundingClientRect: () => ({ left: 0, top: 0 }),
		});
		Reflect.set(terminal, 'hoveredLink', {
			text: 'direct link',
			target: { kind: 'url', url: 'https://example.com/direct' },
			activation: 'direct',
			range: { start: { x: 0, y: 0 }, end: { x: 20, y: 0 } },
		});
		const handleCanvasClick = Reflect.get(terminal, 'handleCanvasClick') as (
			event: MouseEvent,
		) => void;
		const event = {
			clientX: 4,
			clientY: 8,
			timeStamp: 100,
			altKey: false,
			ctrlKey: false,
			metaKey: false,
			shiftKey: false,
		} as MouseEvent;

		handleCanvasClick.call(terminal, event);

		expect(consumeSelectionDragClick).toHaveBeenCalledWith(event);
		expect(openTarget).not.toHaveBeenCalled();
		Reflect.set(terminal, 'hoveredLink', undefined);
		Reflect.set(terminal, 'canvas', undefined);
		Reflect.set(terminal, 'selectionManager', undefined);
		terminal.dispose();
	});

	it('keeps an unmodified context-menu gesture application-owned during mouse tracking', () => {
		const showContextMenu = vi.fn<NonNullable<TerminalHost['showContextMenu']>>(async () => null);
		const terminal = new Terminal({ cols: 40, rows: 3, host: { showContextMenu } });
		terminal.write('\x1b[?1000h\x1b[?1006h');
		const preventDefault = vi.fn<() => void>();
		const stopPropagation = vi.fn<() => void>();
		const handleCanvasContextMenu = Reflect.get(terminal, 'handleCanvasContextMenu') as (
			event: MouseEvent,
		) => void;

		handleCanvasContextMenu.call(terminal, {
			shiftKey: false,
			preventDefault,
			stopPropagation,
		} as unknown as MouseEvent);

		expect(preventDefault).toHaveBeenCalledOnce();
		expect(stopPropagation).toHaveBeenCalledOnce();
		expect(showContextMenu).not.toHaveBeenCalled();
		terminal.dispose();
	});

	it('returns local wheel and selection ownership when a tracked terminal becomes read-only', () => {
		const terminal = new Terminal({
			cols: 20,
			rows: 3,
			scrollback: 10_000,
			smoothScrollDuration: 0,
		});
		terminal.write(createLines('history', 20));
		terminal.write('\x1b[?1000h\x1b[?1006h');
		const createSelectionTerminal = Reflect.get(terminal, 'createSelectionTerminal') as () => {
			shouldSelect(event: MouseEvent): boolean;
		};
		const selectionTerminal = createSelectionTerminal.call(terminal);
		const handleWheel = Reflect.get(terminal, 'handleWheel') as (event: WheelEvent) => void;
		const preventDefault = vi.fn<() => void>();
		const wheelEvent = {
			deltaMode: 0,
			deltaX: 0,
			deltaY: -20,
			preventDefault,
		} as unknown as WheelEvent;

		expect(selectionTerminal.shouldSelect({ shiftKey: false } as MouseEvent)).toBe(false);
		handleWheel.call(terminal, wheelEvent);
		expect(terminal.viewportY).toBe(0);
		handleWheel.call(terminal, { ...wheelEvent, shiftKey: true });
		expect(terminal.viewportY).toBe(1);
		terminal.scrollToBottom();

		terminal.options.disableStdin = true;
		expect(selectionTerminal.shouldSelect({ shiftKey: false } as MouseEvent)).toBe(true);
		handleWheel.call(terminal, wheelEvent);

		expect(preventDefault).toHaveBeenCalledTimes(2);
		expect(terminal.viewportY).toBe(1);
		terminal.dispose();
	});

	it('returns the context menu to a read-only tracked terminal surface', async () => {
		const showContextMenu = vi.fn<NonNullable<TerminalHost['showContextMenu']>>(async () => null);
		const terminal = new Terminal({ cols: 40, rows: 3, host: { showContextMenu } });
		terminal.write('\x1b[?1000h\x1b[?1006h');
		terminal.options.disableStdin = true;
		Reflect.set(terminal, 'canvas', {
			getBoundingClientRect: () => ({ left: 0, top: 0 }),
		});
		const handleCanvasContextMenu = Reflect.get(terminal, 'handleCanvasContextMenu') as (
			event: MouseEvent,
		) => void;
		const preventDefault = vi.fn<() => void>();
		const stopPropagation = vi.fn<() => void>();

		handleCanvasContextMenu.call(terminal, {
			clientX: 1,
			clientY: 1,
			altKey: false,
			ctrlKey: false,
			metaKey: false,
			shiftKey: false,
			preventDefault,
			stopPropagation,
		} as unknown as MouseEvent);

		await vi.waitFor(() => expect(showContextMenu).toHaveBeenCalledOnce());
		expect(preventDefault).toHaveBeenCalledOnce();
		expect(stopPropagation).toHaveBeenCalledOnce();
		Reflect.set(terminal, 'canvas', undefined);
		terminal.dispose();
	});

	it('keeps unshifted link clicks application-owned during mouse tracking', () => {
		const openTarget = vi.fn<NonNullable<TerminalHost['openTarget']>>(async () => {});
		const terminal = new Terminal({ cols: 40, rows: 3, host: { openTarget } });
		terminal.write('https://example.com/tracked\x1b[?1000h\x1b[?1006h');
		Reflect.set(terminal, 'canvas', {
			getBoundingClientRect: () => ({ left: 0, top: 0 }),
			style: {},
		});
		const link = {
			text: 'https://example.com/tracked',
			target: { kind: 'url', url: 'https://example.com/tracked' },
			activation: 'modifier',
			range: { start: { x: 0, y: 0 }, end: { x: 26, y: 0 } },
		};
		Reflect.set(terminal, 'hoveredLink', link);
		const handleCanvasClick = Reflect.get(terminal, 'handleCanvasClick') as (
			event: MouseEvent,
		) => void;
		const event = {
			clientX: 4,
			clientY: 8,
			altKey: false,
			ctrlKey: false,
			metaKey: true,
			shiftKey: false,
		} as MouseEvent;

		handleCanvasClick.call(terminal, event);
		expect(openTarget).not.toHaveBeenCalled();
		Reflect.set(terminal, 'hoveredLink', link);
		handleCanvasClick.call(terminal, { ...event, shiftKey: true });
		expect(openTarget).toHaveBeenCalledOnce();
		Reflect.set(terminal, 'hoveredLink', undefined);
		Reflect.set(terminal, 'canvas', undefined);
		terminal.dispose();
	});
});
