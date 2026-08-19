const HISTORY_SCROLL_DURATION_MILLISECONDS = 100;
const HISTORY_SCROLL_SETTLEMENT_ROWS = 0.01;

export interface HistoryScrollPresentationRequest {
	readonly fromTop: bigint;
	readonly reducedMotion: boolean;
	readonly rowHeight: number;
	readonly toTop: bigint;
	readonly visibleRows: number;
}

interface PreparedHistoryScrollPresentation {
	readonly displacementPixels: number;
	readonly displacementRows: number;
	readonly originalTransform: string;
	readonly originalTransformOrigin: string;
	readonly originalWillChange: string;
	readonly snapshot: HTMLCanvasElement;
	readonly view: Window;
}

interface ActiveHistoryScrollPresentation extends PreparedHistoryScrollPresentation {
	startedAt: number | undefined;
}

/** Owns one reversible DOM projection between two committed Host history windows. */
export class HistoryScrollPresentationTransaction {
	readonly #canvas: HTMLCanvasElement;
	readonly #root: HTMLElement;
	#active: ActiveHistoryScrollPresentation | undefined;
	#animationFrame: number | undefined;
	#disposed = false;
	#prepared: PreparedHistoryScrollPresentation | undefined;

	constructor(root: HTMLElement, canvas: HTMLCanvasElement) {
		this.#root = root;
		this.#canvas = canvas;
	}

	prepare(request: HistoryScrollPresentationRequest): boolean {
		this.cancel();
		if (
			this.#disposed ||
			request.reducedMotion ||
			!Number.isFinite(request.rowHeight) ||
			request.rowHeight <= 0 ||
			!Number.isInteger(request.visibleRows) ||
			request.visibleRows < 1 ||
			request.fromTop === request.toTop
		) {
			return false;
		}
		const displacement = request.toTop - request.fromTop;
		const maximumDisplacement = BigInt(request.visibleRows);
		if (displacement < -maximumDisplacement || displacement > maximumDisplacement) return false;
		const view = this.#root.ownerDocument.defaultView;
		if (
			view === null ||
			typeof view.requestAnimationFrame !== 'function' ||
			typeof view.cancelAnimationFrame !== 'function' ||
			this.#canvas.width < 1 ||
			this.#canvas.height < 1
		) {
			return false;
		}
		const snapshot = this.#root.ownerDocument.createElement('canvas');
		snapshot.width = this.#canvas.width;
		snapshot.height = this.#canvas.height;
		const context = snapshot.getContext('2d', { alpha: true });
		if (context === null) return false;
		try {
			context.drawImage(this.#canvas, 0, 0);
		} catch {
			return false;
		}
		const canvasBounds = this.#canvas.getBoundingClientRect();
		const rootBounds = this.#root.getBoundingClientRect();
		snapshot.dataset['projectedTerminalHistoryMotion'] = 'true';
		snapshot.setAttribute('aria-hidden', 'true');
		snapshot.style.contain = 'strict';
		snapshot.style.height = `${canvasBounds.height}px`;
		snapshot.style.left = `${canvasBounds.left - rootBounds.left}px`;
		snapshot.style.pointerEvents = 'none';
		snapshot.style.position = 'absolute';
		snapshot.style.top = `${canvasBounds.top - rootBounds.top}px`;
		snapshot.style.transformOrigin = 'top left';
		snapshot.style.width = `${canvasBounds.width}px`;
		snapshot.style.zIndex = '0';
		this.#root.appendChild(snapshot);
		this.#prepared = {
			displacementPixels: Number(displacement) * request.rowHeight,
			displacementRows: Number(displacement),
			originalTransform: this.#canvas.style.transform,
			originalTransformOrigin: this.#canvas.style.transformOrigin,
			originalWillChange: this.#canvas.style.willChange,
			snapshot,
			view,
		};
		return true;
	}

	commit(): boolean {
		const prepared = this.#prepared;
		this.#prepared = undefined;
		if (prepared === undefined || this.#disposed) {
			prepared?.snapshot.remove();
			return false;
		}
		this.#active = { ...prepared, startedAt: undefined };
		this.#canvas.style.transform = translateY(prepared.displacementPixels);
		this.#canvas.style.transformOrigin = 'top left';
		this.#canvas.style.willChange = 'transform';
		prepared.snapshot.style.transform = translateY(0);
		prepared.snapshot.style.willChange = 'transform';
		this.#animationFrame = prepared.view.requestAnimationFrame(this.#animate);
		return true;
	}

	cancel(): void {
		this.#finish();
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#finish();
		this.#disposed = true;
	}

	readonly #animate = (timestamp: number): void => {
		this.#animationFrame = undefined;
		const active = this.#active;
		if (active === undefined || this.#disposed) return;
		active.startedAt ??= timestamp;
		const elapsed = Math.max(0, timestamp - active.startedAt);
		const remaining = Math.exp((-5 * elapsed) / HISTORY_SCROLL_DURATION_MILLISECONDS);
		if (Math.abs(active.displacementRows * remaining) < HISTORY_SCROLL_SETTLEMENT_ROWS) {
			this.#finish();
			return;
		}
		this.#canvas.style.transform = translateY(active.displacementPixels * remaining);
		active.snapshot.style.transform = translateY(-active.displacementPixels * (1 - remaining));
		this.#animationFrame = active.view.requestAnimationFrame(this.#animate);
	};

	#finish(): void {
		const presentation = this.#active ?? this.#prepared;
		if (presentation === undefined) return;
		if (this.#animationFrame !== undefined) {
			presentation.view.cancelAnimationFrame(this.#animationFrame);
			this.#animationFrame = undefined;
		}
		this.#canvas.style.transform = presentation.originalTransform;
		this.#canvas.style.transformOrigin = presentation.originalTransformOrigin;
		this.#canvas.style.willChange = presentation.originalWillChange;
		presentation.snapshot.remove();
		this.#active = undefined;
		this.#prepared = undefined;
	}
}

function translateY(offset: number): string {
	return `translateY(${offset}px)`;
}
