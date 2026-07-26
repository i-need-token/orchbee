import type { ResizablePanelGroupHandle } from '@orchbee/ui/components';
import { describe, expect, it, vi } from 'vitest';

import { writePanelGroupLayout } from '../writePanelGroupLayout';

describe('writePanelGroupLayout', () => {
	it('writes a copied layout to the primitive group', () => {
		const setLayout = vi.fn<(layout: Readonly<Record<string, number>>) => void>();
		const group = { setLayout } as unknown as ResizablePanelGroupHandle;
		const layout = { 'split-1--panel-1': 50, 'split-1--panel-2': 50 };

		expect(writePanelGroupLayout(group, layout)).toBe(true);
		expect(setLayout).toHaveBeenCalledWith(layout);
		expect(setLayout.mock.calls[0]?.[0]).not.toBe(layout);
	});

	it('returns false while the primitive group registry is being replaced', () => {
		const group = {
			setLayout: vi.fn<(layout: Readonly<Record<string, number>>) => void>(() => {
				throw new Error("Could not find Group with id 'split-1'");
			}),
		} as unknown as ResizablePanelGroupHandle;

		expect(writePanelGroupLayout(group, {})).toBe(false);
	});

	it('returns false without an imperative group', () => {
		expect(writePanelGroupLayout(null, {})).toBe(false);
	});

	it('does not hide unrelated primitive failures', () => {
		const group = {
			setLayout: vi.fn<(layout: Readonly<Record<string, number>>) => void>(() => {
				throw new Error('Panel ids must be unique');
			}),
		} as unknown as ResizablePanelGroupHandle;

		expect(() => writePanelGroupLayout(group, {})).toThrow('Panel ids must be unique');
	});
});
