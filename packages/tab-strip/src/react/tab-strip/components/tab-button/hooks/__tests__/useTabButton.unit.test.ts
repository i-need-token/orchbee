import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { useTabButton } from '../useTabButton';

afterEach(cleanup);

describe('useTabButton', () => {
	it('owns hover and focus state without changing the initial measurement', () => {
		const { result } = renderHook(() => useTabButton());
		expect(result.current.measuredTabSize).toEqual({ width: 0, height: 0 });

		act(() => {
			result.current.setHovered(true);
			result.current.setFocused(true);
		});
		expect(result.current.hovered).toBe(true);
		expect(result.current.focused).toBe(true);
	});
});
