import {
	getThumbnailPlan,
	ThumbnailCaptureReadiness,
	ThumbnailWaitState,
	type ThumbnailPresentation,
} from './tab-hover-card-controller';
import {
	TabHoverCardThumbnailObserver,
	type HoverCardThumbnailSource,
} from './tab-hover-card-thumbnail-observer';

export const HOVER_CARD_THUMBNAIL_TRANSITION_MS = 200;
const HOVER_CARD_PREVIEW_CROSSFADE_START = 0.25;

export interface HoverCardThumbnailVisual<Image> {
	presentation: ThumbnailPresentation;
	image: Image | null;
}

export interface HoverCardThumbnailTarget<Image> {
	id: string;
	source: HoverCardThumbnailSource<Image> | null;
	previewsEnabled: boolean;
	needsThumbnail: boolean;
	isGroup?: boolean;
	discarded?: boolean;
	crashed?: boolean;
	isInitialShow: boolean;
	useAnimations?: boolean;
	crossfadeStart?: number | null;
}

export interface HoverCardThumbnailSnapshot<Image> {
	revision: number;
	targetId: string | null;
	current: Readonly<HoverCardThumbnailVisual<Image>>;
	fadingOut: Readonly<HoverCardThumbnailVisual<Image>> | null;
	imageTransitioning: boolean;
	waitState: ThumbnailWaitState;
	waitingForCapture: boolean;
}

type Listener = () => void;

const NONE_VISUAL: Readonly<HoverCardThumbnailVisual<never>> = Object.freeze({
	presentation: 'none',
	image: null,
});

function visual<Image>(
	presentation: ThumbnailPresentation,
	image: Image | null = null,
): HoverCardThumbnailVisual<Image> {
	return { presentation, image };
}

/** Host-independent runtime for Chromium's delayed observation and preview crossfade. */
export class TabHoverCardThumbnailRuntime<Image> {
	private readonly observer = new TabHoverCardThumbnailObserver<Image>();
	private readonly removeObserverCallback: () => void;
	private readonly listeners = new Set<Listener>();
	private target: HoverCardThumbnailTarget<Image> | null = null;
	private captureTimer: ReturnType<typeof setTimeout> | null = null;
	private placeholderTimer: ReturnType<typeof setTimeout> | null = null;
	private fadeTimer: ReturnType<typeof setTimeout> | null = null;
	private fadeStartedAt = 0;
	private current: HoverCardThumbnailVisual<Image> = NONE_VISUAL;
	private fadingOut: HoverCardThumbnailVisual<Image> | null = null;
	private waitState: ThumbnailWaitState = ThumbnailWaitState.NotWaiting;
	private revision = 0;
	private snapshot: HoverCardThumbnailSnapshot<Image> = this.createSnapshot();

	public constructor() {
		this.removeObserverCallback = this.observer.addCallback((_observer, image) =>
			this.onImageAvailable(image),
		);
	}

	public getSnapshot = (): HoverCardThumbnailSnapshot<Image> => this.snapshot;

	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public updateTarget(target: Readonly<HoverCardThumbnailTarget<Image>> | null): void {
		if (
			target &&
			this.target &&
			this.target.id === target.id &&
			this.target.source === target.source &&
			this.target.previewsEnabled === target.previewsEnabled &&
			this.target.needsThumbnail === target.needsThumbnail &&
			(this.target.isGroup ?? false) === (target.isGroup ?? false) &&
			(this.target.discarded ?? false) === (target.discarded ?? false) &&
			(this.target.crashed ?? false) === (target.crashed ?? false) &&
			(this.target.useAnimations ?? true) === (target.useAnimations ?? true) &&
			(this.target.crossfadeStart === undefined
				? HOVER_CARD_PREVIEW_CROSSFADE_START
				: this.target.crossfadeStart) ===
				(target.crossfadeStart === undefined
					? HOVER_CARD_PREVIEW_CROSSFADE_START
					: target.crossfadeStart)
		)
			return;
		const sourceChanged = this.target?.source !== target?.source;
		const targetChanged = this.target?.id !== target?.id;
		if (sourceChanged || targetChanged) this.clearPendingCaptureAndPlaceholder();
		this.target = target ? { ...target } : null;

		if (!this.target) {
			this.observer.observe(null);
			this.waitState = ThumbnailWaitState.NotWaiting;
			this.setVisual(visual('none'), false);
			this.commit();
			return;
		}

		const source = this.target.source;
		const plan = getThumbnailPlan({
			previewsEnabled: this.target.previewsEnabled,
			needsThumbnail: this.target.needsThumbnail,
			isGroup: this.target.isGroup ?? false,
			discarded: this.target.discarded ?? false,
			crashed: this.target.crashed ?? false,
			hasThumbnailObject: source !== null,
			hasThumbnailData: source?.hasData() ?? false,
			readiness: source?.getCaptureReadiness() ?? ThumbnailCaptureReadiness.NotReady,
			isInitialShow: this.target.isInitialShow,
			useAnimations: this.target.useAnimations ?? true,
			crossfadeStart:
				this.target.crossfadeStart === undefined
					? HOVER_CARD_PREVIEW_CROSSFADE_START
					: this.target.crossfadeStart,
		});
		this.waitState = plan.waitState;

		if (plan.presentation === 'none' && plan.waitState === ThumbnailWaitState.NotWaiting) {
			this.observer.observe(null);
			this.setVisual(visual('none'));
			this.commit();
			return;
		}
		if (plan.presentation === 'crashed' || plan.presentation === 'placeholder') {
			this.setVisual(visual(plan.presentation));
		}
		if (!source) {
			this.observer.observe(null);
			this.commit();
			return;
		}

		if (plan.captureDelayMs !== null && plan.captureDelayMs > 0) {
			this.observer.observe(null);
			const expectedTargetId = this.target.id;
			this.captureTimer = setTimeout(() => {
				this.captureTimer = null;
				if (this.target?.id === expectedTargetId && this.target.source === source)
					this.observer.observe(source);
				this.commit();
			}, plan.captureDelayMs);
			this.commit();
			return;
		}

		if (plan.shouldObserve) {
			if (plan.waitState === ThumbnailWaitState.WaitingWithPlaceholder) {
				this.setVisual(visual('placeholder'));
			} else if (!this.target.isInitialShow && targetChanged) {
				this.scheduleTransitionPlaceholder(
					this.target.crossfadeStart === undefined
						? HOVER_CARD_PREVIEW_CROSSFADE_START
						: this.target.crossfadeStart,
				);
			}
			this.observer.observe(source);
		}
		this.commit();
	}

	public destroy(): void {
		this.clearPendingCaptureAndPlaceholder();
		this.clearFade();
		this.removeObserverCallback();
		this.observer.destroy();
		this.listeners.clear();
	}

	private scheduleTransitionPlaceholder(crossfadeStart: number | null): void {
		const progress = crossfadeStart === null ? 1 : Math.min(1, Math.max(0, crossfadeStart));
		const expectedTargetId = this.target?.id;
		this.placeholderTimer = setTimeout(() => {
			this.placeholderTimer = null;
			if (
				this.target?.id !== expectedTargetId ||
				this.waitState !== ThumbnailWaitState.WaitingWithoutPlaceholder
			)
				return;
			this.waitState = ThumbnailWaitState.WaitingWithPlaceholder;
			this.setVisual(visual('placeholder'));
			this.commit();
		}, progress * HOVER_CARD_THUMBNAIL_TRANSITION_MS);
	}

	private onImageAvailable(image: Image): void {
		this.clearPendingCaptureAndPlaceholder();
		this.waitState = ThumbnailWaitState.NotWaiting;
		this.setVisual(visual('thumbnail', image));
		this.commit();
	}

	private setVisual(next: HoverCardThumbnailVisual<Image>, animate = true): void {
		if (this.current.presentation === next.presentation && this.current.image === next.image)
			return;
		const animationsEnabled = animate && (this.target?.useAnimations ?? true);
		if (!animationsEnabled || this.current.presentation === 'none') {
			this.clearFade();
			this.current = next;
			return;
		}

		const now = Date.now();
		if (this.fadingOut && now - this.fadeStartedAt <= HOVER_CARD_THUMBNAIL_TRANSITION_MS / 2) {
			this.current = next;
			return;
		}
		this.clearFade();
		this.fadingOut = this.current;
		this.current = next;
		this.fadeStartedAt = now;
		this.fadeTimer = setTimeout(() => {
			this.fadeTimer = null;
			this.fadingOut = null;
			this.commit();
		}, HOVER_CARD_THUMBNAIL_TRANSITION_MS);
	}

	private clearPendingCaptureAndPlaceholder(): void {
		if (this.captureTimer !== null) clearTimeout(this.captureTimer);
		if (this.placeholderTimer !== null) clearTimeout(this.placeholderTimer);
		this.captureTimer = null;
		this.placeholderTimer = null;
	}

	private clearFade(): void {
		if (this.fadeTimer !== null) clearTimeout(this.fadeTimer);
		this.fadeTimer = null;
		this.fadingOut = null;
	}

	private commit(): void {
		this.revision += 1;
		this.snapshot = this.createSnapshot();
		this.listeners.forEach((listener) => listener());
	}

	private createSnapshot(): HoverCardThumbnailSnapshot<Image> {
		return Object.freeze({
			revision: this.revision,
			targetId: this.target?.id ?? null,
			current: Object.freeze({ ...this.current }),
			fadingOut: this.fadingOut ? Object.freeze({ ...this.fadingOut }) : null,
			imageTransitioning: this.fadingOut !== null,
			waitState: this.waitState,
			waitingForCapture: this.captureTimer !== null,
		});
	}
}
