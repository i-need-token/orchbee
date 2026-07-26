import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { PanelSplit } from '../../../../../../model/panel-workspace-state';
import { useAdjacentPanelResize } from '../useAdjacentPanelResize';

const split: PanelSplit = {
	children: [
		{
			id: 'panel-1',
			node: {
				activeChildTerminalId: null,
				childTerminals: [],
				id: 'panel-1',
				kind: 'panel',
				primaryTerminal: {
					cwd: null,
					id: 'terminal-1',
					phase: 'running',
					title: 'Terminal 1',
				},
			},
			weight: 50,
		},
		{
			id: 'panel-2',
			node: {
				activeChildTerminalId: null,
				childTerminals: [],
				id: 'panel-2',
				kind: 'panel',
				primaryTerminal: {
					cwd: null,
					id: 'terminal-2',
					phase: 'running',
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
const onResize = vi.fn<(splitId: string, separatorIndex: number, leadingWeight: number) => void>();

function HookProjection({ value = split }: { readonly value?: PanelSplit }) {
	const resize = useAdjacentPanelResize(value, onResize, 30);
	return createElement('div', null, JSON.stringify({ layout: resize.modelLayout }));
}

describe('useAdjacentPanelResize', () => {
	it('prepares the controlled layout from the semantic split', () => {
		expect(renderToStaticMarkup(createElement(HookProjection))).toContain(
			'&quot;layout&quot;:{&quot;split-1--panel-1&quot;:50,&quot;split-1--panel-2&quot;:50}',
		);
	});

	it('prepares the model layout when a same-axis split adds a child', () => {
		const firstChild = split.children[0];
		const secondChild = split.children[1];
		if (firstChild === undefined || secondChild === undefined) {
			throw new Error('Expected the two-panel split fixture.');
		}
		const threePanelSplit: PanelSplit = {
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
						activeChildTerminalId: null,
						childTerminals: [],
						id: 'panel-3',
						kind: 'panel',
						primaryTerminal: {
							cwd: null,
							id: 'terminal-3',
							phase: 'running',
							title: 'Terminal 3',
						},
					},
					weight: 25,
				},
			],
		};
		const threePanelMarkup = renderToStaticMarkup(
			createElement(HookProjection, { value: threePanelSplit }),
		);

		expect(threePanelMarkup).toContain(
			'&quot;layout&quot;:{&quot;split-1--panel-1&quot;:50,&quot;split-1--panel-2&quot;:25,&quot;split-1--panel-3&quot;:25}',
		);
	});
});
