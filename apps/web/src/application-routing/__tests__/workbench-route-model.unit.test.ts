import { describe, expect, it } from 'vitest';

import {
	canonicalWorkbenchRoute,
	resolveWorkbenchRoute,
	workbenchRoutePath,
	type WorkbenchRouteSnapshot,
} from '../workbench-route-model';

const snapshot: WorkbenchRouteSnapshot = {
	activeLocation: {
		kind: 'workspace',

		workspaceId: 'workspace-two',
	},
	projects: [{ id: 'empty-project' }, { id: 'project/two' }],
	workspaces: [
		{
			activeTabId: 'workspace-two-tab-2',
			id: 'workspace-two',
			projectId: 'project/two',
			tabIds: ['workspace-two-tab-1', 'workspace-two-tab-2'],
		},
		{ activeTabId: null, id: 'workspace-empty', projectId: 'project/two', tabIds: [] },
	],
	standalone: {
		activeTabId: 'standalone-tab-2',
		tabIds: ['standalone-tab-1', 'standalone-tab-2'],
	},
};

describe('Workbench route model', () => {
	it('builds one encoded path segment for every model identifier', () => {
		expect(
			workbenchRoutePath({
				kind: 'workspace',

				tabId: 'tab?#/two',
				workspaceId: 'workspace two',
			}),
		).toBe('/workspaces/workspace%20two/tabs/tab%3F%23%2Ftwo');
		expect(workbenchRoutePath({ kind: 'standalone', tabId: null })).toBe('/terminal');
	});

	it('derives the canonical route from the active Client View location and Tab', () => {
		expect(canonicalWorkbenchRoute(snapshot)).toEqual({
			kind: 'workspace',

			tabId: 'workspace-two-tab-2',
			workspaceId: 'workspace-two',
		});
	});

	it('canonicalizes the root and incomplete standalone routes with replace intent', () => {
		expect(resolveWorkbenchRoute(snapshot, { kind: 'root' })).toEqual({
			kind: 'redirect',
			location: canonicalWorkbenchRoute(snapshot),
		});
		expect(resolveWorkbenchRoute(snapshot, { kind: 'standalone' })).toEqual({
			kind: 'redirect',
			location: { kind: 'standalone', tabId: 'standalone-tab-1' },
		});
	});

	it('keeps an empty standalone context as a canonical displayed route', () => {
		const emptySnapshot = { ...snapshot, standalone: { activeTabId: null, tabIds: [] } };
		expect(resolveWorkbenchRoute(emptySnapshot, { kind: 'standalone' })).toEqual({
			kind: 'ready',
			location: { kind: 'standalone', tabId: null },
		});
	});

	it('selects the first navigable Project, Workspace, and Tab for /projects', () => {
		expect(resolveWorkbenchRoute(snapshot, { kind: 'projects' })).toEqual({
			kind: 'redirect',
			location: {
				kind: 'workspace',

				tabId: 'workspace-two-tab-1',
				workspaceId: 'workspace-two',
			},
		});
	});

	it('resolves an explicit Project through its first Workspace and Tab', () => {
		expect(resolveWorkbenchRoute(snapshot, { kind: 'project', projectId: 'project/two' })).toEqual({
			kind: 'redirect',
			location: {
				kind: 'workspace',

				tabId: 'workspace-two-tab-1',
				workspaceId: 'workspace-two',
			},
		});
	});

	it('keeps an empty Workspace as a canonical displayed route', () => {
		expect(
			resolveWorkbenchRoute(snapshot, {
				kind: 'workspace',

				workspaceId: 'workspace-empty',
			}),
		).toEqual({
			kind: 'ready',
			location: {
				kind: 'workspace',

				tabId: null,
				workspaceId: 'workspace-empty',
			},
		});
	});

	it('never treats an explicit stale identifier as an incomplete route', () => {
		expect(resolveWorkbenchRoute(snapshot, { kind: 'project', projectId: 'missing' })).toEqual({
			kind: 'unavailable',
			reason: 'project-not-found',
		});
		expect(
			resolveWorkbenchRoute(snapshot, {
				kind: 'workspace',

				workspaceId: 'missing',
			}),
		).toEqual({ kind: 'unavailable', reason: 'workspace-not-found' });
		expect(
			resolveWorkbenchRoute(snapshot, {
				kind: 'workspace',

				tabId: 'missing',
				workspaceId: 'workspace-two',
			}),
		).toEqual({ kind: 'unavailable', reason: 'tab-not-found' });
	});

	it('reports an explicit Project with no Workspace as unavailable', () => {
		expect(
			resolveWorkbenchRoute(snapshot, { kind: 'project', projectId: 'empty-project' }),
		).toEqual({ kind: 'unavailable', reason: 'workspace-not-found' });
	});
});
