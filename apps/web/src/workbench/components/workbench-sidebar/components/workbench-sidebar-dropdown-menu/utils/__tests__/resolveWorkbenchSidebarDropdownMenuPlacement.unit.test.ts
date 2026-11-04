import { describe, expect, it } from 'vitest';

import { resolveWorkbenchSidebarDropdownMenuPlacement } from '../resolveWorkbenchSidebarDropdownMenuPlacement';

describe('resolveWorkbenchSidebarDropdownMenuPlacement', () => {
	it('places the menu to the right when its full reference width and clearance fit', () => {
		expect(
			resolveWorkbenchSidebarDropdownMenuPlacement({
				menuInlineSize: 250,
				triggerInlineEnd: 320,
				viewportInlineSize: 579,
			}),
		).toBe('right-start');
	});

	it('falls back below and toward the left when the right side is constrained', () => {
		expect(
			resolveWorkbenchSidebarDropdownMenuPlacement({
				menuInlineSize: 250,
				triggerInlineEnd: 320,
				viewportInlineSize: 578,
			}),
		).toBe('bottom-end');
	});

	it('keeps the right side as the primitive-managed fallback when the token cannot be read', () => {
		expect(
			resolveWorkbenchSidebarDropdownMenuPlacement({
				menuInlineSize: null,
				triggerInlineEnd: 320,
				viewportInlineSize: 320,
			}),
		).toBe('right-start');
	});
});
