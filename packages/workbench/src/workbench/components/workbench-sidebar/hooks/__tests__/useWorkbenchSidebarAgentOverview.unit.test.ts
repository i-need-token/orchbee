// @vitest-environment jsdom

import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { useWorkbenchSidebarAgentOverview } from '../useWorkbenchSidebarAgentOverview';

const representedWorkspaces = [
	{ projectId: 'project-a', workspaceId: 'workspace-a' },
	{ projectId: 'project-b', workspaceId: 'workspace-b' },
];

describe('useWorkbenchSidebarAgentOverview', () => {
	it('keeps one process-local disclosure and clears invalidated detail', async () => {
		const rendered = renderHook(
			({ expandedProjectIds, workspaces }) =>
				useWorkbenchSidebarAgentOverview({ expandedProjectIds, workspaces }),
			{
				initialProps: {
					expandedProjectIds: ['project-a', 'project-b'],
					workspaces: representedWorkspaces,
				},
			},
		);

		expect(rendered.result.current.expandedWorkspace).toBeNull();
		act(() => rendered.result.current.toggleWorkspace('workspace-a'));
		expect(rendered.result.current.expandedWorkspace).toEqual({
			workspaceId: 'workspace-a',
		});
		act(() => rendered.result.current.toggleWorkspace('workspace-b'));
		expect(rendered.result.current.expandedWorkspace).toEqual({
			workspaceId: 'workspace-b',
		});

		rendered.rerender({
			expandedProjectIds: ['project-a'],
			workspaces: representedWorkspaces,
		});
		expect(rendered.result.current.expandedWorkspace).toEqual({ workspaceId: 'workspace-b' });

		act(() => rendered.result.current.toggleWorkspace('workspace-a'));
		rendered.rerender({ expandedProjectIds: ['project-a'], workspaces: [] });
		await waitFor(() => expect(rendered.result.current.expandedWorkspace).toBeNull());
	});
});
