// @vitest-environment jsdom

import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { useWorkbenchConfirmationPopover } from '../useWorkbenchConfirmationPopover';

afterEach(cleanup);

describe('useWorkbenchConfirmationPopover', () => {
	it('projects the confirmation point as a virtual Popover anchor', () => {
		const rendered = renderHook(() => useWorkbenchConfirmationPopover({ x: 320, y: 240 }));

		expect(rendered.result.current.virtualAnchor.getBoundingClientRect()).toMatchObject({
			bottom: 240,
			height: 0,
			left: 320,
			right: 320,
			top: 240,
			width: 0,
			x: 320,
			y: 240,
		});
		expect(rendered.result.current.confirmButtonRef.current).toBeNull();
	});
});
