import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { useElementSize } from '../useElementSize';

afterEach(cleanup);

describe('useElementSize', () => {
	it('provides an attachable ref and a zero initial size', () => {
		const { result } = renderHook(() => useElementSize<HTMLDivElement>());

		expect(result.current[0].current).toBeNull();
		expect(result.current[1]).toEqual({ width: 0, height: 0 });
	});
});
