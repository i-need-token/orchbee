import { createRef } from 'react';
import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { useRoundedScrollbar } from '../useRoundedScrollbar';

afterEach(cleanup);

describe('useRoundedScrollbar', () => {
	it('stays inert until a viewport is attached', () => {
		const viewportRef = createRef<HTMLElement>();
		const { result } = renderHook(() =>
			useRoundedScrollbar({ viewportRef, isAnimatingSize: false }),
		);

		expect(result.current.geometry).toEqual({
			visible: false,
			top: 0,
			height: 0,
			maximumScroll: 0,
		});
		expect(result.current.scrollTop).toBe(0);
	});
});
