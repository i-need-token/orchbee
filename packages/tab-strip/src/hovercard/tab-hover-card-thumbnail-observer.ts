import type { ThumbnailCaptureReadiness } from './tab-hover-card-controller';

export interface HoverCardThumbnailSize {
	width: number;
	height: number;
}

export interface HoverCardThumbnailSubscription<Image> {
	setSizeHint(size: Readonly<HoverCardThumbnailSize>): void;
	setImageCallback(callback: (image: Image) => void): void;
	dispose(): void;
}

/** Browser/Electron adapter corresponding to Chromium's ThumbnailImage. */
export interface HoverCardThumbnailSource<Image> {
	hasData(): boolean;
	getCaptureReadiness(): ThumbnailCaptureReadiness;
	subscribe(): HoverCardThumbnailSubscription<Image> | null;
	requestThumbnailImage(): void;
}

export type HoverCardThumbnailObserverCallback<Image> = (
	observer: TabHoverCardThumbnailObserver<Image>,
	image: Image,
) => void;

export const HOVER_CARD_PREVIEW_SIZE: Readonly<HoverCardThumbnailSize> = Object.freeze({
	width: 320,
	height: 180,
});

/** Direct port of TabHoverCardThumbnailObserver's source/subscription lifecycle. */
export class TabHoverCardThumbnailObserver<Image> {
	private current: HoverCardThumbnailSource<Image> | null = null;
	private subscription: HoverCardThumbnailSubscription<Image> | null = null;
	private readonly callbacks = new Set<HoverCardThumbnailObserverCallback<Image>>();

	public get currentImage(): HoverCardThumbnailSource<Image> | null {
		return this.current;
	}

	public observe(source: HoverCardThumbnailSource<Image> | null): void {
		if (this.current === source) return;
		this.subscription?.dispose();
		this.subscription = null;
		this.current = source;
		if (!source) return;

		const subscription = source.subscribe();
		if (!subscription || this.current !== source) {
			subscription?.dispose();
			return;
		}
		this.subscription = subscription;
		subscription.setSizeHint(HOVER_CARD_PREVIEW_SIZE);
		subscription.setImageCallback((image) => {
			if (this.current !== source) return;
			this.callbacks.forEach((callback) => callback(this, image));
		});
		source.requestThumbnailImage();
	}

	public addCallback(callback: HoverCardThumbnailObserverCallback<Image>): () => void {
		this.callbacks.add(callback);
		return () => this.callbacks.delete(callback);
	}

	public destroy(): void {
		this.observe(null);
		this.callbacks.clear();
	}
}
