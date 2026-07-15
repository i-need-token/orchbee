import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	SelectionManager,
	type ISelectionTerminal,
	type SelectionCell,
	type SelectionModel,
} from '../selection-manager';
import type { TerminalClipboardHost } from '../../host';

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

function cells(text: string): SelectionCell[] {
	return [...text].map((character) => ({
		codepoint: character.codePointAt(0) ?? 0,
		width: 1,
		grapheme_len: 1,
	}));
}

function createSelectionModel(text: string): SelectionModel {
	let range: ReturnType<SelectionModel['getRange']> = null;
	return {
		hasSelection: () => range !== null,
		getSelection: () => (range ? text : ''),
		getRange: () => range,
		clearSelection: () => {
			const changed = range !== null;
			range = null;
			return changed;
		},
		setRange: (start, end, rectangle) => {
			range = { start, end, ...(rectangle === undefined ? {} : { rectangle }) };
			return true;
		},
		selectWordAt: () => false,
		selectLineAt: () => false,
		selectAll: () => false,
		pressGesture: () => false,
		dragGesture: () => false,
		releaseGesture: () => true,
		getAutoscroll: () => null,
		resetGesture: () => {},
	};
}

describe('SelectionManager', () => {
	it('selects and clears text through the programmatic API', () => {
		const documentTarget = {
			addEventListener: () => {},
			removeEventListener: () => {},
		};
		vi.stubGlobal('document', documentTarget);
		const canvas = {
			addEventListener: () => {},
			removeEventListener: () => {},
			contains: () => false,
		} as unknown as HTMLCanvasElement;
		const lines = [cells('hello'), cells('world')];
		const requestRender = vi.fn<() => void>();
		const terminal: ISelectionTerminal = {
			getCellDimensions: () => ({ width: 8, height: 17 }),
			getDimensions: () => ({ cols: 5, rows: 2 }),
			getCanvasOffset: () => ({ left: 0, top: 0 }),
			getScrollbackLength: () => 0,
			getViewportY: () => 0,
			getLine: (row) => lines[row] ?? null,
			getScrollbackLine: () => null,
			getGraphemeString: (row, column) =>
				String.fromCodePoint(lines[row]?.[column]?.codepoint ?? 0),
			scrollLines: () => {},
			requestRender,
		};
		const manager = new SelectionManager(terminal, canvas, {
			selectionModel: createSelectionModel('ell'),
		});
		manager.select(1, 0, 3);
		expect(manager.hasSelection()).toBe(true);
		expect(manager.getSelection()).toBe('ell');
		expect(manager.getSelectionPosition()).toEqual({
			start: { x: 1, y: 0 },
			end: { x: 3, y: 0 },
		});
		manager.clearSelection();
		expect(manager.hasSelection()).toBe(false);
		expect(requestRender).toHaveBeenCalled();
		requestRender.mockClear();
		manager.clearSelection();
		expect(requestRender).not.toHaveBeenCalled();
		manager.dispose();
	});

	it('writes selections through the injected clipboard capability', async () => {
		vi.stubGlobal('document', {
			addEventListener: () => {},
			removeEventListener: () => {},
		});
		const canvas = {
			addEventListener: () => {},
			removeEventListener: () => {},
			contains: () => false,
		} as unknown as HTMLCanvasElement;
		const line = cells('hello');
		const write = vi.fn<TerminalClipboardHost['write']>(async () => {});
		const manager = new SelectionManager(
			{
				getCellDimensions: () => ({ width: 8, height: 17 }),
				getDimensions: () => ({ cols: 5, rows: 1 }),
				getCanvasOffset: () => ({ left: 0, top: 0 }),
				getScrollbackLength: () => 0,
				getViewportY: () => 0,
				getLine: () => line,
				getScrollbackLine: () => null,
				getGraphemeString: (_row, column) => String.fromCodePoint(line[column]?.codepoint ?? 0),
				scrollLines: () => {},
				requestRender: () => {},
			},
			canvas,
			{
				selectionModel: createSelectionModel('ell'),
				clipboard: {
					read: async () => null,
					write,
				},
			},
		);
		manager.select(1, 0, 3);

		await expect(manager.copySelectionAsync()).resolves.toEqual({ kind: 'written' });
		expect(write).toHaveBeenCalledWith({
			target: 'standard',
			source: 'copy',
			payload: { text: 'ell' },
		});
		manager.dispose();
	});

	it('reports clipboard failures without claiming a successful async copy', async () => {
		vi.stubGlobal('document', {
			addEventListener: () => {},
			removeEventListener: () => {},
		});
		const canvas = {
			addEventListener: () => {},
			removeEventListener: () => {},
			contains: () => false,
		} as unknown as HTMLCanvasElement;
		const line = cells('error');
		const error = new Error('permission denied');
		const onCopyError = vi.fn<(value: unknown) => void>();
		const manager = new SelectionManager(
			{
				getCellDimensions: () => ({ width: 8, height: 17 }),
				getDimensions: () => ({ cols: 5, rows: 1 }),
				getCanvasOffset: () => ({ left: 0, top: 0 }),
				getScrollbackLength: () => 0,
				getViewportY: () => 0,
				getLine: () => line,
				getScrollbackLine: () => null,
				getGraphemeString: (_row, column) => String.fromCodePoint(line[column]?.codepoint ?? 0),
				scrollLines: () => {},
				requestRender: () => {},
			},
			canvas,
			{
				selectionModel: createSelectionModel('error'),
				clipboard: {
					read: async () => null,
					write: async () => {
						throw error;
					},
				},
				onCopyError,
			},
		);
		manager.select(0, 0, 5);

		await expect(manager.copySelectionAsync()).resolves.toEqual({ kind: 'failed', error });
		expect(onCopyError).toHaveBeenCalledWith(error);
		manager.dispose();
	});

	it('delegates selection state and text to the terminal-owned model', () => {
		vi.stubGlobal('document', {
			addEventListener: () => {},
			removeEventListener: () => {},
		});
		const canvas = {
			addEventListener: () => {},
			removeEventListener: () => {},
			contains: () => false,
		} as unknown as HTMLCanvasElement;
		let range: ReturnType<SelectionModel['getRange']> = null;
		const model: SelectionModel = {
			hasSelection: () => range !== null,
			getSelection: () => (range ? 'tracked' : ''),
			getRange: () => range,
			clearSelection: () => {
				const changed = range !== null;
				range = null;
				return changed;
			},
			setRange: (start, end) => {
				range = { start, end };
				return true;
			},
			selectWordAt: () => false,
			selectLineAt: () => false,
			selectAll: () => {
				range = { start: { x: 0, y: 0 }, end: { x: 4, y: 1 } };
				return true;
			},
			selectOutputAt: (column, row) => {
				range = { start: { x: column, y: row }, end: { x: 4, y: row } };
				return true;
			},
			adjustSelection: (adjustment) => {
				if (!range || adjustment !== 'left') return false;
				range = { start: range.start, end: { x: range.end.x - 1, y: range.end.y } };
				return true;
			},
			pressGesture: () => false,
			dragGesture: () => false,
			releaseGesture: () => true,
			getAutoscroll: () => null,
			resetGesture: () => {},
		};
		const requestRender = vi.fn<() => void>();
		let scrollbackLength = 0;
		let viewportY = 0;
		const manager = new SelectionManager(
			{
				getCellDimensions: () => ({ width: 8, height: 17 }),
				getDimensions: () => ({ cols: 5, rows: 2 }),
				getCanvasOffset: () => ({ left: 0, top: 0 }),
				getScrollbackLength: () => scrollbackLength,
				getViewportY: () => viewportY,
				getLine: () => null,
				getScrollbackLine: () => null,
				scrollLines: () => {},
				requestRender,
			},
			canvas,
			{ selectionModel: model },
		);

		manager.select(1, 0, 3);
		expect(manager.getSelection()).toBe('tracked');
		expect(model.getRange()).toEqual({
			start: { x: 1, y: 0 },
			end: { x: 3, y: 0 },
		});
		manager.selectAll();
		expect(manager.getSelectionPosition()).toEqual({
			start: { x: 0, y: 0 },
			end: { x: 4, y: 1 },
		});
		expect(manager.selectOutputAt(2, 1)).toBe(true);
		expect(manager.getSelectionPosition()).toEqual({
			start: { x: 2, y: 1 },
			end: { x: 4, y: 1 },
		});
		expect(manager.adjustSelection('left')).toBe(true);
		expect(manager.getSelectionPosition()).toEqual({
			start: { x: 2, y: 1 },
			end: { x: 3, y: 1 },
		});
		expect(manager.selectOutputAtAbsolute(3, 4)).toBe(true);
		expect(manager.getSelectionPosition()).toEqual({
			start: { x: 3, y: 4 },
			end: { x: 4, y: 4 },
		});
		scrollbackLength = 10;
		viewportY = 3;
		expect(manager.getSelectionCoords()).toMatchObject({ startRow: -3, endRow: -3 });
		expect(manager.getSelectionPosition()).toEqual({
			start: { x: 3, y: 4 },
			end: { x: 4, y: 4 },
		});
		manager.clearSelection();
		expect(model.hasSelection()).toBe(false);
		expect(requestRender).toHaveBeenCalled();
		manager.dispose();
	});

	it('routes pointer gestures through the terminal-owned gesture state machine', () => {
		const documentListeners = new Map<string, EventListener>();
		const canvasListeners = new Map<string, EventListener>();
		vi.stubGlobal('document', {
			addEventListener: (name: string, listener: EventListener) =>
				documentListeners.set(name, listener),
			removeEventListener: (name: string) => documentListeners.delete(name),
		});
		const canvas = {
			addEventListener: (name: string, listener: EventListener) =>
				canvasListeners.set(name, listener),
			removeEventListener: (name: string) => canvasListeners.delete(name),
			contains: () => false,
			getBoundingClientRect: () => ({ left: 10, top: 20, right: 90, bottom: 54, height: 34 }),
		} as unknown as HTMLCanvasElement;
		let range: ReturnType<SelectionModel['getRange']> = null;
		const pressGesture = vi.fn<NonNullable<SelectionModel['pressGesture']>>(() => true);
		const dragGesture = vi.fn<NonNullable<SelectionModel['dragGesture']>>((input) => {
			range = { start: { x: 1, y: 4 }, end: { x: input.column, y: input.row } };
			return true;
		});
		const releaseGesture = vi.fn<NonNullable<SelectionModel['releaseGesture']>>(() => true);
		const model: SelectionModel = {
			hasSelection: () => range !== null,
			getSelection: () => (range ? 'gesture' : ''),
			getRange: () => range,
			clearSelection: () => false,
			setRange: () => false,
			selectWordAt: () => false,
			selectLineAt: () => false,
			selectAll: () => false,
			pressGesture,
			dragGesture,
			releaseGesture,
			getAutoscroll: () => null,
			resetGesture: () => {},
		};
		const manager = new SelectionManager(
			{
				getCellDimensions: () => ({ width: 8, height: 17 }),
				getDimensions: () => ({ cols: 10, rows: 2 }),
				getCanvasOffset: () => ({ left: 10, top: 20 }),
				getScrollbackLength: () => 5,
				getViewportY: () => 2,
				getLine: () => null,
				getScrollbackLine: () => null,
				scrollLines: () => {},
				requestRender: () => {},
			},
			canvas,
			{ selectionModel: model },
		);

		documentListeners.get('mousemove')?.({
			clientX: 42,
			clientY: 37,
			altKey: false,
			buttons: 1,
		} as unknown as MouseEvent);
		expect(dragGesture).not.toHaveBeenCalled();

		canvasListeners.get('mousedown')?.({
			button: 0,
			clientX: 18,
			clientY: 37,
			timeStamp: 100,
			metaKey: true,
		} as unknown as MouseEvent);
		documentListeners.get('mousemove')?.({
			clientX: 42,
			clientY: 37,
			altKey: false,
			buttons: 1,
		} as unknown as MouseEvent);
		documentListeners.get('mouseup')?.({
			clientX: 42,
			clientY: 37,
		} as unknown as MouseEvent);

		expect(pressGesture).toHaveBeenCalledWith(
			expect.objectContaining({
				column: 1,
				row: 4,
				surfaceX: 8,
				surfaceY: 17,
				selectOutput: true,
			}),
		);
		expect(dragGesture).toHaveBeenCalledWith(
			expect.objectContaining({ column: 4, row: 4, columns: 10, cellWidth: 8 }),
		);
		expect(releaseGesture).toHaveBeenCalledWith(4, 4);

		pressGesture.mockClear();
		dragGesture.mockClear();
		releaseGesture.mockClear();
		canvasListeners.get('mousedown')?.({
			button: 0,
			clientX: -100,
			clientY: -100,
			timeStamp: 200,
		} as unknown as MouseEvent);
		documentListeners.get('mousemove')?.({
			clientX: 1_000,
			clientY: 1_000,
			altKey: false,
			buttons: 1,
		} as unknown as MouseEvent);
		documentListeners.get('mouseup')?.({
			clientX: 1_000,
			clientY: 1_000,
		} as unknown as MouseEvent);

		expect(pressGesture).toHaveBeenCalledWith(expect.objectContaining({ column: 0, row: 3 }));
		expect(dragGesture).toHaveBeenCalledWith(expect.objectContaining({ column: 9, row: 4 }));
		expect(releaseGesture).toHaveBeenCalledWith(9, 4);
		manager.dispose();
	});

	it('does not consume a click after pointer jitter below Ghostty drag threshold', () => {
		const documentListeners = new Map<string, EventListener>();
		const canvasListeners = new Map<string, EventListener>();
		vi.stubGlobal('document', {
			addEventListener: (name: string, listener: EventListener) =>
				documentListeners.set(name, listener),
			removeEventListener: (name: string) => documentListeners.delete(name),
		});
		const canvas = {
			addEventListener: (name: string, listener: EventListener) =>
				canvasListeners.set(name, listener),
			removeEventListener: (name: string) => canvasListeners.delete(name),
			contains: () => true,
			getBoundingClientRect: () => ({ left: 0, top: 0, right: 80, bottom: 34, height: 34 }),
		} as unknown as HTMLCanvasElement;
		const model = createSelectionModel('');
		model.pressGesture = () => true;
		const isGestureDragged = vi.fn<NonNullable<SelectionModel['isGestureDragged']>>(() => false);
		model.isGestureDragged = isGestureDragged;
		const manager = new SelectionManager(
			{
				getCellDimensions: () => ({ width: 8, height: 17 }),
				getDimensions: () => ({ cols: 10, rows: 2 }),
				getCanvasOffset: () => ({ left: 0, top: 0 }),
				getScrollbackLength: () => 0,
				getViewportY: () => 0,
				scrollLines: () => {},
				requestRender: () => {},
			},
			canvas,
			{ selectionModel: model },
		);

		canvasListeners.get('mousedown')?.({
			button: 0,
			clientX: 10,
			clientY: 8,
			timeStamp: 100,
		} as unknown as MouseEvent);
		documentListeners.get('mousemove')?.({
			buttons: 1,
			clientX: 11,
			clientY: 8,
			altKey: false,
		} as unknown as MouseEvent);
		documentListeners.get('mouseup')?.({
			clientX: 11,
			clientY: 8,
			timeStamp: 110,
		} as unknown as MouseEvent);

		expect(isGestureDragged).toHaveBeenCalledOnce();
		expect(
			manager.consumeSelectionDragClick({
				clientX: 11,
				clientY: 8,
				timeStamp: 111,
			} as MouseEvent),
		).toBe(false);
		manager.dispose();
	});

	it('continues an edge drag in the externally scrolled viewport', () => {
		vi.useFakeTimers();
		class TestNode {
			readonly nodeType = 1;
		}
		vi.stubGlobal('Node', TestNode);
		const documentListeners = new Map<string, EventListener>();
		const canvasListeners = new Map<string, EventListener>();
		vi.stubGlobal('document', {
			addEventListener: (name: string, listener: EventListener) =>
				documentListeners.set(name, listener),
			removeEventListener: (name: string) => documentListeners.delete(name),
		});
		const canvas = {
			addEventListener: (name: string, listener: EventListener) =>
				canvasListeners.set(name, listener),
			removeEventListener: (name: string) => canvasListeners.delete(name),
			contains: () => false,
			getBoundingClientRect: () => ({ left: 10, top: 20, right: 90, bottom: 54, height: 34 }),
		} as unknown as HTMLCanvasElement;
		let viewportY = 2;
		const scrollLines = vi.fn<(amount: number) => void>((amount) => {
			viewportY -= amount;
		});
		let autoscroll: 'up' | 'down' | null = null;
		const model = createSelectionModel('dragged');
		model.pressGesture = () => true;
		const dragGesture = vi.fn<SelectionModel['dragGesture']>((input) => {
			model.setRange({ x: 0, y: 0 }, { x: input.column, y: input.row });
			if (input.surfaceY < 0) autoscroll = 'up';
			else if (input.surfaceY > 34) autoscroll = 'down';
			else autoscroll = null;
			return true;
		});
		model.dragGesture = dragGesture;
		model.isGestureDragged = () => model.hasSelection();
		model.getAutoscroll = () => autoscroll;
		const resetGesture = vi.fn<() => void>(() => {
			autoscroll = null;
		});
		model.resetGesture = resetGesture;
		model.releaseGesture = () => {
			autoscroll = null;
			return true;
		};
		const manager = new SelectionManager(
			{
				getCellDimensions: () => ({ width: 8, height: 17 }),
				getDimensions: () => ({ cols: 10, rows: 2 }),
				getCanvasOffset: () => ({ left: 10, top: 20 }),
				getScrollbackLength: () => 5,
				getViewportY: () => viewportY,
				scrollLines,
				requestRender: () => {},
			},
			canvas,
			{ selectionModel: model },
		);

		canvasListeners.get('mousedown')?.({
			button: 0,
			clientX: 18,
			clientY: 37,
			timeStamp: 100,
		} as unknown as MouseEvent);
		documentListeners.get('mousemove')?.({
			clientX: 42,
			clientY: 19,
			altKey: false,
			buttons: 1,
		} as unknown as MouseEvent);
		vi.advanceTimersByTime(16);

		expect(scrollLines).toHaveBeenCalledWith(-1);
		expect(viewportY).toBe(3);
		expect(dragGesture).toHaveBeenLastCalledWith(
			expect.objectContaining({ column: 4, row: 2, surfaceY: -1 }),
		);
		documentListeners.get('mousemove')?.({
			clientX: 42,
			clientY: 19,
			buttons: 0,
		} as unknown as MouseEvent);
		expect(resetGesture).toHaveBeenCalledOnce();
		scrollLines.mockClear();
		vi.advanceTimersByTime(32);
		expect(scrollLines).not.toHaveBeenCalled();

		dragGesture.mockClear();
		canvasListeners.get('mousedown')?.({
			button: 0,
			clientX: 18,
			clientY: 37,
			timeStamp: 200,
		} as unknown as MouseEvent);
		documentListeners.get('mousemove')?.({
			clientX: 42,
			clientY: 55,
			altKey: false,
			buttons: 1,
		} as unknown as MouseEvent);
		vi.advanceTimersByTime(16);

		expect(scrollLines).toHaveBeenCalledWith(1);
		expect(viewportY).toBe(2);
		expect(dragGesture).toHaveBeenLastCalledWith(
			expect.objectContaining({ column: 4, row: 4, surfaceY: 35 }),
		);
		const outside = new TestNode();
		documentListeners.get('mouseup')?.({
			clientX: 42,
			clientY: 55,
			timeStamp: 300,
			target: outside,
		} as unknown as MouseEvent);
		documentListeners.get('click')?.({
			clientX: 42,
			clientY: 55,
			timeStamp: 301,
			target: outside,
		} as unknown as MouseEvent);
		expect(manager.hasSelection()).toBe(true);

		documentListeners.get('click')?.({
			clientX: 60,
			clientY: 60,
			timeStamp: 1_500,
			target: outside,
		} as unknown as MouseEvent);
		expect(manager.hasSelection()).toBe(false);
		manager.dispose();
	});
});
