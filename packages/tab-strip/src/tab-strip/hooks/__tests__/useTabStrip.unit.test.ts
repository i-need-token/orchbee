import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TabStripOrientation } from '../../types/tab-strip-types';
import { TabModel } from '../../model/tab-model';
import { StrictMode } from 'react';
import { TabStripComboButtonController } from '../../model/tab-strip-combo-button';
import { useTabStrip } from '../useTabStrip';

afterEach(async () => {
	cleanup();
	await Promise.resolve();
	vi.useRealTimers();
});

describe('useTabStrip', () => {
	it('prepares the horizontal view model without rendering JSX', () => {
		const model = new TabModel({
			tabs: [{ id: 'tab', title: 'Tab', pinned: false, group: null }],
			groups: [],
			activeTabId: 'tab',
		});
		const { result } = renderHook(() => useTabStrip({ model }));

		expect(result.current.orientation).toBe(TabStripOrientation.Horizontal);
		expect(result.current.snapshot.activeTabId).toBe('tab');
	});
});

describe('owned combo button lifecycle', () => {
	it('keeps the initial open animation through StrictMode replay', async () => {
		vi.useFakeTimers();
		const model = new TabModel();
		const { result } = renderHook(() => useTabStrip({ model, verticalTabSearchOpen: true }), {
			wrapper: StrictMode,
		});
		await act(async () => {
			await Promise.resolve();
		});
		expect(result.current.comboButtonSnapshot.tabSearchAnimating).toBe(true);
		act(() => vi.advanceTimersByTime(299));
		expect(result.current.comboButtonSnapshot.tabSearchAnimating).toBe(true);
		act(() => vi.advanceTimersByTime(1));
		expect(result.current.comboButtonSnapshot.tabSearchAnimating).toBe(false);
		expect(result.current.comboButtonSnapshot.tabSearchVisible).toBe(true);
	});

	it('retires internal timers even when an external controller is selected at unmount', async () => {
		vi.useFakeTimers();
		const model = new TabModel();
		const external = new TabStripComboButtonController();
		const initialProps: { controller: TabStripComboButtonController | undefined } = {
			controller: undefined,
		};
		const { result, rerender, unmount } = renderHook(
			({ controller }: { controller: TabStripComboButtonController | undefined }) =>
				useTabStrip({
					model,
					...(controller === undefined ? {} : { verticalComboButtonController: controller }),
				}),
			{ initialProps },
		);
		const owned = result.current.comboButtonController;
		act(() => {
			owned.onBubbleInitializing();
			owned.onBubbleDestroying();
		});
		const before = owned.getSnapshot();
		const listener = vi.fn<() => void>();
		owned.subscribe(listener);
		rerender({ controller: external });
		unmount();
		await Promise.resolve();
		expect(vi.getTimerCount()).toBe(0);
		act(() => vi.advanceTimersByTime(3000));
		expect(owned.getSnapshot()).toBe(before);
		expect(listener).not.toHaveBeenCalled();
		external.destroy();
	});

	it('leaves external controller deadlines and final release to the host', async () => {
		vi.useFakeTimers();
		const model = new TabModel();
		const external = new TabStripComboButtonController({ richAnimations: false });
		external.onBubbleInitializing();
		external.onBubbleDestroying();
		const listener = vi.fn<() => void>();
		external.subscribe(listener);
		const { unmount } = renderHook(() =>
			useTabStrip({ model, verticalComboButtonController: external }),
		);
		unmount();
		await Promise.resolve();
		act(() => vi.advanceTimersByTime(1999));
		expect(external.getSnapshot().tabSearchVisible).toBe(true);
		act(() => vi.advanceTimersByTime(1));
		expect(external.getSnapshot().tabSearchVisible).toBe(false);
		expect(listener).toHaveBeenCalledOnce();
		external.destroy();
	});
});

describe('controlled combo replacement', () => {
	it.each([true, false])('reconciles an incoming controller with open=%s', async (open) => {
		vi.useFakeTimers();
		const model = new TabModel();
		const outgoing = new TabStripComboButtonController({ richAnimations: false });
		const incoming = new TabStripComboButtonController({ richAnimations: false });
		if (!open) incoming.onBubbleInitializing();
		const { rerender, unmount } = renderHook(
			({ controller }) =>
				useTabStrip({
					model,
					verticalComboButtonController: controller,
					verticalTabSearchOpen: open,
				}),
			{ initialProps: { controller: outgoing } },
		);
		try {
			const before = outgoing.getSnapshot();
			rerender({ controller: incoming });
			expect(incoming.getSnapshot().bubbleOpen).toBe(open);
			expect(outgoing.getSnapshot()).toBe(before);
		} finally {
			unmount();
			await Promise.resolve();
			outgoing.destroy();
			incoming.destroy();
		}
	});

	it('keeps the remaining hide deadline of an already closing replacement', async () => {
		vi.useFakeTimers();
		const model = new TabModel();
		const outgoing = new TabStripComboButtonController({ richAnimations: false });
		const incoming = new TabStripComboButtonController({ richAnimations: false });
		incoming.onBubbleInitializing();
		incoming.onBubbleDestroying();
		vi.advanceTimersByTime(1000);
		const { rerender, unmount } = renderHook(
			({ controller }) => useTabStrip({ model, verticalComboButtonController: controller }),
			{ initialProps: { controller: outgoing } },
		);
		try {
			rerender({ controller: incoming });
			act(() => vi.advanceTimersByTime(999));
			expect(incoming.getSnapshot().tabSearchVisible).toBe(true);
			act(() => vi.advanceTimersByTime(1));
			expect(incoming.getSnapshot().tabSearchVisible).toBe(false);
		} finally {
			unmount();
			await Promise.resolve();
			outgoing.destroy();
			incoming.destroy();
		}
	});
});
