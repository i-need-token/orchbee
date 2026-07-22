export const GLIC_SPLIT_BUTTON_FLAT_EDGE_RADIUS = 2;
export const GLIC_SPLIT_BUTTON_ROUNDED_EDGE_RADIUS = 10;
export const GLIC_CONTAINER_CORNER_RADIUS = 12;

export const GlicButtonWidthState = {
	Normal: 'normal',
	Nudge: 'nudge',
	Collapsed: 'collapsed',
} as const;

export type GlicButtonWidthState = (typeof GlicButtonWidthState)[keyof typeof GlicButtonWidthState];

export const GlicNudgeTransitionState = {
	None: 'none',
	Showing: 'showing',
	Hiding: 'hiding',
} as const;

export type GlicNudgeTransitionState =
	(typeof GlicNudgeTransitionState)[keyof typeof GlicNudgeTransitionState];

export const GlicLockedExpansionMode = {
	None: 'none',
	WillShowGlic: 'will-show-glic',
	WillHideGlic: 'will-hide-glic',
	WillShowActor: 'will-show-actor',
	WillHideActor: 'will-hide-actor',
} as const;

export type GlicLockedExpansionMode =
	(typeof GlicLockedExpansionMode)[keyof typeof GlicLockedExpansionMode];

export interface GlicControlsSnapshot {
	revision: number;
	widthState: GlicButtonWidthState;
	nudgeLabel: string;
	actorVisible: boolean;
	actorNudgeLabel: string;
	actorNudgeShowing: boolean;
	actorPressed: boolean;
	panelOpen: boolean;
	pinned: boolean;
	nudgeTransition: GlicNudgeTransitionState;
	lockedExpansionMode: GlicLockedExpansionMode;
}

export interface GlicControlsControllerInit extends Partial<
	Omit<GlicControlsSnapshot, 'revision'>
> {
	richAnimations?: boolean;
}

export interface GlicButtonPresentation {
	label: string;
	showLabel: boolean;
	showCloseButton: boolean;
}

export function getGlicButtonPresentation(
	state: GlicButtonWidthState,
	nudgeLabel: string,
): GlicButtonPresentation {
	switch (state) {
		case GlicButtonWidthState.Nudge:
			return {
				label: nudgeLabel || 'Ask Gemini about this page',
				showLabel: true,
				showCloseButton: true,
			};
		case GlicButtonWidthState.Collapsed:
			return { label: 'Gemini', showLabel: false, showCloseButton: false };
		default:
			return { label: 'Gemini', showLabel: true, showCloseButton: false };
	}
}

type Listener = () => void;

/**
 * Browser-independent state from TabStripGlicButton, the actor task icon, and
 * TabStripActionContainer's mutually-exclusive nudge rules.
 */
export class GlicControlsController {
	private snapshot: GlicControlsSnapshot;
	private readonly listeners = new Set<Listener>();
	private readonly richAnimations: boolean;
	private nudgeTransitionTimer: ReturnType<typeof setTimeout> | null = null;
	private lockedExpansionHovered = false;

	public constructor(init: GlicControlsControllerInit = {}) {
		this.richAnimations = init.richAnimations ?? true;
		this.snapshot = Object.freeze({
			revision: 0,
			widthState: init.widthState ?? GlicButtonWidthState.Normal,
			nudgeLabel: init.nudgeLabel ?? '',
			actorVisible: init.actorVisible ?? false,
			actorNudgeLabel: init.actorNudgeLabel ?? '',
			actorNudgeShowing: init.actorNudgeShowing ?? Boolean(init.actorNudgeLabel),
			actorPressed: init.actorPressed ?? false,
			panelOpen: init.panelOpen ?? false,
			pinned: init.pinned ?? true,
			nudgeTransition: init.nudgeTransition ?? GlicNudgeTransitionState.None,
			lockedExpansionMode: init.lockedExpansionMode ?? GlicLockedExpansionMode.None,
		});
	}

	public getSnapshot = (): GlicControlsSnapshot => this.snapshot;

	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public setCollapsed(collapsed: boolean): void {
		this.clearNudgeTransitionTimer();
		this.update({
			widthState: collapsed ? GlicButtonWidthState.Collapsed : GlicButtonWidthState.Normal,
			nudgeLabel: '',
			nudgeTransition: GlicNudgeTransitionState.None,
			lockedExpansionMode: GlicLockedExpansionMode.None,
		});
	}

	public triggerGlicNudge(label: string): void {
		// Chromium gives an actor task nudge precedence over the Gemini nudge.
		if (this.snapshot.actorNudgeShowing) return;
		if (this.snapshot.widthState === GlicButtonWidthState.Nudge) {
			this.update({ nudgeLabel: label });
			return;
		}
		if (this.lockedExpansionHovered) {
			this.update({
				nudgeLabel: label,
				actorNudgeLabel: '',
				actorNudgeShowing: false,
				lockedExpansionMode: GlicLockedExpansionMode.WillShowGlic,
			});
			return;
		}
		this.executeShowGlicNudge(label);
	}

	public hideGlicNudge(): void {
		if (this.lockedExpansionHovered) {
			this.update({ lockedExpansionMode: GlicLockedExpansionMode.WillHideGlic });
			return;
		}
		this.executeHideGlicNudge();
	}

	/** Dismiss/click bypasses hover locking, matching OnGlicButtonDismissed. */
	public dismissGlicNudge(): void {
		this.update({ lockedExpansionMode: GlicLockedExpansionMode.None });
		this.executeHideGlicNudge();
	}

	/** Mirrors the MouseWatcher around locked_expansion_view_. */
	public setLockedExpansionHovered(hovered: boolean): void {
		if (this.lockedExpansionHovered === hovered) return;
		this.lockedExpansionHovered = hovered;
		if (hovered) return;
		const pending = this.snapshot.lockedExpansionMode;
		if (pending === GlicLockedExpansionMode.None) return;
		this.update({ lockedExpansionMode: GlicLockedExpansionMode.None });
		switch (pending) {
			case GlicLockedExpansionMode.WillShowGlic:
				this.executeShowGlicNudge(this.snapshot.nudgeLabel);
				break;
			case GlicLockedExpansionMode.WillHideGlic:
				this.executeHideGlicNudge();
				break;
			case GlicLockedExpansionMode.WillShowActor:
				this.executeShowActorNudge();
				break;
			case GlicLockedExpansionMode.WillHideActor:
				this.executeHideActorNudge();
				break;
			default:
				break;
		}
	}

	/** Host hook for GlicNudgeController's active-tab observation. */
	public notifyActiveTabChanged(): void {
		if (
			this.snapshot.widthState === GlicButtonWidthState.Nudge ||
			this.snapshot.lockedExpansionMode === GlicLockedExpansionMode.WillShowGlic
		) {
			this.hideGlicNudge();
		}
		this.update({ nudgeLabel: '' });
	}

	private executeShowGlicNudge(label: string): void {
		this.clearNudgeTransitionTimer();
		this.update({
			widthState: GlicButtonWidthState.Nudge,
			nudgeLabel: label,
			actorNudgeLabel: '',
			actorNudgeShowing: false,
			lockedExpansionMode: GlicLockedExpansionMode.None,
			nudgeTransition: this.richAnimations
				? GlicNudgeTransitionState.Showing
				: GlicNudgeTransitionState.None,
		});
		if (this.richAnimations) {
			this.nudgeTransitionTimer = setTimeout(() => {
				this.nudgeTransitionTimer = null;
				this.update({ nudgeTransition: GlicNudgeTransitionState.None });
			}, TAB_STRIP_NUDGE_EXPANSION_IN_MS);
		}
	}

	private executeHideGlicNudge(): void {
		if (this.snapshot.widthState !== GlicButtonWidthState.Nudge) return;
		this.clearNudgeTransitionTimer();
		if (!this.richAnimations) {
			this.update({
				widthState: GlicButtonWidthState.Normal,
				nudgeLabel: '',
				nudgeTransition: GlicNudgeTransitionState.None,
				lockedExpansionMode: GlicLockedExpansionMode.None,
			});
			return;
		}
		this.update({ nudgeTransition: GlicNudgeTransitionState.Hiding });
		this.nudgeTransitionTimer = setTimeout(() => {
			this.nudgeTransitionTimer = null;
			this.update({
				widthState: GlicButtonWidthState.Normal,
				nudgeLabel: '',
				nudgeTransition: GlicNudgeTransitionState.None,
				lockedExpansionMode: GlicLockedExpansionMode.None,
			});
		}, TAB_STRIP_NUDGE_EXPANSION_OUT_MS);
	}

	public setActorVisible(visible: boolean): void {
		this.update({
			actorVisible: visible,
			actorNudgeLabel: visible ? this.snapshot.actorNudgeLabel : '',
			actorNudgeShowing: visible && this.snapshot.actorNudgeShowing,
			actorPressed: visible && this.snapshot.actorPressed,
			lockedExpansionMode:
				!visible &&
				(this.snapshot.lockedExpansionMode === GlicLockedExpansionMode.WillShowActor ||
					this.snapshot.lockedExpansionMode === GlicLockedExpansionMode.WillHideActor)
					? GlicLockedExpansionMode.None
					: this.snapshot.lockedExpansionMode,
		});
	}

	public triggerActorNudge(label: string): void {
		const wasShowingGlicNudge = this.snapshot.widthState === GlicButtonWidthState.Nudge;
		if (wasShowingGlicNudge) this.hideGlicNudge();
		this.update({
			actorVisible: true,
			actorNudgeLabel: label,
			actorNudgeShowing: !this.lockedExpansionHovered,
			lockedExpansionMode: this.lockedExpansionHovered
				? GlicLockedExpansionMode.WillShowActor
				: GlicLockedExpansionMode.None,
		});
	}

	public hideActorNudge(): void {
		if (!this.snapshot.actorNudgeLabel && !this.snapshot.actorNudgeShowing) return;
		if (this.lockedExpansionHovered) {
			this.update({ lockedExpansionMode: GlicLockedExpansionMode.WillHideActor });
			return;
		}
		this.executeHideActorNudge();
	}

	private executeShowActorNudge(): void {
		if (!this.snapshot.actorNudgeLabel) return;
		if (this.snapshot.widthState === GlicButtonWidthState.Nudge) this.executeHideGlicNudge();
		this.update({ actorVisible: true, actorNudgeShowing: true });
	}

	private executeHideActorNudge(): void {
		this.update({ actorNudgeLabel: '', actorNudgeShowing: false });
	}

	public setActorPressed(pressed: boolean): void {
		this.update({ actorPressed: pressed });
	}

	public setPanelOpen(open: boolean): void {
		this.update({ panelOpen: open });
	}

	public setPinned(pinned: boolean): void {
		this.update({ pinned });
	}

	public destroy(): void {
		this.clearNudgeTransitionTimer();
		this.lockedExpansionHovered = false;
		this.listeners.clear();
	}

	private clearNudgeTransitionTimer(): void {
		if (this.nudgeTransitionTimer !== null) clearTimeout(this.nudgeTransitionTimer);
		this.nudgeTransitionTimer = null;
	}

	private update(change: Partial<Omit<GlicControlsSnapshot, 'revision'>>): void {
		const next = { ...this.snapshot, ...change, revision: this.snapshot.revision + 1 };
		const changed =
			next.widthState !== this.snapshot.widthState ||
			next.nudgeLabel !== this.snapshot.nudgeLabel ||
			next.actorVisible !== this.snapshot.actorVisible ||
			next.actorNudgeLabel !== this.snapshot.actorNudgeLabel ||
			next.actorNudgeShowing !== this.snapshot.actorNudgeShowing ||
			next.actorPressed !== this.snapshot.actorPressed ||
			next.panelOpen !== this.snapshot.panelOpen ||
			next.pinned !== this.snapshot.pinned ||
			next.nudgeTransition !== this.snapshot.nudgeTransition ||
			next.lockedExpansionMode !== this.snapshot.lockedExpansionMode;
		if (!changed) return;
		this.snapshot = Object.freeze(next);
		this.listeners.forEach((listener) => listener());
	}
}
import {
	TAB_STRIP_NUDGE_EXPANSION_IN_MS,
	TAB_STRIP_NUDGE_EXPANSION_OUT_MS,
} from './tab-strip-nudge-animation';
