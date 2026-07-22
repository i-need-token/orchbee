import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useContextMenu } from '../useContextMenu';

afterEach(cleanup);

describe('useContextMenu', () => {
	it('owns submenu state', () => {
		const { result } = renderHook(() =>
			useContextMenu({
				state: { kind: 'new-tab', x: 0, y: 0 },
				onClose: vi.fn<() => void>(),
			}),
		);

		act(() => {
			result.current.setTabGroupSubmenuOpen(true);
		});
		expect(result.current.tabGroupSubmenuOpen).toBe(true);
	});
});
