import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	GlicButtonWidthState,
	GlicControlsController,
	GlicLockedExpansionMode,
	GlicNudgeTransitionState,
	getGlicButtonPresentation,
} from '../glic-controls';

describe('Glic tab-strip controls', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('maps the three Chromium width states', () => {
		expect(getGlicButtonPresentation(GlicButtonWidthState.Normal, '')).toEqual({
			label: 'Gemini',
			showLabel: true,
			showCloseButton: false,
		});
		expect(getGlicButtonPresentation(GlicButtonWidthState.Nudge, 'Summarize')).toEqual({
			label: 'Summarize',
			showLabel: true,
			showCloseButton: true,
		});
		expect(getGlicButtonPresentation(GlicButtonWidthState.Collapsed, '')).toEqual({
			label: 'Gemini',
			showLabel: false,
			showCloseButton: false,
		});
	});

	it('makes Gemini and actor nudges mutually exclusive', () => {
		const controller = new GlicControlsController();
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);
		controller.triggerGlicNudge('Ask about tabs');
		expect(controller.getSnapshot().widthState).toBe(GlicButtonWidthState.Nudge);
		controller.triggerActorNudge('Task ready');
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Nudge,
			nudgeTransition: GlicNudgeTransitionState.Hiding,
			actorVisible: true,
			actorNudgeLabel: 'Task ready',
		});
		vi.advanceTimersByTime(250);
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Normal,
			nudgeLabel: '',
		});
		expect(listener).toHaveBeenCalledTimes(4);
	});

	it('clears transient actor state when hidden', () => {
		const controller = new GlicControlsController({ actorVisible: true });
		controller.triggerActorNudge('Working');
		controller.setActorPressed(true);
		controller.setActorVisible(false);
		expect(controller.getSnapshot()).toMatchObject({
			actorVisible: false,
			actorNudgeLabel: '',
			actorPressed: false,
		});
	});

	it("retains the nudge content through Chromium's 250ms hide session", () => {
		const controller = new GlicControlsController();
		controller.triggerGlicNudge('Summarize');
		expect(controller.getSnapshot().nudgeTransition).toBe(GlicNudgeTransitionState.Showing);
		vi.advanceTimersByTime(500);
		expect(controller.getSnapshot().nudgeTransition).toBe(GlicNudgeTransitionState.None);

		controller.hideGlicNudge();
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Nudge,
			nudgeLabel: 'Summarize',
			nudgeTransition: GlicNudgeTransitionState.Hiding,
		});
		vi.advanceTimersByTime(249);
		expect(controller.getSnapshot().widthState).toBe(GlicButtonWidthState.Nudge);
		vi.advanceTimersByTime(1);
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Normal,
			nudgeLabel: '',
			nudgeTransition: GlicNudgeTransitionState.None,
		});
	});

	it('stages nudge expansion and collapse until the pointer leaves the locked view', () => {
		const controller = new GlicControlsController({ richAnimations: false });
		controller.setLockedExpansionHovered(true);
		controller.triggerGlicNudge('Summarize');
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Normal,
			nudgeLabel: 'Summarize',
			lockedExpansionMode: GlicLockedExpansionMode.WillShowGlic,
		});

		controller.setLockedExpansionHovered(false);
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Nudge,
			lockedExpansionMode: GlicLockedExpansionMode.None,
		});

		controller.setLockedExpansionHovered(true);
		controller.hideGlicNudge();
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Nudge,
			lockedExpansionMode: GlicLockedExpansionMode.WillHideGlic,
		});
		controller.setLockedExpansionHovered(false);
		expect(controller.getSnapshot().widthState).toBe(GlicButtonWidthState.Normal);
	});

	it('bypasses the hover lock for dismissal and hides on active-tab changes', () => {
		const controller = new GlicControlsController({ richAnimations: false });
		controller.triggerGlicNudge('Summarize');
		controller.setLockedExpansionHovered(true);
		controller.dismissGlicNudge();
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Normal,
			lockedExpansionMode: GlicLockedExpansionMode.None,
		});

		controller.setLockedExpansionHovered(false);
		controller.triggerGlicNudge('Explain');
		controller.setLockedExpansionHovered(true);
		controller.notifyActiveTabChanged();
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Nudge,
			nudgeLabel: '',
			lockedExpansionMode: GlicLockedExpansionMode.WillHideGlic,
		});
		controller.setLockedExpansionHovered(false);
		expect(controller.getSnapshot().widthState).toBe(GlicButtonWidthState.Normal);
	});

	it('stages actor nudge text while hovered and gives it precedence on release', () => {
		const controller = new GlicControlsController({ richAnimations: false });
		controller.triggerGlicNudge('Gemini');
		controller.setLockedExpansionHovered(true);
		controller.triggerActorNudge('Check tasks');
		expect(controller.getSnapshot()).toMatchObject({
			actorVisible: true,
			actorNudgeLabel: 'Check tasks',
			actorNudgeShowing: false,
			lockedExpansionMode: GlicLockedExpansionMode.WillShowActor,
		});
		controller.setLockedExpansionHovered(false);
		expect(controller.getSnapshot()).toMatchObject({
			widthState: GlicButtonWidthState.Normal,
			actorNudgeShowing: true,
		});
	});
});
