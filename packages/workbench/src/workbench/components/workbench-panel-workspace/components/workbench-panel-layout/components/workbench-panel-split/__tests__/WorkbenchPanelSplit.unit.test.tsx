import type { ReactNode } from 'react';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { PanelSplit } from '../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../types/panel-workspace-view';
import { WorkbenchPanelSplit } from '../WorkbenchPanelSplit';

vi.mock('@orchbee/ui/components', async () => {
	const { createElement: createMockElement } = await import('react');

	return {
		ResizableHandle: ({ id }: { readonly id?: string }) =>
			createMockElement('div', { 'data-id': id, 'data-mock': 'handle' }),
		ResizablePanel: ({
			children,
			defaultSize,
			minSize,
		}: {
			readonly children?: ReactNode;
			readonly defaultSize?: number | string;
			readonly minSize?: number | string;
		}) =>
			createMockElement(
				'div',
				{
					'data-default-size': defaultSize,
					'data-min-size': minSize,
					'data-mock': 'panel',
				},
				children,
			),
		ResizablePanelGroup: ({ children }: { readonly children?: ReactNode }) =>
			createMockElement('div', { 'data-mock': 'group' }, children),
	};
});

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

describe('WorkbenchPanelSplit', () => {
	it('leaves split creation unconstrained while the resize controller owns the pixel minimum', () => {
		const view = {
			resizeAdjacentPanels:
				vi.fn<(splitId: string, separatorIndex: number, leadingWeight: number) => void>(),
		} as unknown as WorkbenchPanelWorkspaceView;
		const markup = renderToStaticMarkup(
			createElement(WorkbenchPanelSplit, {
				renderNode: () => createElement('span'),
				split,
				view,
			}),
		);

		expect(markup.match(/data-mock="panel"/g)).toHaveLength(2);
		expect(markup).toContain('data-id="split-1--panel-1--panel-2"');
		expect(markup).not.toContain('data-default-size');
		expect(markup).not.toContain('data-min-size');
	});
});
