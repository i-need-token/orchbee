import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	getThumbnailPlan,
	HoverCardUpdateType,
	TabHoverCardController,
	ThumbnailCaptureReadiness,
	ThumbnailWaitState,
} from '../tab-hover-card-controller';

const target = (id: string, width = 64) => ({
	id,
	width,
	minimumWidth: 64,
	standardWidth: 256,
});

describe('TabHoverCardController', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('ports Chromium logarithmic width delay including the max-width addition', () => {
		const delays = [64, 256, 128].map((width) => {
			const controller = new TabHoverCardController();
			controller.updateHoverCard(target('a', width), HoverCardUpdateType.Hover);
			const delay = controller.getSnapshot().showDelayMs;
			controller.destroy();
			return delay;
		});
		expect(delays[0]).toBe(300);
		expect(delays[1]).toBe(1300);
		expect(delays[2]).toBeGreaterThan(300);
		expect(delays[2]).toBeLessThan(800);
	});

	it('waits for an initial card and switches visible cards immediately', () => {
		const controller = new TabHoverCardController();
		controller.updateHoverCard(target('a'), HoverCardUpdateType.Hover);
		expect(controller.getSnapshot()).toMatchObject({
			visible: false,
			waitingToShow: true,
			showDelayMs: 300,
		});
		vi.advanceTimersByTime(300);
		expect(controller.getSnapshot()).toMatchObject({ visible: true, targetId: 'a', cardsSeen: 1 });

		controller.updateHoverCard(target('b'), HoverCardUpdateType.Hover);
		expect(controller.getSnapshot()).toMatchObject({
			visible: true,
			targetId: 'b',
			transitioning: true,
			cardsSeen: 2,
		});
		vi.advanceTimersByTime(200);
		expect(controller.getSnapshot().transitioning).toBe(false);
	});

	it('reshows immediately within 300ms of an accidental mouse exit', () => {
		const controller = new TabHoverCardController();
		controller.updateHoverCard(target('a'), HoverCardUpdateType.Hover, 1_000);
		vi.advanceTimersByTime(300);
		controller.updateHoverCard(null, HoverCardUpdateType.Hover, 1_300);
		controller.updateHoverCard(target('b'), HoverCardUpdateType.Hover, 1_599);
		expect(controller.getSnapshot()).toMatchObject({
			visible: true,
			waitingToShow: false,
			targetId: 'b',
		});
	});

	it('suppresses cards while a scoped hide lock is alive', () => {
		const controller = new TabHoverCardController();
		const lock = controller.getHideLock();
		controller.updateHoverCard(target('a'), HoverCardUpdateType.Hover);
		vi.runAllTimers();
		expect(controller.getSnapshot()).toMatchObject({ visible: false, hideLockCount: 1 });
		lock.dispose();
		expect(controller.getSnapshot().hideLockCount).toBe(0);
	});
});

describe('hover-card thumbnail planning', () => {
	const base = {
		previewsEnabled: true,
		needsThumbnail: true,
		isGroup: false,
		discarded: false,
		crashed: false,
		hasThumbnailObject: true,
		hasThumbnailData: false,
		readiness: ThumbnailCaptureReadiness.ReadyForInitialCapture,
		isInitialShow: false,
		useAnimations: true,
		crossfadeStart: 0,
	} as const;

	it('delays a loading capture and exposes a placeholder', () => {
		expect(getThumbnailPlan(base)).toEqual({
			presentation: 'placeholder',
			waitState: ThumbnailWaitState.WaitingWithPlaceholder,
			captureDelayMs: 300,
			shouldObserve: false,
		});
	});

	it('does not request a missing discarded thumbnail and shows crash state', () => {
		expect(getThumbnailPlan({ ...base, discarded: true }).presentation).toBe('none');
		expect(getThumbnailPlan({ ...base, crashed: true }).presentation).toBe('crashed');
	});
});
