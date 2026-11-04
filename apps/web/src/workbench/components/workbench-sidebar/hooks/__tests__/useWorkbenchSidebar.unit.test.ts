// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import {
	createSnapshotRuntime,
	fixtureProjectId,
	fixtureWorkspaceId,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../../../__tests__/server-workbench-snapshot';
import {
	createProjectWorkspaceState,
	type ProjectWorkspaceState,
} from '../../../../../project-workspace';
import { useWorkbenchSidebar } from '../useWorkbenchSidebar';

afterEach(cleanup);

describe('useWorkbenchSidebar', () => {
	it('prepares and clears a fresh Project creation draft', () => {
		const rendered = renderHook(() => useWorkbenchSidebar(createProjectWorkspaceState()));

		act(() => rendered.result.current.showProjectCreation());
		expect(rendered.result.current.projectCreation?.path).toBe('');
		act(() => rendered.result.current.projectCreation?.setPath('/workspace/orchbee'));
		expect(rendered.result.current.projectCreation?.path).toBe('/workspace/orchbee');
		act(() => rendered.result.current.projectCreation?.close());
		expect(rendered.result.current.projectCreation).toBeNull();
	});

	it('prepares and clears Project and Workspace rename drafts', () => {
		const state = projectState();
		const rendered = renderHook(
			({ snapshot }: { readonly snapshot: ProjectWorkspaceState }) => useWorkbenchSidebar(snapshot),
			{ initialProps: { snapshot: state } },
		);

		act(() => rendered.result.current.showProjectRename(fixtureProjectId(1)));
		expect(rendered.result.current.projectRename).toMatchObject({
			name: 'orchbee',
			project: { id: fixtureProjectId(1) },
		});
		act(() => rendered.result.current.projectRename?.setName('Orchbee'));
		expect(rendered.result.current.projectRename?.name).toBe('Orchbee');
		act(() => rendered.result.current.projectRename?.close());
		expect(rendered.result.current.projectRename).toBeNull();

		act(() => rendered.result.current.showWorkspaceRename(fixtureWorkspaceId(1)));
		expect(rendered.result.current.workspaceRename).toMatchObject({
			name: 'Primary',
			workspace: { id: fixtureWorkspaceId(1) },
		});
		act(() => rendered.result.current.workspaceRename?.setName('Feature'));
		expect(rendered.result.current.workspaceRename?.name).toBe('Feature');
		act(() => rendered.result.current.workspaceRename?.close());
		expect(rendered.result.current.workspaceRename).toBeNull();
	});
});

function projectState(): ProjectWorkspaceState {
	return createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [snapshotDirectory(1)],
				projects: [snapshotProject(1, 'orchbee')],
				workspaces: [snapshotWorkspace(1, { label: 'Primary', projectId: fixtureProjectId(1) })],
			},
			[snapshotContext(1, [snapshotTab(1)])],
		),
	).getSnapshot();
}
