import { afterEach, describe, expect, it, vi } from 'vitest';

import { canArmWorkbenchTabDrag } from '../canArmWorkbenchTabDrag';

class TestNode {
	readonly testNode = true;
}

describe('canArmWorkbenchTabDrag', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('accepts a primary pointer that originated inside the draggable item', () => {
		vi.stubGlobal('Node', TestNode);
		const target = new TestNode();

		expect(
			canArmWorkbenchTabDrag({
				button: 0,
				currentTarget: { contains: (node) => node === (target as unknown as Node) },
				isPrimary: true,
				target: target as unknown as EventTarget,
			}),
		).toBe(true);
	});

	it('rejects a primary pointer from a portaled context-menu layer', () => {
		vi.stubGlobal('Node', TestNode);
		const target = new TestNode();

		expect(
			canArmWorkbenchTabDrag({
				button: 0,
				currentTarget: { contains: () => false },
				isPrimary: true,
				target: target as unknown as EventTarget,
			}),
		).toBe(false);
	});

	it('rejects a secondary pointer inside the draggable item', () => {
		vi.stubGlobal('Node', TestNode);
		const target = new TestNode();

		expect(
			canArmWorkbenchTabDrag({
				button: 2,
				currentTarget: { contains: () => true },
				isPrimary: true,
				target: target as unknown as EventTarget,
			}),
		).toBe(false);
	});
});
