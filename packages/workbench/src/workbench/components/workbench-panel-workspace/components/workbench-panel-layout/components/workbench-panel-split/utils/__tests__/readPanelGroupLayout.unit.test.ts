import type { ResizablePanelGroupHandle } from '@orchbee/ui/components';
import { describe, expect, it, vi } from 'vitest';

import { readPanelGroupLayout } from '../readPanelGroupLayout';

describe('readPanelGroupLayout', () => {
	it('returns the current primitive layout', () => {
		const layout = { 'split-1--panel-1': 50, 'split-1--panel-2': 50 };
		const group = {
			getLayout: vi.fn<() => Readonly<Record<string, number>>>(() => layout),
		} as unknown as ResizablePanelGroupHandle;

		expect(readPanelGroupLayout(group)).toEqual(layout);
	});

	it('returns null while the primitive group registry is being replaced', () => {
		const group = {
			getLayout: vi.fn<() => Readonly<Record<string, number>>>(() => {
				throw new Error("Could not find Group with id 'split-1'");
			}),
		} as unknown as ResizablePanelGroupHandle;

		expect(readPanelGroupLayout(group)).toBeNull();
	});

	it('returns null without an imperative group', () => {
		expect(readPanelGroupLayout(null)).toBeNull();
	});

	it('does not hide unrelated primitive failures', () => {
		const group = {
			getLayout: vi.fn<() => Readonly<Record<string, number>>>(() => {
				throw new Error('Panel ids must be unique');
			}),
		} as unknown as ResizablePanelGroupHandle;

		expect(() => readPanelGroupLayout(group)).toThrow('Panel ids must be unique');
	});
});
