import { TabStripOrientation } from '../common';

export const TAB_STRIP_COMBO_ANIMATION_DURATION_MS = 300;
export const TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS = 2_000;

export const TabStripComboContext = {
	HorizontalTabStrip: 'horizontal-tab-strip',
	VerticalTabStrip: 'vertical-tab-strip',
} as const;

export type TabStripComboContext = (typeof TabStripComboContext)[keyof typeof TabStripComboContext];

export const TabStripComboStartKind = {
	Projects: 'projects',
	EverythingMenu: 'everything-menu',
} as const;

export type TabStripComboStartKind =
	(typeof TabStripComboStartKind)[keyof typeof TabStripComboStartKind];

export type TabStripComboButtonId = 'start' | 'tab-search';

export interface TabStripComboAnimationFactors {
	expansion: number;
	opacity: number;
	radius: number;
}

export interface TabStripComboButtonSnapshot {
	revision: number;
	orientation: TabStripOrientation;
	startKind: TabStripComboStartKind | null;
	startPinned: boolean;
	tabSearchPinned: boolean;
	tabSearchEphemeral: boolean;
	startVisible: boolean;
	tabSearchVisible: boolean;
	startTargetVisible: boolean;
	tabSearchTargetVisible: boolean;
	startAnimating: boolean;
	tabSearchAnimating: boolean;
	bubbleOpen: boolean;
	menuOpenFor: TabStripComboButtonId | null;
	keepCurrentStateLock: boolean;
	hideTabSearchPending: boolean;
}

export interface TabStripComboButtonControllerOptions {
	context?: TabStripComboContext;
	startKind?: TabStripComboStartKind;
	startPinned?: boolean;
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
	private readonly context: TabStripComboContext;
	private readonly configuredStartKind: TabStripComboStartKind;
	private readonly richAnimations: boolean;
	private startPinned: boolean;
	private tabSearchPinned: boolean;
	private tabSearchEphemeral = false;
	private orientation: TabStripOrientation = TabStripOrientation.Horizontal;
	private startVisible: boolean;
	private tabSearchVisible: boolean;
	private startTargetVisible: boolean;
	private tabSearchTargetVisible: boolean;
	private startAnimating = false;
	private tabSearchAnimating = false;
	private bubbleOpen = false;
	private menuOpenFor: TabStripComboButtonId | null = null;
	private keepCurrentStateLock = false;
	private hideTimer: ReturnType<typeof setTimeout> | null = null;
	private startAnimationTimer: ReturnType<typeof setTimeout> | null = null;
	private endAnimationTimer: ReturnType<typeof setTimeout> | null = null;
	private revision = 0;
	private snapshot: TabStripComboButtonSnapshot;

	public constructor(options: TabStripComboButtonControllerOptions = {}) {
		this.options = options;
		this.context = options.context ?? TabStripComboContext.VerticalTabStrip;
		this.configuredStartKind = options.startKind ?? TabStripComboStartKind.Projects;
		this.richAnimations = options.richAnimations ?? true;
		this.startPinned = options.startPinned ?? false;
		this.tabSearchPinned = options.tabSearchPinned ?? false;
		this.startTargetVisible =
			this.context === TabStripComboContext.VerticalTabStrip && this.startPinned;
		this.tabSearchTargetVisible = this.tabSearchPinned;
		this.startVisible = this.startTargetVisible;
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
		if (button === 'start') {
			if (this.startPinned === pinned) return;
			this.startPinned = pinned;
		} else {
			if (this.tabSearchPinned === pinned) return;
			this.tabSearchPinned = pinned;
		}
		this.updateButtonsVisibility();
		this.options.onPinnedChange?.(button, pinned);
	}

	public executeTogglePinned(button: TabStripComboButtonId): void {
		if (button === 'tab-search') {
			this.tabSearchEphemeral = false;
			this.clearHideTimer();
			const pinned = !this.tabSearchPinned;
			this.options.onUserAction?.(
				`TabStripComboButton.TabSearch.${pinned ? 'Pinned' : 'Unpinned'}`,
			);
			this.setPinned(button, pinned);
			return;
		}
		const pinned = !this.startPinned;
		const name =
			this.configuredStartKind === TabStripComboStartKind.Projects
				? 'ProjectsPanel'
				: 'EverythingMenu';
		this.options.onUserAction?.(`TabStripComboButton.${name}.${pinned ? 'Pinned' : 'Unpinned'}`);
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
		this.clearAnimationTimer('start');
		this.clearAnimationTimer('tab-search');
		this.listeners.clear();
	}

	private updateButtonsVisibility(): void {
		this.transitionButton(
			'start',
			this.context === TabStripComboContext.VerticalTabStrip && this.startPinned,
		);
		this.transitionButton('tab-search', this.tabSearchPinned || this.tabSearchEphemeral);
		this.commit();
	}

	private transitionButton(button: TabStripComboButtonId, targetVisible: boolean): void {
		const currentTarget =
			button === 'start' ? this.startTargetVisible : this.tabSearchTargetVisible;
		if (currentTarget === targetVisible) return;
		if (button === 'start') this.startTargetVisible = targetVisible;
		else this.tabSearchTargetVisible = targetVisible;
		this.clearAnimationTimer(button);

		if (targetVisible) {
			if (button === 'start') {
				this.startVisible = true;
				this.startAnimating = this.richAnimations;
			} else {
				this.tabSearchVisible = true;
				this.tabSearchAnimating = this.richAnimations;
			}
		} else if (button === 'start') {
			this.startAnimating = this.richAnimations && this.startVisible;
		} else {
			this.tabSearchAnimating = this.richAnimations && this.tabSearchVisible;
		}

		if (!this.richAnimations) {
			if (button === 'start') this.startVisible = targetVisible;
			else this.tabSearchVisible = targetVisible;
			return;
		}
		const timer = setTimeout(() => {
			if (button === 'start') {
				this.startAnimationTimer = null;
				this.startAnimating = false;
				this.startVisible = this.startTargetVisible;
			} else {
				this.endAnimationTimer = null;
				this.tabSearchAnimating = false;
				this.tabSearchVisible = this.tabSearchTargetVisible;
			}
			this.commit();
		}, TAB_STRIP_COMBO_ANIMATION_DURATION_MS);
		if (button === 'start') this.startAnimationTimer = timer;
		else this.endAnimationTimer = timer;
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

	private clearAnimationTimer(button: TabStripComboButtonId): void {
		const timer = button === 'start' ? this.startAnimationTimer : this.endAnimationTimer;
		if (timer !== null) clearTimeout(timer);
		if (button === 'start') this.startAnimationTimer = null;
		else this.endAnimationTimer = null;
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
			startKind:
				this.context === TabStripComboContext.VerticalTabStrip ? this.configuredStartKind : null,
			startPinned: this.startPinned,
			tabSearchPinned: this.tabSearchPinned,
			tabSearchEphemeral: this.tabSearchEphemeral,
			startVisible: this.startVisible,
			tabSearchVisible: this.tabSearchVisible,
			startTargetVisible: this.startTargetVisible,
			tabSearchTargetVisible: this.tabSearchTargetVisible,
			startAnimating: this.startAnimating,
			tabSearchAnimating: this.tabSearchAnimating,
			bubbleOpen: this.bubbleOpen,
			menuOpenFor: this.menuOpenFor,
			keepCurrentStateLock: this.keepCurrentStateLock,
			hideTabSearchPending: this.hideTimer !== null,
		});
	}
}
