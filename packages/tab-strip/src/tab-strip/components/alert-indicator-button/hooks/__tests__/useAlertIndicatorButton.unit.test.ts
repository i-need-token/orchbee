import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { useAlertIndicatorButton } from '../useAlertIndicatorButton';

afterEach(cleanup);

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
