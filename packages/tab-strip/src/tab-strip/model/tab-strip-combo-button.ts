import { TabStripOrientation } from '../types/tab-strip-types';

export const TAB_STRIP_COMBO_ANIMATION_DURATION_MS = 300;
export const TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS = 2_000;

type TabStripComboButtonId = 'tab-search';

interface TabStripComboAnimationFactors {
	expansion: number;
	opacity: number;
	radius: number;
}

export interface TabStripComboButtonSnapshot {
	revision: number;
	orientation: TabStripOrientation;
	tabSearchPinned: boolean;
	tabSearchEphemeral: boolean;
	tabSearchVisible: boolean;
	tabSearchTargetVisible: boolean;
	tabSearchAnimating: boolean;
	bubbleOpen: boolean;
	menuOpenFor: TabStripComboButtonId | null;
	keepCurrentStateLock: boolean;
	hideTabSearchPending: boolean;
}

interface TabStripComboButtonControllerOptions {
	tabSearchPinned?: boolean;
	richAnimations?: boolean;
	onPinnedChange?: (button: TabStripComboButtonId, pinned: boolean) => void;
	onUserAction?: (action: string) => void;
	onTabSearchBubbleShown?: () => void;
}

type Listener = () => void;

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

/** Exact three-stage factor mapping from TabStripComboButton::AnimationProgressed. */
export function getTabStripComboAnimationFactors(value: number): TabStripComboAnimationFactors {
	return {
		expansion: clamp01(value * 2),
		opacity: clamp01((value - 0.25) * 2),
		radius: clamp01((value - 0.5) * 2),
	};
}

/** Preference, ephemeral-search, menu-lock, and visibility controller port. */
export class TabStripComboButtonController {
	private readonly options: TabStripComboButtonControllerOptions;
	private readonly listeners = new Set<Listener>();
	private readonly richAnimations: boolean;
	private tabSearchPinned: boolean;
	private tabSearchEphemeral = false;
	private orientation: TabStripOrientation = TabStripOrientation.Horizontal;
	private tabSearchVisible: boolean;
	private tabSearchTargetVisible: boolean;
	private tabSearchAnimating = false;
	private bubbleOpen = false;
	private menuOpenFor: TabStripComboButtonId | null = null;
	private keepCurrentStateLock = false;
	private hideTimer: ReturnType<typeof setTimeout> | null = null;
	private animationTimer: ReturnType<typeof setTimeout> | null = null;
	private revision = 0;
	private snapshot: TabStripComboButtonSnapshot;

	public constructor(options: TabStripComboButtonControllerOptions = {}) {
		this.options = options;
		this.richAnimations = options.richAnimations ?? true;
		this.tabSearchPinned = options.tabSearchPinned ?? false;
		this.tabSearchTargetVisible = this.tabSearchPinned;
		this.tabSearchVisible = this.tabSearchTargetVisible;
		this.snapshot = this.createSnapshot();
	}

	public getSnapshot = (): TabStripComboButtonSnapshot => this.snapshot;

	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public setOrientation(orientation: TabStripOrientation): void {
		if (this.orientation === orientation) return;
		this.orientation = orientation;
		this.commit();
	}

	public setPinned(button: TabStripComboButtonId, pinned: boolean): void {
		if (this.tabSearchPinned === pinned) return;
		this.tabSearchPinned = pinned;
		this.updateButtonsVisibility();
		this.options.onPinnedChange?.(button, pinned);
	}

	public executeTogglePinned(button: TabStripComboButtonId): void {
		this.tabSearchEphemeral = false;
		this.clearHideTimer();
		const pinned = !this.tabSearchPinned;
		this.options.onUserAction?.(`TabStripComboButton.TabSearch.${pinned ? 'Pinned' : 'Unpinned'}`);
		this.setPinned(button, pinned);
	}

	public onBubbleInitializing(): void {
		this.clearHideTimer();
		this.bubbleOpen = true;
		this.keepCurrentStateLock = true;
		this.options.onTabSearchBubbleShown?.();
		if (!this.tabSearchPinned) {
			this.tabSearchEphemeral = true;
			this.updateButtonsVisibility();
		} else {
			this.commit();
		}
	}

	public onBubbleDestroying(): void {
		this.bubbleOpen = false;
		this.keepCurrentStateLock = false;
		if (this.tabSearchPinned) {
			this.commit();
			return;
		}
		this.scheduleTabSearchHide();
		this.commit();
	}

	public onMenuOpened(button: TabStripComboButtonId): void {
		this.menuOpenFor = button;
		this.keepCurrentStateLock = true;
		this.commit();
	}

	public onMenuClosed(): void {
		this.menuOpenFor = null;
		this.keepCurrentStateLock = false;
		if (this.tabSearchEphemeral && !this.tabSearchPinned) {
			this.scheduleTabSearchHide();
		}
		this.commit();
	}

	public isIphEligible(tabSearchUsed: boolean): boolean {
		return this.tabSearchPinned && tabSearchUsed;
	}

	public destroy(): void {
		this.clearHideTimer();
		this.clearAnimationTimer();
		this.listeners.clear();
	}

	private updateButtonsVisibility(): void {
		this.transitionButton(this.tabSearchPinned || this.tabSearchEphemeral);
		this.commit();
	}

	private transitionButton(targetVisible: boolean): void {
		if (this.tabSearchTargetVisible === targetVisible) return;
		this.tabSearchTargetVisible = targetVisible;
		this.clearAnimationTimer();

		if (targetVisible) {
			this.tabSearchVisible = true;
			this.tabSearchAnimating = this.richAnimations;
		} else {
			this.tabSearchAnimating = this.richAnimations && this.tabSearchVisible;
		}

		if (!this.richAnimations) {
			this.tabSearchVisible = targetVisible;
			return;
		}
		this.animationTimer = setTimeout(() => {
			this.animationTimer = null;
			this.tabSearchAnimating = false;
			this.tabSearchVisible = this.tabSearchTargetVisible;
			this.commit();
		}, TAB_STRIP_COMBO_ANIMATION_DURATION_MS);
	}

	private clearHideTimer(): void {
		if (this.hideTimer !== null) clearTimeout(this.hideTimer);
		this.hideTimer = null;
	}

	private scheduleTabSearchHide(): void {
		this.clearHideTimer();
		this.hideTimer = setTimeout(() => {
			this.hideTimer = null;
			if (this.tabSearchPinned || this.menuOpenFor !== null) {
				this.commit();
				return;
			}
			this.tabSearchEphemeral = false;
			this.updateButtonsVisibility();
		}, TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS);
	}

	private clearAnimationTimer(): void {
		if (this.animationTimer !== null) clearTimeout(this.animationTimer);
		this.animationTimer = null;
	}

	private commit(): void {
		this.revision += 1;
		this.snapshot = this.createSnapshot();
		this.listeners.forEach((listener) => listener());
	}

	private createSnapshot(): TabStripComboButtonSnapshot {
		return Object.freeze({
			revision: this.revision,
			orientation: this.orientation,
			tabSearchPinned: this.tabSearchPinned,
			tabSearchEphemeral: this.tabSearchEphemeral,
			tabSearchVisible: this.tabSearchVisible,
			tabSearchTargetVisible: this.tabSearchTargetVisible,
			tabSearchAnimating: this.tabSearchAnimating,
			bubbleOpen: this.bubbleOpen,
			menuOpenFor: this.menuOpenFor,
			keepCurrentStateLock: this.keepCurrentStateLock,
			hideTabSearchPending: this.hideTimer !== null,
		});
	}
}
