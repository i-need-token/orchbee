import { afterEach, describe, expect, it, vi } from 'vitest';
import { FitAddon } from '../fit';
import type { ITerminalCore } from '../../types';

afterEach(() => vi.unstubAllGlobals());

describe('FitAddon', () => {
	it('calculates dimensions from renderer metrics and container size', () => {
		vi.stubGlobal('window', {
			getComputedStyle: () => ({ getPropertyValue: () => '0' }),
		});
		const element = { clientWidth: 815, clientHeight: 340 } as HTMLElement;
		const terminal = {
			cols: 80,
			rows: 24,
			element,
			renderer: { getMetrics: () => ({ width: 8, height: 17 }) },
		} as ITerminalCore & { renderer: { getMetrics(): { width: number; height: number } } };
		const addon = new FitAddon();
		addon.activate(terminal);
		expect(addon.proposeDimensions()).toEqual({ cols: 101, rows: 20 });
		addon.dispose();
	});

	it('resizes only when proposed dimensions change', () => {
		vi.stubGlobal('window', {
			getComputedStyle: () => ({ getPropertyValue: () => '0' }),
		});
		const resize = vi.fn<(cols: number, rows: number) => void>();
		const terminal = {
			cols: 80,
			rows: 24,
			element: { clientWidth: 655, clientHeight: 340 } as HTMLElement,
			renderer: { getMetrics: () => ({ width: 8, height: 17 }) },
			resize,
		} as ITerminalCore & {
			renderer: { getMetrics(): { width: number; height: number } };
			resize(cols: number, rows: number): void;
		};
		const addon = new FitAddon();
		addon.activate(terminal);
		addon.fit();
		expect(resize).toHaveBeenCalledWith(81, 20);
		addon.dispose();
	});

	it('refits after an external resize while the container is unchanged', () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', {
			getComputedStyle: () => ({ getPropertyValue: () => '0' }),
		});
		const resize = vi.fn<(cols: number, rows: number) => void>();
		const terminal = {
			cols: 80,
			rows: 24,
			element: { clientWidth: 815, clientHeight: 340 } as HTMLElement,
			renderer: { getMetrics: () => ({ width: 8, height: 17 }) },
			resize(cols: number, rows: number): void {
				this.cols = cols;
				this.rows = rows;
				resize(cols, rows);
			},
		};
		const addon = new FitAddon();

		try {
			addon.activate(terminal);
			addon.fit();
			vi.advanceTimersByTime(50);
			terminal.cols = 40;
			terminal.rows = 10;
			addon.fit();

			expect(resize.mock.calls).toEqual([
				[101, 20],
				[101, 20],
			]);
		} finally {
			addon.dispose();
			vi.useRealTimers();
		}
	});

	it('queues a container resize observed during the fit reentry window', () => {
		vi.useFakeTimers();
		vi.stubGlobal('window', {
			getComputedStyle: () => ({ getPropertyValue: () => '0' }),
		});
		let notifyResize: ResizeObserverCallback | undefined;
		const observe = vi.fn<() => void>();
		const disconnect = vi.fn<() => void>();
		vi.stubGlobal(
			'ResizeObserver',
			class {
				constructor(callback: ResizeObserverCallback) {
					notifyResize = callback;
				}
				observe = observe;
				disconnect = disconnect;
			},
		);
		const resize = vi.fn<(cols: number, rows: number) => void>();
		const element = { clientWidth: 815, clientHeight: 340 } as HTMLElement;
		const terminal = {
			cols: 80,
			rows: 24,
			element,
			renderer: { getMetrics: () => ({ width: 8, height: 17 }) },
			resize(cols: number, rows: number): void {
				this.cols = cols;
				this.rows = rows;
				resize(cols, rows);
			},
		};
		const addon = new FitAddon();

		try {
			addon.activate(terminal);
			addon.fit();
			addon.observeResize();
			Object.defineProperty(element, 'clientWidth', { configurable: true, value: 600 });
			notifyResize?.([], {} as ResizeObserver);
			vi.advanceTimersByTime(100);

			expect(resize.mock.calls).toEqual([
				[101, 20],
				[75, 20],
			]);
		} finally {
			addon.dispose();
			vi.useRealTimers();
		}
	});

	it('rounds fractional grid dimensions down to complete cells', () => {
		vi.stubGlobal('window', {
			getComputedStyle: () => ({ getPropertyValue: () => '0' }),
		});
		const terminal = {
			cols: 80,
			rows: 24,
			element: { clientWidth: 35, clientHeight: 40 } as HTMLElement,
			renderer: { getMetrics: () => ({ width: 6, height: 15 }) },
		} as ITerminalCore & { renderer: { getMetrics(): { width: number; height: number } } };
		const addon = new FitAddon();
		addon.activate(terminal);

		expect(addon.proposeDimensions()).toEqual({ cols: 5, rows: 2 });
		addon.dispose();
	});

	it('returns no dimensions for a zero-sized content area', () => {
		vi.stubGlobal('window', {
			getComputedStyle: () => ({ getPropertyValue: () => '0' }),
		});
		const terminal = {
			cols: 80,
			rows: 24,
			element: { clientWidth: 0, clientHeight: 0 } as HTMLElement,
			renderer: { getMetrics: () => ({ width: 8, height: 17 }) },
		} as ITerminalCore & { renderer: { getMetrics(): { width: number; height: number } } };
		const addon = new FitAddon();
		addon.activate(terminal);

		expect(addon.proposeDimensions()).toBeUndefined();
		addon.dispose();
	});
});
