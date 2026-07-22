import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { useGroupHeader } from '../useGroupHeader';

afterEach(cleanup);

describe('useGroupHeader', () => {
	it('initializes pointer and editor interaction state', () => {
		const { result } = renderHook(() => useGroupHeader());

		expect(result.current.lastPointerType.current).toBe('mouse');
		expect(result.current.editorOpenedOnPress.current).toBe(false);
		expect(result.current.measuredHeaderSize).toEqual({ width: 0, height: 0 });
	});
});
