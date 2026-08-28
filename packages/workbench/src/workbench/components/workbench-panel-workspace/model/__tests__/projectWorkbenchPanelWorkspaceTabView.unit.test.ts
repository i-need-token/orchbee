import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../../../terminal-presentation';
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
				launchCwd: null,
				lifecycle: { kind: 'preview' },
				naturalTitle: null,
				presentationId: terminalPresentationId(`${tabId}-presentation`),
				title: tabId,
			},
		},
		tabId,
	};
}

describe('projectWorkbenchPanelWorkspaceTabView', () => {
	it('rebinds layout state and clears drag projection for a retained Tab', () => {
		const active = tab('active');
		const retained = tab('retained');
		const view = {
			activeTab: active,
			panelDrag: {
				draggingPanelId: 'active-panel',
				dropTarget: { direction: 'right', panelId: 'active-panel' },
				ghost: {},
				previewRoot: active.root,
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

		const projected = projectWorkbenchPanelWorkspaceTabView(view, retained);

		expect(projected).toMatchObject({
			activeTab: retained,
			displayedRoot: retained.root,
			maximizedPanelId: null,
			panelCount: 1,
			panelDrag: {
				draggingPanelId: null,
				dropTarget: null,
				ghost: null,
				previewRoot: null,
			},
			panelVisualSelection: null,
		});
		expect(projectWorkbenchPanelWorkspaceTabView(view, active)).toBe(view);
	});
});
