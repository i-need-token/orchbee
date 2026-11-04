import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { TabPanelWorkspace } from '../panel-workspace-state';
import { projectWorkbenchPanelWorkspaceTabView } from '../projectWorkbenchPanelWorkspaceTabView';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';

function tab(tabId: string): TabPanelWorkspace {
	return {
		focusedPanelId: `${tabId}-panel`,
		maximizedPanelId: null,
		panelFocusHistory: [`${tabId}-panel`],
		root: {
			activeChildPresentationId: null,
			childPresentations: [],
			id: `${tabId}-panel`,
			kind: 'panel',
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: null,
				lifecycle: { kind: 'running' },
				naturalTitle: null,
				presentationId: terminalPresentationId(`${tabId}-presentation`),
				title: tabId,
			},
		},
		tabId,
	};
}

describe('projectWorkbenchPanelWorkspaceTabView', () => {
	it('rebinds layout state and shows a drag only in the layer of its own revision', () => {
		const active = tab('active');
		const retained = tab('retained');
		const view = {
			activeTab: active,
			panelDrag: {
				draggingPanelId: 'active-panel',
				dropTarget: { direction: 'right', panelId: 'active-panel' },
				ghost: {},
				previewRoot: active.root,
				sourceRevision: 'drag-source',
			},
			panelVisualSelection: {
				mode: 'focus',
				panelIds: ['active-panel', 'other-panel'],
				prefix: '',
				sourcePanelId: 'active-panel',
				tabId: 'active',
				targets: [{ label: '1', panelId: 'other-panel' }],
			},
		} as unknown as WorkbenchPanelWorkspaceView;

		const noDrag = {
			draggingPanelId: null,
			dropTarget: null,
			ghost: null,
			previewRoot: null,
			sourceRevision: null,
		};

		const projected = projectWorkbenchPanelWorkspaceTabView(view, retained, 'other-revision');

		expect(projected).toMatchObject({
			activeTab: retained,
			displayedRoot: retained.root,
			maximizedPanelId: null,
			panelCount: 1,
			panelDrag: noDrag,
			panelVisualSelection: null,
		});
		expect(projectWorkbenchPanelWorkspaceTabView(view, active, 'drag-source')).toBe(view);

		const retainedRevision = { ...active, maximizedPanelId: 'active-panel' };
		const projectedRevision = projectWorkbenchPanelWorkspaceTabView(
			view,
			retainedRevision,
			'other-revision',
		);
		expect(projectedRevision).not.toBe(view);
		expect(projectedRevision).toMatchObject({
			activeTab: retainedRevision,
			displayedRoot: retainedRevision.root,
			maximizedPanelId: 'active-panel',
			panelDrag: noDrag,
			panelVisualSelection: null,
		});

		// A released drop stays with the layer it was made in, including one that is being replaced.
		expect(projectWorkbenchPanelWorkspaceTabView(view, retained, 'drag-source').panelDrag).toBe(
			view.panelDrag,
		);
		// The Tab that replaced it starts from what the authority published.
		expect(projectWorkbenchPanelWorkspaceTabView(view, active, 'other-revision').panelDrag).toEqual(
			noDrag,
		);
	});
});
