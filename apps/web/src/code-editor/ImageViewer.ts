import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';

/** A zoom factor, `1` being the natural size, or `'fit'`: scaled down to fit the viewer. */
export type ImageViewerScale = number | 'fit';

export interface ImageViewerViewState {
	readonly scale: ImageViewerScale;
	readonly scrollLeft: number;
	readonly scrollTop: number;
}

/** Only the larger dimension: the other follows from the aspect ratio. */
interface ImageSize {
	readonly height?: number;
	readonly width?: number;
}

/** Images scaled by more than this show their pixels rather than a smoothed blur. */
const PIXELATION_THRESHOLD = 3;
const SCALE_PINCH_FACTOR = 0.075;
const MAXIMUM_SCALE = 20;
const MINIMUM_SCALE = 0.1;
const ZOOM_LEVELS: readonly number[] = [
	0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1, 1.5, 2, 3, 5, 7, 10, 15, 20,
];

function nextZoomLevel(scale: number): number {
	return ZOOM_LEVELS.find((level) => level > scale) ?? MAXIMUM_SCALE;
}

function previousZoomLevel(scale: number): number {
	let previous = MINIMUM_SCALE;
	for (const level of ZOOM_LEVELS) {
		if (level >= scale) break;
		previous = level;
	}
	return previous;
}

function largerDimension(width: number, height: number): ImageSize {
	return width >= height ? { width } : { height };
}

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(Math.max(value, minimum), maximum);
}

/**
 * VS Code's image viewer over two elements React renders: a scroll container and its image. It
 * fits the image by default; a click zooms in through fixed levels and a modified click zooms out
 * (Ctrl, or Option on Apple systems), as do the modified wheel and a pinch, keeping the visible
 * center in place. State shows through attributes for the stylesheet: `data-ready`, `data-zoomed` and
 * `data-zoom-out` on the container, `data-fit` and `data-pixelated` on the image.
 */
export class ImageViewer {
	private readonly element: HTMLElement;
	private hasImage = false;
	private readonly image: HTMLImageElement;
	/** The image's size when it was last fitted, for an image without a natural size. */
	private lastFitSize: ImageSize | undefined;
	private readonly listeners = new AbortController();
	/** An image without an intrinsic width, such as an SVG with only a viewBox, would stretch. */
	private needsExplicitSize = false;
	private requestId = 0;
	private scale: ImageViewerScale = 'fit';
	private readonly zoomOutModifier: 'altKey' | 'ctrlKey';

	public constructor(
		element: HTMLElement,
		image: HTMLImageElement,
		platform: KeyboardShortcutPlatform,
	) {
		this.element = element;
		this.image = image;
		this.zoomOutModifier = platform === 'apple' ? 'altKey' : 'ctrlKey';
		this.applyScale('fit');
		this.listen();
	}

	/** Forgets a pending source, so it does not replace the shown image when it decodes. */
	public cancelPendingSource(): void {
		this.requestId += 1;
	}

	public dispose(): void {
		this.cancelPendingSource();
		this.listeners.abort();
	}

	/** The shown image's view state, or `undefined` without one. */
	public getViewState(): ImageViewerViewState | undefined {
		if (!this.hasImage) return undefined;
		return {
			scale: this.scale,
			scrollLeft: this.element.scrollLeft,
			scrollTop: this.element.scrollTop,
		};
	}

	/**
	 * Shows the image at `source`, decoded before it replaces the shown one so switching does not
	 * flash, and restores `viewState` or fits it. Resolves whether it decoded and still shows; a
	 * newer source supersedes it.
	 */
	public async setSource(
		source: string,
		viewState: ImageViewerViewState | undefined,
	): Promise<boolean> {
		this.requestId += 1;
		const requestId = this.requestId;
		const loader = this.element.ownerDocument.createElement('img');
		loader.src = source;
		let decoded = true;
		try {
			await loader.decode();
		} catch {
			decoded = false;
		}
		if (requestId !== this.requestId) return false;

		this.image.src = source;
		this.hasImage = decoded;
		this.element.toggleAttribute('data-ready', decoded);
		if (!decoded) {
			this.applyScale('fit');
			return false;
		}
		this.needsExplicitSize = !this.hasIntrinsicWidth(loader);
		this.lastFitSize = undefined;
		this.applyScale('fit');
		if (viewState !== undefined && viewState.scale !== 'fit') {
			this.applyScale(viewState.scale);
			this.element.scrollTo(viewState.scrollLeft, viewState.scrollTop);
		}
		return true;
	}

	public zoomIn(): void {
		if (this.hasImage) this.applyScale(nextZoomLevel(this.numericScale()));
	}

	public zoomOut(): void {
		if (this.hasImage) this.applyScale(previousZoomLevel(this.numericScale()));
	}

	private listen(): void {
		const options = { signal: this.listeners.signal };
		this.element.addEventListener(
			'wheel',
			(event) => {
				// A pinch arrives as a wheel with Ctrl, which must never zoom the whole window.
				if (event.ctrlKey) event.preventDefault();
				if (!this.hasImage || !(event[this.zoomOutModifier] || event.ctrlKey)) return;
				event.preventDefault();
				if (event.deltaY === 0) return;
				const delta = event.deltaY > 0 ? 1 : -1;
				this.applyScale(this.numericScale() * (1 - delta * SCALE_PINCH_FACTOR));
			},
			{ passive: false, signal: this.listeners.signal },
		);

		// Focus moves before the click, so the click reads what the press saw.
		let consumeClick = false;
		let zoomOutOnClick = false;
		this.element.addEventListener(
			'mousedown',
			(event) => {
				if (event.button !== 0) return;
				consumeClick = !this.element.contains(this.element.ownerDocument.activeElement);
				zoomOutOnClick = event[this.zoomOutModifier];
			},
			options,
		);
		this.element.addEventListener(
			'click',
			(event) => {
				if (event.button !== 0 || !this.hasImage) return;
				// A click that only focuses the viewer does not zoom.
				if (consumeClick) {
					consumeClick = false;
					return;
				}
				if (zoomOutOnClick) this.zoomOut();
				else this.zoomIn();
			},
			options,
		);

		const showZoomOutCursor = (event: KeyboardEvent | MouseEvent): void => {
			this.element.toggleAttribute('data-zoom-out', event[this.zoomOutModifier]);
		};
		this.element.addEventListener('keydown', showZoomOutCursor, options);
		this.element.addEventListener('keyup', showZoomOutCursor, options);
		this.element.addEventListener('mousemove', showZoomOutCursor, options);
	}

	/** The scale as a number: for a fitted image, the factor it shows at. */
	private numericScale(): number {
		if (this.scale !== 'fit') return this.scale;
		const size = this.needsExplicitSize ? this.baseSize() : { width: this.image.naturalWidth };
		if (size?.width) return this.image.clientWidth / size.width;
		if (size?.height) return this.image.clientHeight / size.height;
		return 1;
	}

	/** Fits the image, or zooms it to a factor while keeping the visible center in place. */
	private applyScale(next: ImageViewerScale): void {
		const container = this.element;
		const image = this.image;
		if (next === 'fit') {
			this.scale = 'fit';
			image.toggleAttribute('data-fit', true);
			image.toggleAttribute('data-pixelated', false);
			image.style.removeProperty('zoom');
			image.style.removeProperty('width');
			image.style.removeProperty('height');
			// Dropping the overflow before scrolling avoids a costly synchronous scroll update.
			const wasZoomed = container.hasAttribute('data-zoomed');
			container.toggleAttribute('data-zoomed', false);
			if (wasZoomed) container.scrollTo(0, 0);
			return;
		}
		if (!this.hasImage) return;

		const scale = clamp(next, MINIMUM_SCALE, MAXIMUM_SCALE);
		this.scale = scale;
		const centerX =
			container.scrollWidth > 0
				? (container.scrollLeft + container.clientWidth / 2) / container.scrollWidth
				: 0.5;
		const centerY =
			container.scrollHeight > 0
				? (container.scrollTop + container.clientHeight / 2) / container.scrollHeight
				: 0.5;
		// Read while still fitted: the fallback size is the fitted one.
		const size = this.needsExplicitSize ? this.baseSize() : undefined;

		image.toggleAttribute('data-fit', false);
		image.toggleAttribute('data-pixelated', scale >= PIXELATION_THRESHOLD);
		image.style.width = size?.width ? `${size.width}px` : '';
		image.style.height = size?.height ? `${size.height}px` : '';
		image.style.setProperty('zoom', String(scale));
		container.toggleAttribute('data-zoomed', true);
		// Setting the zoom lays out synchronously, so the scroll size already reflects it.
		container.scrollTo(
			container.scrollWidth * centerX - container.clientWidth / 2,
			container.scrollHeight * centerY - container.clientHeight / 2,
		);
	}

	/** The image's size at a zoom of 1: natural sizes round, so only the larger dimension. */
	private baseSize(): ImageSize | undefined {
		const image = this.image;
		if (this.scale === 'fit' && (image.clientWidth || image.clientHeight)) {
			this.lastFitSize = largerDimension(image.clientWidth, image.clientHeight);
		}
		if (image.naturalWidth || image.naturalHeight) {
			return largerDimension(image.naturalWidth, image.naturalHeight);
		}
		return this.lastFitSize;
	}

	/** Whether the image keeps its natural width inside a container much wider than it. */
	private hasIntrinsicWidth(image: HTMLImageElement): boolean {
		const document = this.element.ownerDocument;
		const probe = document.createElement('div');
		probe.style.position = 'absolute';
		probe.style.visibility = 'hidden';
		probe.style.width = '100000px';
		probe.append(image);
		document.body.append(probe);
		const intrinsic = Math.abs(image.clientWidth - image.naturalWidth) <= 1;
		probe.remove();
		return intrinsic;
	}
}
