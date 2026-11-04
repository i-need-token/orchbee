// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useResizablePanelGroup } from '../useResizablePanelGroup';

const childIds = ['panel-1', 'panel-2'];
const modelLayout = { 'panel-1': 40, 'panel-2': 60 };

afterEach(cleanup);

describe('useResizablePanelGroup', () => {
	it('synchronizes a matching primitive topology to the model layout', () => {
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() =>
			useResizablePanelGroup({
				canSynchronizeLayout: () => true,
				childIds,
				modelLayout,
				orientation: 'horizontal',
			}),
		);
		result.current.groupRef.current = {
			getLayout: () => ({ 'panel-1': 50, 'panel-2': 50 }),
			setLayout,
		};

		act(() => {
			result.current.requestLayoutSync();
		});

		expect(setLayout).toHaveBeenCalledOnce();
		expect(setLayout).toHaveBeenCalledWith(modelLayout);
	});

	it('defers model synchronization while its caller owns an interaction', () => {
		let canSynchronize = false;
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() =>
			useResizablePanelGroup({
				canSynchronizeLayout: () => canSynchronize,
				childIds,
				modelLayout,
				orientation: 'horizontal',
			}),
		);
		result.current.groupRef.current = {
			getLayout: () => ({ 'panel-1': 50, 'panel-2': 50 }),
			setLayout,
		};

		act(() => {
			result.current.requestLayoutSync();
		});
		expect(setLayout).not.toHaveBeenCalled();

		canSynchronize = true;
		act(() => {
			result.current.requestLayoutSync();
		});
		expect(setLayout).toHaveBeenCalledOnce();
	});

	it('does not synchronize an incompatible primitive topology', () => {
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() =>
			useResizablePanelGroup({
				canSynchronizeLayout: () => true,
				childIds,
				modelLayout,
				orientation: 'horizontal',
			}),
		);
		result.current.groupRef.current = {
			getLayout: () => ({ 'other-panel': 100 }),
			setLayout,
		};

		act(() => {
			result.current.requestLayoutSync();
		});

		expect(setLayout).not.toHaveBeenCalled();
	});

	it.each([
		{ expected: 300, orientation: 'horizontal' as const },
		{ expected: 150, orientation: 'vertical' as const },
	])('measures $orientation panel space', ({ expected, orientation }) => {
		const { result } = renderHook(() =>
			useResizablePanelGroup({
				canSynchronizeLayout: () => true,
				childIds,
				modelLayout,
				orientation,
			}),
		);
		const groupElement = document.createElement('div');
		const leadingPanel = document.createElement('div');
		const separator = document.createElement('div');
		const trailingPanel = document.createElement('div');
		leadingPanel.dataset['slot'] = 'resizable-panel';
		separator.dataset['slot'] = 'resizable-panel-handle';
		trailingPanel.dataset['slot'] = 'resizable-panel';
		vi.spyOn(leadingPanel, 'getBoundingClientRect').mockReturnValue({
			bottom: 50,
			height: 50,
			left: 0,
			right: 100,
			toJSON: vi.fn<() => unknown>(),
			top: 0,
			width: 100,
			x: 0,
			y: 0,
		});
		vi.spyOn(trailingPanel, 'getBoundingClientRect').mockReturnValue({
			bottom: 100,
			height: 100,
			left: 100,
			right: 300,
			toJSON: vi.fn<() => unknown>(),
			top: 0,
			width: 200,
			x: 100,
			y: 0,
		});
		groupElement.append(leadingPanel, separator, trailingPanel);
		result.current.groupElementRef.current = groupElement;

		expect(result.current.measureAvailablePanelSize()).toBe(expected);
	});

	it('exposes guarded primitive layout reads and writes', () => {
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() =>
			useResizablePanelGroup({
				canSynchronizeLayout: () => true,
				childIds,
				modelLayout,
				orientation: 'horizontal',
			}),
		);
		result.current.groupRef.current = {
			getLayout: () => modelLayout,
			setLayout,
		};

		expect(result.current.readLayout()).toEqual(modelLayout);
		expect(result.current.writeLayout({ 'panel-1': 45, 'panel-2': 55 })).toBe(true);
		expect(setLayout).toHaveBeenCalledWith({ 'panel-1': 45, 'panel-2': 55 });
	});
});
