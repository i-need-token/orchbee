/**
 * FitAddon — auto-resize terminal to fit container.
 *
 * Compatible with xterm.js FitAddon API.
 */

import type { ITerminalAddon, ITerminalCore } from '../types';

const MINIMUM_COLS = 2;
const MINIMUM_ROWS = 1;
const RESIZE_DEBOUNCE_MS = 100;

type FitTerminal = ITerminalCore & {
	renderer?: { getMetrics(): { width: number; height: number } };
	resize?(cols: number, rows: number): void;
};

export class FitAddon implements ITerminalAddon {
	private terminal?: FitTerminal | undefined;
	private resizeObserver?: ResizeObserver | undefined;
	private resizeDebounceTimer?: ReturnType<typeof setTimeout> | undefined;
	private isResizing = false;

	activate(terminal: ITerminalCore): void {
		this.terminal = terminal;
	}

	dispose(): void {
		if (this.resizeObserver) {
			this.resizeObserver.disconnect();
			this.resizeObserver = undefined;
		}
		if (this.resizeDebounceTimer) {
			clearTimeout(this.resizeDebounceTimer);
			this.resizeDebounceTimer = undefined;
		}
		this.terminal = undefined;
	}

	fit(): void {
		if (this.isResizing) return;

		const dims = this.proposeDimensions();
		if (!dims || !this.terminal) return;

		const currentCols = this.terminal.cols;
		const currentRows = this.terminal.rows;

		if (dims.cols === currentCols && dims.rows === currentRows) {
			return;
		}

		this.isResizing = true;

		try {
			this.terminal.resize?.(dims.cols, dims.rows);
		} finally {
			setTimeout(() => {
				this.isResizing = false;
			}, 50);
		}
	}

	proposeDimensions(): { cols: number; rows: number } | undefined {
		if (!this.terminal?.element) return undefined;

		const renderer = this.terminal.renderer;
		if (!renderer) return undefined;

		const metrics = renderer.getMetrics();
		if (!metrics || metrics.width === 0 || metrics.height === 0) return undefined;

		const terminalElement = this.terminal.element;
		if (typeof terminalElement.clientWidth === 'undefined') return undefined;

		const style = window.getComputedStyle(terminalElement);
		const paddingTop = Number.parseInt(style.getPropertyValue('padding-top')) || 0;
		const paddingBottom = Number.parseInt(style.getPropertyValue('padding-bottom')) || 0;
		const paddingLeft = Number.parseInt(style.getPropertyValue('padding-left')) || 0;
		const paddingRight = Number.parseInt(style.getPropertyValue('padding-right')) || 0;

		const availableWidth = terminalElement.clientWidth - paddingLeft - paddingRight;
		const availableHeight = terminalElement.clientHeight - paddingTop - paddingBottom;

		if (availableWidth <= 0 || availableHeight <= 0) return undefined;

		return {
			cols: Math.max(MINIMUM_COLS, Math.floor(availableWidth / metrics.width)),
			rows: Math.max(MINIMUM_ROWS, Math.floor(availableHeight / metrics.height)),
		};
	}

	observeResize(): void {
		if (!this.terminal?.element || this.resizeObserver) return;

		this.resizeObserver = new ResizeObserver(() => {
			if (this.resizeDebounceTimer) clearTimeout(this.resizeDebounceTimer);
			this.resizeDebounceTimer = setTimeout(() => this.fit(), RESIZE_DEBOUNCE_MS);
		});

		this.resizeObserver.observe(this.terminal.element);
	}
}
