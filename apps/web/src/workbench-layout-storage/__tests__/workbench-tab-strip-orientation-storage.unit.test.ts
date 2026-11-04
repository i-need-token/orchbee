import { TabStripOrientation } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import {
	readWorkbenchTabStripOrientation,
	writeWorkbenchTabStripOrientation,
} from '../workbench-tab-strip-orientation-storage';

describe('workbench tab-strip orientation storage', () => {
	it.each([
		['{"orientation":"horizontal"}', TabStripOrientation.Horizontal],
		['{"orientation":"vertical"}', TabStripOrientation.Vertical],
	])('reads a supported stored orientation', (serializedValue, expectedOrientation) => {
		expect(
			readWorkbenchTabStripOrientation({
				getItem: () => serializedValue,
				setItem: () => undefined,
			}),
		).toBe(expectedOrientation);
	});

	it.each([
		null,
		'{}',
		'{"orientation":"diagonal"}',
		'{"orientation":"vertical","extra":true}',
		'not-json',
	])('falls back to horizontal for unavailable or invalid input', (serializedValue) => {
		expect(
			readWorkbenchTabStripOrientation({
				getItem: () => serializedValue,
				setItem: () => undefined,
			}),
		).toBe(TabStripOrientation.Horizontal);
	});

	it('falls back when storage is unavailable', () => {
		expect(
			readWorkbenchTabStripOrientation({
				getItem: () => {
					throw new Error('unavailable');
				},
				setItem: () => undefined,
			}),
		).toBe(TabStripOrientation.Horizontal);
	});

	it('writes the canonical versioned record and ignores storage failures', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		writeWorkbenchTabStripOrientation(
			{ getItem: () => null, setItem },
			TabStripOrientation.Vertical,
		);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:workbench:tab-strip-orientation:v1',
			'{"orientation":"vertical"}',
		);

		expect(() =>
			writeWorkbenchTabStripOrientation(
				{
					getItem: () => null,
					setItem: () => {
						throw new Error('unavailable');
					},
				},
				TabStripOrientation.Horizontal,
			),
		).not.toThrow();
	});
});
