// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import type { MouseEvent, PointerEvent } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useTabCloseButton } from '../useTabCloseButton';

afterEach(cleanup);

describe('useTabCloseButton', () => {
	it('remembers the pointer type used for the close click', () => {
		const onClose = vi.fn<(event: MouseEvent<HTMLButtonElement>, pointerType: string) => void>();
		const { result } = renderHook(() => useTabCloseButton({ onClose }));
		const button = document.createElement('button');
		const stopPropagation = vi.fn<() => void>();
		const pointerEvent = {
			button: 0,
			clientX: 1,
			clientY: 1,
			currentTarget: button,
			pointerType: 'pen',
			preventDefault: vi.fn<() => void>(),
			stopPropagation,
		} as unknown as PointerEvent<HTMLButtonElement>;
		const clickEvent = { stopPropagation } as unknown as MouseEvent<HTMLButtonElement>;

		act(() => {
			result.current.onPointerDown(pointerEvent);
			result.current.onClick(clickEvent);
		});

		expect(onClose).toHaveBeenCalledWith(clickEvent, 'pen');
		expect(stopPropagation).toHaveBeenCalled();
	});
});
