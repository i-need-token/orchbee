// @vitest-environment jsdom

import { act, cleanup, fireEvent, renderHook } from '@testing-library/react';
import { createElement, useLayoutEffect } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { PanelSplit } from '../../../../../../model/panel-workspace-state';
import type { SharedSplitResize } from '../../../../../../types/panel-workspace-view';
import { useAdjacentPanelResize } from '../useAdjacentPanelResize';

const split: PanelSplit = {
	children: [
		{
			id: 'panel-1',
			node: {
				activeChildPresentationId: null,
				childPresentations: [],
				id: 'panel-1',
				kind: 'panel',
				primaryPresentation: {
					closeActivity: 'unknown',
					currentCwd: null,
					lifecycle: { kind: 'running' },
					naturalTitle: null,
					presentationId: terminalPresentationId('terminal-1'),
					title: 'Terminal 1',
				},
			},
			weight: 50,
		},
		{
			id: 'panel-2',
			node: {
				activeChildPresentationId: null,
				childPresentations: [],
				id: 'panel-2',
				kind: 'panel',
				primaryPresentation: {
					closeActivity: 'unknown',
					currentCwd: null,
					lifecycle: { kind: 'running' },
					naturalTitle: null,
					presentationId: terminalPresentationId('terminal-2'),
					title: 'Terminal 2',
				},
			},
			weight: 50,
		},
	],
	id: 'split-1',
	kind: 'split',
	orientation: 'horizontal',
};
const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
/** No share floor beyond the pixel minimum, so each case shows what the pixels alone allow. */
const sharedResize: SharedSplitResize = { commit, minimumChildPercent: 0 };

function createThreePanelSplit(): PanelSplit {
	const firstChild = split.children[0];
	const secondChild = split.children[1];
	if (firstChild === undefined || secondChild === undefined) {
		throw new Error('Expected the two-panel split fixture.');
	}

	return {
		...split,
		children: [
			{
				...firstChild,
				weight: 50,
			},
			{
				...secondChild,
				weight: 25,
			},
			{
				id: 'panel-3',
				node: {
					activeChildPresentationId: null,
					childPresentations: [],
					id: 'panel-3',
					kind: 'panel',
					primaryPresentation: {
						closeActivity: 'unknown',
						currentCwd: null,
						lifecycle: { kind: 'running' },
						naturalTitle: null,
						presentationId: terminalPresentationId('terminal-3'),
						title: 'Terminal 3',
					},
				},
				weight: 25,
			},
		],
	};
}

function createPanelGroupElement(panelWidths: readonly number[]): HTMLDivElement {
	const groupElement = document.createElement('div');
	for (const [index, width] of panelWidths.entries()) {
		const panelElement = document.createElement('div');
		panelElement.dataset['slot'] = 'resizable-panel';
		vi.spyOn(panelElement, 'getBoundingClientRect').mockReturnValue({
			bottom: 100,
			height: 100,
			left: panelWidths.slice(0, index).reduce((total, candidate) => total + candidate, 0),
			right: panelWidths.slice(0, index + 1).reduce((total, candidate) => total + candidate, 0),
			toJSON: vi.fn<() => unknown>(),
			top: 0,
			width,
			x: 0,
			y: 0,
		});
		groupElement.append(panelElement);
	}
	return groupElement;
}

function firePointerCompletion(type: string, pointerId: number): void {
	switch (type) {
		case 'lostpointercapture':
			fireEvent.lostPointerCapture(document, { pointerId });
			break;
		case 'pointercancel':
			fireEvent.pointerCancel(document, { pointerId });
			break;
		case 'pointerup':
			fireEvent.pointerUp(document, { pointerId });
			break;
		default:
			throw new Error(`Unsupported pointer completion event: ${type}`);
	}
}

function HookProjection({
	maximizedChildId = null,
	value = split,
}: {
	readonly maximizedChildId?: string | null;
	readonly value?: PanelSplit;
}) {
	const resize = useAdjacentPanelResize(value, 30, maximizedChildId, sharedResize);
	return createElement('div', null, JSON.stringify({ layout: resize.displayLayout }));
}

afterEach(() => {
	cleanup();
	commit.mockClear();
});

describe('useAdjacentPanelResize', () => {
	it('prepares the controlled layout from the semantic split', () => {
		expect(renderToStaticMarkup(createElement(HookProjection))).toContain(
			'&quot;layout&quot;:{&quot;split-1--panel-1&quot;:50,&quot;split-1--panel-2&quot;:50}',
		);
	});

	it('projects maximization without changing the semantic split weights', () => {
		const markup = renderToStaticMarkup(
			createElement(HookProjection, { maximizedChildId: 'panel-2' }),
		);

		expect(markup).toContain(
			'&quot;layout&quot;:{&quot;split-1--panel-1&quot;:0,&quot;split-1--panel-2&quot;:100}',
		);
		expect(split.children.map((child) => child.weight)).toEqual([50, 50]);
	});

	it('synchronizes the projected layout on maximize and restores canonical weights on minimize', () => {
		let primitiveLayout: Record<string, number> = {
			'split-1--panel-1': 50,
			'split-1--panel-2': 50,
		};
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => {
				primitiveLayout = layout;
				return layout;
			},
		);
		const initialProps: { readonly maximizedChildId: string | null } = {
			maximizedChildId: null,
		};
		const { result, rerender } = renderHook(
			({ maximizedChildId }: { readonly maximizedChildId: string | null }) =>
				useAdjacentPanelResize(split, 30, maximizedChildId, sharedResize),
			{ initialProps },
		);
		result.current.groupRef.current = {
			getLayout: () => primitiveLayout,
			setLayout,
		};

		rerender({ maximizedChildId: 'panel-2' });
		expect(setLayout).toHaveBeenLastCalledWith({
			'split-1--panel-1': 0,
			'split-1--panel-2': 100,
		});

		rerender({ maximizedChildId: null });
		expect(setLayout).toHaveBeenLastCalledWith({
			'split-1--panel-1': 50,
			'split-1--panel-2': 50,
		});
		expect(commit).not.toHaveBeenCalled();
	});

	it('cancels an active resize before projecting maximization', () => {
		const initialProps: { readonly maximizedChildId: string | null } = {
			maximizedChildId: null,
		};
		const { result, rerender } = renderHook(
			({ maximizedChildId }: { readonly maximizedChildId: string | null }) =>
				useAdjacentPanelResize(split, 30, maximizedChildId, sharedResize),
			{ initialProps },
		);
		result.current.groupElementRef.current = createPanelGroupElement([500, 500]);
		result.current.groupRef.current = {
			getLayout: () => ({ 'split-1--panel-1': 50, 'split-1--panel-2': 50 }),
			setLayout: (layout) => layout,
		};

		act(() => {
			result.current.startKeyboardResize(0);
		});
		rerender({ maximizedChildId: 'panel-2' });
		act(() => {
			result.current.onLayoutChange({
				'split-1--panel-1': 0,
				'split-1--panel-2': 100,
			});
			result.current.finishKeyboardResize(0);
		});

		expect(commit).not.toHaveBeenCalled();
	});

	it('prepares the model layout when a same-axis split adds a child', () => {
		const threePanelSplit = createThreePanelSplit();
		const threePanelMarkup = renderToStaticMarkup(
			createElement(HookProjection, { value: threePanelSplit }),
		);

		expect(threePanelMarkup).toContain(
			'&quot;layout&quot;:{&quot;split-1--panel-1&quot;:50,&quot;split-1--panel-2&quot;:25,&quot;split-1--panel-3&quot;:25}',
		);
	});

	it('submits the accepted primitive layout once when the divider is released', () => {
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() => useAdjacentPanelResize(split, 30, null, sharedResize));
		result.current.groupElementRef.current = createPanelGroupElement([500, 500]);
		result.current.groupRef.current = {
			getLayout: () => ({ 'split-1--panel-1': 50, 'split-1--panel-2': 50 }),
			setLayout,
		};

		act(() => {
			result.current.startKeyboardResize(0);
			result.current.onLayoutChange({
				'split-1--panel-1': 60,
				'split-1--panel-2': 40,
			});
			result.current.onLayoutChange({
				'split-1--panel-1': 60,
				'split-1--panel-2': 40,
			});
		});

		expect(commit).not.toHaveBeenCalled();
		expect(setLayout).not.toHaveBeenCalled();

		act(() => {
			result.current.finishKeyboardResize(0);
		});

		expect(commit).toHaveBeenCalledExactlyOnceWith('split-1', 60);
	});

	it('submits a resize that returns to its initial layout', () => {
		const { result } = renderHook(() => useAdjacentPanelResize(split, 30, null, sharedResize));
		result.current.groupElementRef.current = createPanelGroupElement([500, 500]);
		result.current.groupRef.current = {
			getLayout: () => ({ 'split-1--panel-1': 50, 'split-1--panel-2': 50 }),
			setLayout: (layout) => layout,
		};

		act(() => {
			result.current.startKeyboardResize(0);
			result.current.onLayoutChange({
				'split-1--panel-1': 60,
				'split-1--panel-2': 40,
			});
			result.current.onLayoutChange({
				'split-1--panel-1': 50,
				'split-1--panel-2': 50,
			});
			result.current.finishKeyboardResize(0);
		});

		expect(commit).toHaveBeenCalledExactlyOnceWith('split-1', 50);
	});

	it('corrects a primitive layout at the measured pixel minimum', () => {
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() => useAdjacentPanelResize(split, 30, null, sharedResize));
		result.current.groupElementRef.current = createPanelGroupElement([500, 500]);
		result.current.groupRef.current = {
			getLayout: () => ({ 'split-1--panel-1': 50, 'split-1--panel-2': 50 }),
			setLayout,
		};

		act(() => {
			result.current.startKeyboardResize(0);
			result.current.onLayoutChange({
				'split-1--panel-1': 99,
				'split-1--panel-2': 1,
			});
			result.current.finishKeyboardResize(0);
		});

		expect(setLayout).toHaveBeenCalledWith({
			'split-1--panel-1': 97,
			'split-1--panel-2': 3,
		});
		expect(commit).toHaveBeenCalledWith('split-1', 97);
	});

	it('rejects a candidate from a separator outside the active resize session', () => {
		const threePanelSplit = createThreePanelSplit();
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() =>
			useAdjacentPanelResize(threePanelSplit, 30, null, sharedResize),
		);
		result.current.groupElementRef.current = createPanelGroupElement([500, 250, 250]);
		result.current.groupRef.current = {
			getLayout: () => ({
				'split-1--panel-1': 50,
				'split-1--panel-2': 25,
				'split-1--panel-3': 25,
			}),
			setLayout,
		};

		act(() => {
			result.current.startKeyboardResize(0);
			result.current.onLayoutChange({
				'split-1--panel-1': 50,
				'split-1--panel-2': 35,
				'split-1--panel-3': 15,
			});
		});

		expect(setLayout).not.toHaveBeenCalled();

		act(() => {
			result.current.finishKeyboardResize(0);
		});

		expect(commit).not.toHaveBeenCalled();
	});

	it.each([
		{ completionEvent: 'lostpointercapture', submitted: [] },
		{ completionEvent: 'pointercancel', submitted: [] },
		{ completionEvent: 'pointerup', submitted: [['split-1', 70]] },
	])(
		'keeps a pointer session active until its initiating pointer receives $completionEvent',
		({ completionEvent, submitted }) => {
			const groupElement = createPanelGroupElement([500, 500]);
			const { result } = renderHook(() => {
				const resize = useAdjacentPanelResize(split, 30, null, sharedResize);
				useLayoutEffect(() => {
					resize.groupElementRef.current = groupElement;
				}, [resize.groupElementRef]);
				return resize;
			});
			result.current.groupRef.current = {
				getLayout: () => ({ 'split-1--panel-1': 50, 'split-1--panel-2': 50 }),
				setLayout: (layout) => layout,
			};

			act(() => {
				result.current.startPointerResize(0, 7);
				result.current.onLayoutChange({
					'split-1--panel-1': 60,
					'split-1--panel-2': 40,
				});
			});
			act(() => {
				result.current.startPointerResize(0, 8);
			});
			fireEvent.pointerMove(document, { clientX: 900, pointerId: 8 });
			firePointerCompletion(completionEvent, 8);
			expect(commit).not.toHaveBeenCalled();
			act(() => {
				result.current.onLayoutChange({
					'split-1--panel-1': 70,
					'split-1--panel-2': 30,
				});
			});
			firePointerCompletion(completionEvent, 7);
			act(() => {
				result.current.onLayoutChange({
					'split-1--panel-1': 80,
					'split-1--panel-2': 20,
				});
			});

			expect(commit.mock.calls).toEqual(submitted);
		},
	);

	it('remeasures the active axis before applying each primitive candidate', () => {
		const setLayout = vi.fn<(layout: Record<string, number>) => Record<string, number>>(
			(layout) => layout,
		);
		const { result } = renderHook(() => useAdjacentPanelResize(split, 30, null, sharedResize));
		result.current.groupElementRef.current = createPanelGroupElement([500, 500]);
		result.current.groupRef.current = {
			getLayout: () => ({ 'split-1--panel-1': 50, 'split-1--panel-2': 50 }),
			setLayout,
		};

		act(() => {
			result.current.startKeyboardResize(0);
			result.current.onLayoutChange({
				'split-1--panel-1': 60,
				'split-1--panel-2': 40,
			});
		});
		result.current.groupElementRef.current = createPanelGroupElement([250, 250]);
		act(() => {
			result.current.onLayoutChange({
				'split-1--panel-1': 99,
				'split-1--panel-2': 1,
			});
			result.current.finishKeyboardResize(0);
		});

		expect(setLayout).toHaveBeenCalledOnce();
		expect(setLayout).toHaveBeenCalledWith({
			'split-1--panel-1': 94,
			'split-1--panel-2': 6,
		});
		expect(commit).toHaveBeenCalledExactlyOnceWith('split-1', 94);
	});
});
