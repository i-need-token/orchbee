import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { StrictMode } from 'react';
import type { TabAlertState } from '../../../../../../../../model/tab-model';
import { useAlertIndicatorButton } from '../useAlertIndicatorButton';

afterEach(async () => {
	cleanup();
	await Promise.resolve();
	vi.useRealTimers();
});

describe('useAlertIndicatorButton', () => {
	it('starts with the source-compatible empty indicator state', () => {
		const { result } = renderHook(() =>
			useAlertIndicatorButton({
				alertState: 'none',
			}),
		);

		expect(result.current.visible).toBe(false);
		expect(result.current.snapshot.showingAlertState).toBe('none');
	});
});

describe('owned alert lifecycle', () => {
	it.each([
		{ alertState: 'audio-playing', fadeKind: 'fade-in', duration: 200 },
		{ alertState: 'recording', fadeKind: 'recording', duration: 2600 },
	] as const)(
		'preserves the original $alertState completion through StrictMode replay',
		async ({ alertState, fadeKind, duration }) => {
			vi.useFakeTimers();
			const { result } = renderHook(() => useAlertIndicatorButton({ alertState }), {
				wrapper: StrictMode,
			});
			await act(async () => {
				await Promise.resolve();
			});
			expect(result.current.snapshot.fadeKind).toBe(fadeKind);
			act(() => vi.advanceTimersByTime(duration - 1));
			expect(result.current.snapshot.fadeKind).toBe(fadeKind);
			act(() => vi.advanceTimersByTime(1));
			expect(result.current.snapshot.fadeKind).toBe('none');
			expect(result.current.snapshot.showingAlertState).toBe(alertState);
		},
	);

	it('keeps the original five-second recording hold after replay', async () => {
		vi.useFakeTimers();
		vi.setSystemTime(0);
		const { result, rerender } = renderHook(
			({ alertState }: { alertState: TabAlertState }) => useAlertIndicatorButton({ alertState }),
			{
				initialProps: { alertState: 'recording' },
				wrapper: StrictMode,
			},
		);
		await act(async () => {
			await Promise.resolve();
		});
		act(() => vi.advanceTimersByTime(1000));
		rerender({ alertState: 'none' });
		act(() => vi.advanceTimersByTime(3999));
		expect(result.current.snapshot.showingAlertState).toBe('recording');
		act(() => vi.advanceTimersByTime(1));
		expect(result.current.visible).toBe(false);
	});

	it('clears the pending alert timer on final unmount', async () => {
		vi.useFakeTimers();
		const { unmount } = renderHook(() => useAlertIndicatorButton({ alertState: 'recording' }));
		expect(vi.getTimerCount()).toBe(1);
		unmount();
		await Promise.resolve();
		expect(vi.getTimerCount()).toBe(0);
	});
});
