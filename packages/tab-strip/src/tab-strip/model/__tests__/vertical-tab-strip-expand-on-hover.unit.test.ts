import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	ExpandOnHoverLockType,
	VerticalTabStripExpandOnHoverController,
	VerticalTabStripExpandOnHoverLock,
} from '../vertical-tab-strip-expand-on-hover';

describe('VerticalTabStripExpandOnHoverController', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it("expands after Chromium's fixed delay and collapses on exit", () => {
		const controller = new VerticalTabStripExpandOnHoverController(
			350,
			{},
			{
				useVelocityHeuristic: false,
			},
		);
		controller.setHovered(true);
		expect(controller.getSnapshot().waitingToExpand).toBe(true);
		vi.advanceTimersByTime(349);
		expect(controller.getSnapshot().expanded).toBe(false);
		vi.advanceTimersByTime(1);
		expect(controller.getSnapshot().expanded).toBe(true);

		controller.setHovered(false);
		expect(controller.getSnapshot().expanded).toBe(false);
	});

	it('applies force-collapse before current-state and keep-expanded locks', () => {
		const controller = new VerticalTabStripExpandOnHoverController(
			0,
			{},
			{
				useVelocityHeuristic: false,
			},
		);
		const expanded = controller.getLock(ExpandOnHoverLockType.KeepExpanded);
		vi.runAllTimers();
		expect(controller.getSnapshot().expanded).toBe(true);

		const current = controller.getLock(ExpandOnHoverLockType.KeepCurrentState);
		const collapsed = controller.getLock(ExpandOnHoverLockType.ForceCollapse);
		expect(controller.getSnapshot().expanded).toBe(false);

		collapsed.dispose();
		expect(controller.getSnapshot().expanded).toBe(false);
		current.dispose();
		expect(controller.getSnapshot().expanded).toBe(true);
		expanded.dispose();
		expect(controller.getSnapshot().expanded).toBe(false);
	});

	it('holds the exact current state while a context-menu lock is alive', () => {
		const controller = new VerticalTabStripExpandOnHoverController(
			10,
			{},
			{
				useVelocityHeuristic: false,
			},
		);
		controller.setHovered(true);
		vi.advanceTimersByTime(10);
		const lock = controller.getLock(ExpandOnHoverLockType.KeepCurrentState);

		controller.setHovered(false);
		expect(controller.getSnapshot().expanded).toBe(true);
		lock.dispose();
		expect(controller.getSnapshot().expanded).toBe(false);
	});

	it('does not expand when the strip is not collapsed or the frame is inactive', () => {
		const controller = new VerticalTabStripExpandOnHoverController(
			0,
			{ collapsed: false },
			{
				useVelocityHeuristic: false,
			},
		);
		controller.setHovered(true);
		vi.runAllTimers();
		expect(controller.getSnapshot().expanded).toBe(false);

		controller.setEnvironment({ collapsed: true, frameActive: false });
		const lock = controller.getLock(ExpandOnHoverLockType.KeepExpanded);
		expect(controller.getSnapshot().expanded).toBe(true);
		lock.dispose();
		expect(controller.getSnapshot().expanded).toBe(false);
	});

	it('allows a null-region lock to be created and disposed safely', () => {
		const lock = new VerticalTabStripExpandOnHoverLock(
			null,
			ExpandOnHoverLockType.KeepCurrentState,
		);
		expect(() => lock.dispose()).not.toThrow();
	});

	it("uses Chromium's inside-edge delay for a stationary pointer", () => {
		const controller = new VerticalTabStripExpandOnHoverController();
		controller.recordPointerPosition(72, 72);
		controller.setHovered(true);

		vi.advanceTimersByTime(200);
		expect(controller.getSnapshot()).toMatchObject({ expanded: false, waitingToExpand: true });
		vi.advanceTimersByTime(10);
		expect(controller.getSnapshot()).toMatchObject({ expanded: true, waitingToExpand: false });
	});

	it('expands after the minimum velocity delay when the pointer is fully inside', () => {
		const controller = new VerticalTabStripExpandOnHoverController();
		controller.recordPointerPosition(72, 72);
		controller.setHovered(true);
		controller.recordPointerPosition(58, 72);

		vi.advanceTimersByTime(50);
		expect(controller.getSnapshot().expanded).toBe(false);
		vi.advanceTimersByTime(10);
		expect(controller.getSnapshot().expanded).toBe(true);
	});

	it('keeps sampling a fast pointer until its average horizontal speed falls', () => {
		const controller = new VerticalTabStripExpandOnHoverController();
		controller.recordPointerPosition(72, 72);
		controller.setHovered(true);
		vi.advanceTimersByTime(10);
		controller.recordPointerPosition(20, 72);

		vi.advanceTimersByTime(190);
		expect(controller.getSnapshot().expanded).toBe(false);
		vi.advanceTimersByTime(10);
		expect(controller.getSnapshot().expanded).toBe(true);
	});
});
