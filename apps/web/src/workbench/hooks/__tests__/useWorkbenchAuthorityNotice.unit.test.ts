// @vitest-environment jsdom

import type { ActionResult } from '@orchbee/actions';
import { OrchbeeI18nProvider, workbenchMessages } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import {
	activeTerminalContext,
	createSnapshotRuntime,
	FIXTURE_INTL,
	fixtureContextId,
	fixtureDirectoryId,
	fixtureTabId,
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
	WorkbenchAuthorityClosure,
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
 * The window shows Workspace 1 of 1 and 2 and closes `closure` through an authority the test drives:
 * `leave` takes Workspace 1 out as its accepted close does, or for a Tab close only says the Tab
 * left, and `settle` ends the close.
 */
function closeShown(
	closure: WorkbenchAuthorityClosure = {
		type: 'workspace-close',
		workspaceId: fixtureWorkspaceId(1),
	},
) {
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
		closing = rendered.result.current.authority.close(context, closure);
	});
	return {
		context,
		leave: () =>
			act(() => {
				if (closure.type === 'workspace-close')
					publishSnapshot(runtime, workspacesSnapshot(2, [2]));
				markLeft?.();
			}),
		/** The title the window gave the first Tab of Workspace 1. */
		tabLabel: activeTerminalContext(runtime).tabModel.getSnapshot().tabs[0]?.title ?? '',
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
		const close = closeShown();
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
		const close = closeShown();
		close.leave();
		expect(close.notice()).toEqual({ message: CLOSED, persistent: false });

		await close.settle({ failure: { code: 'cleanup-blocked' }, kind: 'failed' });
		expect(close.notice()).toEqual({ message: STOPPED, persistent: true });
	});

	it('keeps one Tab close off the status line unless it still runs a second after it began', async () => {
		const closeTab = () =>
			closeShown({
				tabIds: [fixtureTabId(1)],
				terminalContextId: fixtureContextId(1),
				type: 'tabs-close',
			});
		const quick = closeTab();
		act(() => vi.advanceTimersByTime(20));
		quick.leave();
		act(() => vi.advanceTimersByTime(700));
		expect(quick.notice()).toBeNull();
		await quick.settle({ kind: 'completed' });
		act(() => vi.advanceTimersByTime(10_000));
		expect(quick.notice()).toBeNull();
		cleanup();

		const slow = closeTab();
		const closing = {
			message: FIXTURE_INTL.formatMessage(workbenchMessages.tabCloseClosing, {
				label: slow.tabLabel,
			}),
			persistent: false,
		};
		act(() => vi.advanceTimersByTime(20));
		slow.leave();
		act(() => vi.advanceTimersByTime(979));
		expect(slow.notice()).toBeNull();
		act(() => vi.advanceTimersByTime(1));
		expect(slow.notice()).toEqual(closing);
		await slow.settle({ kind: 'completed' });
		expect(slow.notice()).toBeNull();
		cleanup();

		// A Tab still in the strip, such as one whose stopped close a new Close joined, says nothing yet.
		const held = closeTab();
		act(() => vi.advanceTimersByTime(1_500));
		expect(held.notice()).toBeNull();
		held.leave();
		expect(held.notice()).toEqual(closing);
		await held.settle({ kind: 'completed' });
	});

	it.each(['cleanup-blocked', 'awaiting-removal', 'outcome-unknown', 'status-unavailable'])(
		'says one sentence naming the Tab, until a press, when its close ends at %s',
		async (code) => {
			const close = closeShown({
				tabIds: [fixtureTabId(1)],
				terminalContextId: fixtureContextId(1),
				type: 'tabs-close',
			});
			close.leave();
			await close.settle({ failure: { code }, kind: 'failed' });
			expect(close.notice()).toEqual({
				message: FIXTURE_INTL.formatMessage(workbenchMessages.tabCloseFailed, {
					label: close.tabLabel,
				}),
				persistent: true,
			});
		},
	);
});
