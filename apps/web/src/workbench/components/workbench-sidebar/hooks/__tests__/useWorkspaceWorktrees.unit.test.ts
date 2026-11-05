// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import type { WorkbenchWorktreeDiscoveryResult } from '../../../../types/workbench-worktrees';
import type { WorkbenchWorktreeCommands } from '../../types/workbench-sidebar-types';
import { useWorkspaceWorktrees } from '../useWorkspaceWorktrees';
import type { WorkbenchSidebarWorkspaceLocation } from '../useWorkbenchSidebarWorkspaceLocations';

const WORKSPACE = 'workspace-1';

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

function rejected(): never {
	throw new Error('The test runs no Worktree command.');
}

function location(
	line: WorkbenchSidebarWorkspaceLocation['line'],
): ReadonlyMap<string, WorkbenchSidebarWorkspaceLocation> {
	return new Map([
		[
			WORKSPACE,
			{ ahead: 0, behind: 0, directory: null, divergenceLabel: null, heldName: null, line },
		],
	]);
}

const NOT_REPOSITORY: WorkbenchWorktreeDiscoveryResult = {
	kind: 'failed',
	reason: 'not-repository',
	stoppedRemoval: null,
};
const TIMED_OUT: WorkbenchWorktreeDiscoveryResult = {
	kind: 'failed',
	reason: 'unavailable',
	stoppedRemoval: null,
};
const GIT: WorkbenchWorktreeDiscoveryResult = {
	discovery: { checkouts: [], removal: null, workspaceId: WORKSPACE },
	kind: 'discovered',
	stoppedRemoval: null,
};

/** Each read waits until the test answers it. */
function renderWorktrees(initial: ReadonlyMap<string, WorkbenchSidebarWorkspaceLocation>) {
	const answers: ((result: WorkbenchWorktreeDiscoveryResult) => void)[] = [];
	const commands: WorkbenchWorktreeCommands = {
		create: rejected,
		discover: () =>
			new Promise((resolve) => {
				answers.push(resolve);
			}),
		open: rejected,
		remove: rejected,
		resume: rejected,
	};
	const rendered = renderHook(
		({ locations }) =>
			useWorkspaceWorktrees({
				beginWorkspaceActivation: () => () => undefined,
				commands,
				locations,
			}),
		{ initialProps: { locations: initial }, wrapper: I18nWrapper },
	);
	/** Whether the Workspace menu and the Project menu each show Worktree rows now. */
	const shown = () => {
		const { menu, projectRows } = rendered.result.current;
		return {
			project: projectRows(WORKSPACE).create !== null,
			workspace: menu(WORKSPACE).section.length > 0,
		};
	};
	const read = async (result: WorkbenchWorktreeDiscoveryResult) => {
		act(() => rendered.result.current.refresh(WORKSPACE));
		const during = shown();
		await act(async () => answers.at(-1)?.(result));
		return { after: shown(), during };
	};
	return { read, rerender: rendered.rerender, shown };
}

afterEach(() => cleanup());

describe('useWorkspaceWorktrees', () => {
	const hidden = { project: false, workspace: false };
	const visible = { project: true, workspace: true };

	it('never shows Worktree rows for a folder outside Git, before, during, or after its read', async () => {
		const worktrees = renderWorktrees(location({ kind: 'folder', text: 'C:\\notes' }));

		expect(worktrees.shown()).toEqual(hidden);
		await expect(worktrees.read(NOT_REPOSITORY)).resolves.toEqual({
			after: hidden,
			during: hidden,
		});
		await expect(worktrees.read(TIMED_OUT)).resolves.toEqual({ after: hidden, during: hidden });
	});

	it('shows the rows once Git is known there, and hides them again after a failed read', async () => {
		const worktrees = renderWorktrees(location({ kind: 'folder', text: 'C:\\notes' }));
		await worktrees.read(NOT_REPOSITORY);

		// The terminal changed into a repository, and Git named its branch.
		worktrees.rerender({ locations: location({ kind: 'branch', text: 'main' }) });
		expect(worktrees.shown()).toEqual(visible);
		await expect(worktrees.read(TIMED_OUT)).resolves.toEqual({ after: hidden, during: visible });

		// A detached checkout shows no branch, so only a read that finds Git shows the rows.
		worktrees.rerender({ locations: location({ kind: 'folder', text: 'C:\\repo' }) });
		expect(worktrees.shown()).toEqual(hidden);
		await expect(worktrees.read(GIT)).resolves.toEqual({ after: visible, during: hidden });
	});
});
