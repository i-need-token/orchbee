// @vitest-environment jsdom

import type { ActionResult } from '@orchbee/actions';
import { OrchbeeI18nProvider, workbenchMessages } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import {
	createSnapshotRuntime,
	FIXTURE_INTL,
	fixtureDirectoryId,
	fixtureWorkspaceId,
	publishSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityCloseContext,
} from '../../types/workbench-authority';
import { useWorkbenchAuthorityNotice } from '../useWorkbenchAuthorityNotice';

const CLOSED = FIXTURE_INTL.formatMessage(workbenchMessages.workspaceCloseClosed, {
	label: 'Workspace 1',
});
const CLOSING = FIXTURE_INTL.formatMessage(workbenchMessages.workspaceCloseClosing, {
	label: 'Workspace 1',
});
const STOPPED = FIXTURE_INTL.formatMessage(workbenchMessages.workspaceCloseStopped, {
	label: 'Workspace 1',
});

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

/** Workspaces `sequences` in Recents, each in Directory 1 with one Tab numbered like it. */
function workspacesSnapshot(revision: number, sequences: readonly number[]) {
	return workbenchSnapshot(
		revision,
		{
			directories: [snapshotDirectory(1)],
			workspaces: sequences.map((sequence) =>
				snapshotWorkspace(sequence, { primaryDirectoryId: fixtureDirectoryId(1) }),
			),
		},
		sequences.map((sequence) => snapshotContext(sequence, [snapshotTab(sequence)])),
	);
}

/**
 * The window shows Workspace 1 of 1 and 2 and closes it through an authority the test drives: `leave`
 * takes Workspace 1 out as an accepted close does, and `settle` ends the close.
 */
function closeShownWorkspace() {
	let markLeft: (() => void) | undefined;
	let settle: ((result: ActionResult<void>) => void) | undefined;
	const authority = inertWorkbenchAuthority({
		close: (context) =>
			new Promise<ActionResult<void>>((resolve) => {
				markLeft = context.markLeft;
				settle = resolve;
			}),
	} satisfies Partial<WorkbenchAuthority>);
	const runtime = createSnapshotRuntime(workspacesSnapshot(1, [1, 2]));
	const rendered = renderHook(() => useWorkbenchAuthorityNotice(authority, runtime), {
		wrapper: I18nWrapper,
	});
	const context = {
		markCommitted: vi.fn<() => void>(),
		markLeft: vi.fn<() => void>(),
		signal: new AbortController().signal,
	} satisfies WorkbenchAuthorityCloseContext;
	let closing: Promise<ActionResult<void>> | undefined;
	act(() => {
		closing = rendered.result.current.authority.close(context, {
			type: 'workspace-close',
			workspaceId: fixtureWorkspaceId(1),
		});
	});
	return {
		context,
		leave: () =>
			act(() => {
				publishSnapshot(runtime, workspacesSnapshot(2, [2]));
				markLeft?.();
			}),
		notice: () => rendered.result.current.notice,
		publishAgain: () =>
			act(() => {
				publishSnapshot(runtime, workspacesSnapshot(3, [2]));
			}),
		settle: (result: ActionResult<void>) =>
			act(async () => {
				settle?.(result);
				await closing;
			}),
	};
}

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe('useWorkbenchAuthorityNotice', () => {
	it('says Closed once the closing Workspace leaves, never Closing after it, and only once', async () => {
		const close = closeShownWorkspace();
		act(() => vi.advanceTimersByTime(250));
		expect(close.notice()).toEqual({ message: CLOSING, persistent: false });

		close.leave();
		expect(close.notice()).toEqual({ message: CLOSED, persistent: false });
		expect(close.context.markLeft).toHaveBeenCalledOnce();

		close.publishAgain();
		act(() => vi.advanceTimersByTime(250));
		expect(close.notice()).toEqual({ message: CLOSED, persistent: false });

		// Closed is one result: it expires from when it was said, and settling says nothing more.
		act(() => vi.advanceTimersByTime(10_000));
		expect(close.notice()).toBeNull();
		await close.settle({ kind: 'completed' });
		expect(close.notice()).toBeNull();
	});

	it('replaces Closed with the report once a close whose Workspace left stops', async () => {
		const close = closeShownWorkspace();
		close.leave();
		expect(close.notice()).toEqual({ message: CLOSED, persistent: false });

		await close.settle({ failure: { code: 'cleanup-blocked' }, kind: 'failed' });
		expect(close.notice()).toEqual({ message: STOPPED, persistent: true });
	});
});
