const HOVER_CARD_MINIMUM_TRIGGER_DELAY_MS = 300;
const HOVER_CARD_MAXIMUM_TRIGGER_DELAY_MS = 800;
const HOVER_CARD_MAX_WIDTH_ADDITIONAL_DELAY_MS = 500;
const HOVER_CARD_IMMEDIATE_RESHOW_BUFFER_MS = 300;
const HOVER_CARD_SLIDE_DURATION_MS = 200;

export const HoverCardUpdateType = {
	SelectionChanged: 'selection-changed',
	Hover: 'hover',
	TabDataChanged: 'tab-data-changed',
	TabRemoved: 'tab-removed',
	Animating: 'animating',
	Event: 'event',
	Focus: 'focus',
} as const;

export type HoverCardUpdateType = (typeof HoverCardUpdateType)[keyof typeof HoverCardUpdateType];

interface HoverCardTarget {
	id: string;
	width: number;
	minimumWidth: number;
	standardWidth: number;
	valid?: boolean;
	focused?: boolean;
}

export interface TabHoverCardSnapshot {
	revision: number;
	targetId: string | null;
	visible: boolean;
	waitingToShow: boolean;
	transitioning: boolean;
	hideLockCount: number;
	cardsSeen: number;
	showDelayMs: number | null;
}

type Listener = () => void;

/** Port of TabHoverCardController's logarithmic GetShowDelay helper. */
function getHoverCardShowDelay(
	tabWidth: number,
	minimumWidth: number,
	standardWidth: number,
	additionalMaxWidthDelayMs = HOVER_CARD_MAX_WIDTH_ADDITIONAL_DELAY_MS,
): number {
	if (tabWidth <= minimumWidth || standardWidth <= minimumWidth) {
		return HOVER_CARD_MINIMUM_TRIGGER_DELAY_MS;
	}
	const logarithmicFraction =
		Math.log(tabWidth - minimumWidth + 1) / Math.log(standardWidth - minimumWidth + 1);
	let delay =
		logarithmicFraction *
			(HOVER_CARD_MAXIMUM_TRIGGER_DELAY_MS - HOVER_CARD_MINIMUM_TRIGGER_DELAY_MS) +
		HOVER_CARD_MINIMUM_TRIGGER_DELAY_MS;
	if (tabWidth >= standardWidth) delay += additionalMaxWidthDelayMs;
	return Math.round(delay);
}

class ScopedHideHoverCardLock {
	private disposed = false;
	private controller: TabHoverCardController | null;

	public constructor(controller: TabHoverCardController | null) {
		this.controller = controller;
		this.controller?.registerHideLock();
	}

	public dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.controller?.unregisterHideLock();
		this.controller = null;
	}
}

/** DOM-independent port of the hover-card target/timer/lock state machine. */
export class TabHoverCardController {
	private target: HoverCardTarget | null = null;
	private visible = false;
	private showTimer: ReturnType<typeof setTimeout> | null = null;
	private transitionTimer: ReturnType<typeof setTimeout> | null = null;
	private transitioning = false;
	private hideLockCount = 0;
	private lastMouseExitTimestamp: number | null = null;
	private lastSeenTargetId: string | null = null;
	private cardsSeen = 0;
	private showDelayMs: number | null = null;
	private revision = 0;
	private snapshot: TabHoverCardSnapshot = this.createSnapshot();
	private readonly listeners = new Set<Listener>();

	public getSnapshot = (): TabHoverCardSnapshot => this.snapshot;

	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public updateHoverCard(
		target: Readonly<HoverCardTarget> | null,
		updateType: HoverCardUpdateType,
		timestamp = Date.now(),
	): void {
		let nextTarget = target && target.valid !== false ? { ...target } : null;
		let nextUpdateType = updateType;
		if (this.hideLockCount > 0) {
			nextTarget = null;
			nextUpdateType = HoverCardUpdateType.Animating;
		}

		const targetChanged = this.target?.id !== nextTarget?.id;
		if (targetChanged) {
			this.clearShowTimer();
			if (nextTarget && this.target) this.startTransition();
			this.target = nextTarget;
		} else if (nextTarget) {
			this.target = nextTarget;
		}

		if (nextUpdateType === HoverCardUpdateType.SelectionChanged) this.resetCardsSeen();
		if (nextUpdateType === HoverCardUpdateType.Hover && !nextTarget) {
			this.lastMouseExitTimestamp = timestamp;
		}

		if (!nextTarget) {
			this.hide();
			return;
		}

		if (nextUpdateType === HoverCardUpdateType.TabDataChanged && !this.visible) {
			this.commit();
			return;
		}

		if (this.visible) {
			this.recordCardSeen(nextTarget.id);
			this.showDelayMs = null;
			this.commit();
			return;
		}

		const initial = !this.shouldShowImmediately(nextTarget, timestamp);
		if (!initial) {
			this.showNow(nextTarget.id);
			return;
		}

		this.resetCardsSeen();
		const delay = getHoverCardShowDelay(
			nextTarget.width,
			nextTarget.minimumWidth,
			nextTarget.standardWidth,
		);
		this.showDelayMs = delay;
		this.showTimer = setTimeout(() => {
			this.showTimer = null;
			if (this.target?.id === nextTarget.id && this.hideLockCount === 0)
				this.showNow(nextTarget.id);
			else this.commit();
		}, delay);
		this.commit();
	}

	public preventImmediateReshow(): void {
		this.lastMouseExitTimestamp = null;
	}

	public getHideLock(): ScopedHideHoverCardLock {
		return new ScopedHideHoverCardLock(this);
	}

	public destroy(): void {
		this.clearShowTimer();
		this.clearTransitionTimer();
		this.listeners.clear();
	}

	public registerHideLock(): void {
		this.hideLockCount += 1;
		this.clearShowTimer();
		this.hide();
	}

	public unregisterHideLock(): void {
		if (this.hideLockCount === 0) throw new Error('Hover-card hide lock count underflow.');
		this.hideLockCount -= 1;
		this.commit();
	}

	private shouldShowImmediately(target: Readonly<HoverCardTarget>, timestamp: number): boolean {
		const withinReshowBuffer =
			this.lastMouseExitTimestamp !== null &&
			timestamp - this.lastMouseExitTimestamp <= HOVER_CARD_IMMEDIATE_RESHOW_BUFFER_MS;
		return withinReshowBuffer || target.focused === true;
	}

	private showNow(targetId: string): void {
		if (this.target?.id !== targetId || this.hideLockCount > 0) return;
		this.clearShowTimer();
		this.visible = true;
		this.showDelayMs = null;
		this.recordCardSeen(targetId);
		this.commit();
	}

	private hide(): void {
		this.clearShowTimer();
		this.clearTransitionTimer();
		this.visible = false;
		this.transitioning = false;
		this.showDelayMs = null;
		this.lastSeenTargetId = null;
		this.commit();
	}

	private startTransition(): void {
		this.clearTransitionTimer();
		if (!this.visible) return;
		this.transitioning = true;
		this.transitionTimer = setTimeout(() => {
			this.transitionTimer = null;
			this.transitioning = false;
			this.commit();
		}, HOVER_CARD_SLIDE_DURATION_MS);
	}

	private recordCardSeen(targetId: string): void {
		if (this.lastSeenTargetId === targetId) return;
		this.lastSeenTargetId = targetId;
		this.cardsSeen += 1;
	}

	private resetCardsSeen(): void {
		this.lastSeenTargetId = null;
		this.cardsSeen = 0;
	}

	private clearShowTimer(): void {
		if (this.showTimer !== null) clearTimeout(this.showTimer);
		this.showTimer = null;
	}

	private clearTransitionTimer(): void {
		if (this.transitionTimer !== null) clearTimeout(this.transitionTimer);
		this.transitionTimer = null;
	}

	private commit(): void {
		const next = this.createSnapshot();
		if (
			next.targetId === this.snapshot.targetId &&
			next.visible === this.snapshot.visible &&
			next.waitingToShow === this.snapshot.waitingToShow &&
			next.transitioning === this.snapshot.transitioning &&
			next.hideLockCount === this.snapshot.hideLockCount &&
			next.cardsSeen === this.snapshot.cardsSeen &&
			next.showDelayMs === this.snapshot.showDelayMs
		)
			return;
		this.revision += 1;
		this.snapshot = Object.freeze({ ...next, revision: this.revision });
		this.listeners.forEach((listener) => listener());
	}

	private createSnapshot(): TabHoverCardSnapshot {
		return Object.freeze({
			revision: this.revision,
			targetId: this.target?.id ?? null,
			visible: this.visible,
			waitingToShow: this.showTimer !== null,
			transitioning: this.transitioning,
			hideLockCount: this.hideLockCount,
			cardsSeen: this.cardsSeen,
			showDelayMs: this.showDelayMs,
		});
	}
}

export const ThumbnailCaptureReadiness = {
	NotReady: 'not-ready',
	ReadyForInitialCapture: 'ready-for-initial-capture',
	ReadyForFinalCapture: 'ready-for-final-capture',
} as const;

export type ThumbnailCaptureReadiness =
	(typeof ThumbnailCaptureReadiness)[keyof typeof ThumbnailCaptureReadiness];

export const ThumbnailWaitState = {
	NotWaiting: 'not-waiting',
	WaitingWithPlaceholder: 'waiting-with-placeholder',
	WaitingWithoutPlaceholder: 'waiting-without-placeholder',
} as const;

export type ThumbnailWaitState = (typeof ThumbnailWaitState)[keyof typeof ThumbnailWaitState];

export type ThumbnailPresentation = 'none' | 'placeholder' | 'crashed' | 'thumbnail';

interface ThumbnailPlanInput {
	previewsEnabled: boolean;
	needsThumbnail: boolean;
	isGroup: boolean;
	discarded: boolean;
	crashed: boolean;
	hasThumbnailObject: boolean;
	hasThumbnailData: boolean;
	readiness: ThumbnailCaptureReadiness;
	isInitialShow: boolean;
	useAnimations: boolean;
	crossfadeStart: number | null;
}

interface ThumbnailPlan {
	presentation: ThumbnailPresentation;
	waitState: ThumbnailWaitState;
	captureDelayMs: number | null;
	shouldObserve: boolean;
}

function getThumbnailCaptureDelay(readiness: ThumbnailCaptureReadiness): number {
	if (readiness === ThumbnailCaptureReadiness.NotReady) return 800;
	return 300;
}

/** Pure port of MaybeStartThumbnailObservation's observable decisions. */
export function getThumbnailPlan(input: Readonly<ThumbnailPlanInput>): ThumbnailPlan {
	const none: ThumbnailPlan = {
		presentation: 'none',
		waitState: ThumbnailWaitState.NotWaiting,
		captureDelayMs: null,
		shouldObserve: false,
	};
	if (!input.previewsEnabled || !input.needsThumbnail || input.isGroup) return none;
	if (input.discarded && !input.hasThumbnailData) return none;
	if (input.crashed) return { ...none, presentation: 'crashed' };
	if (!input.hasThumbnailObject) return { ...none, presentation: 'placeholder' };

	const delay =
		input.isInitialShow || input.hasThumbnailData ? 0 : getThumbnailCaptureDelay(input.readiness);
	if (delay === 0) {
		return {
			presentation: input.hasThumbnailData ? 'thumbnail' : 'none',
			waitState:
				input.useAnimations && input.crossfadeStart === 0
					? ThumbnailWaitState.WaitingWithPlaceholder
					: ThumbnailWaitState.WaitingWithoutPlaceholder,
			captureDelayMs: 0,
			shouldObserve: true,
		};
	}
	return {
		presentation: 'placeholder',
		waitState: ThumbnailWaitState.WaitingWithPlaceholder,
		captureDelayMs: delay,
		shouldObserve: false,
	};
}
