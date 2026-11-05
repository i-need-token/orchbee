// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchAgentSnapshot } from '../../types/workbench-agent-observation';
import { useWorkbenchAgentAttention } from '../useWorkbenchAgentAttention';

const PRESENTATION_ID = fixturePresentationId(1);

function observation(
	sequence: bigint,
	presentationIds: readonly string[] = [PRESENTATION_ID],
): WorkbenchAgentSnapshot {
	return {
		complete: true,
		presentations: new Map(
			presentationIds.map((id) => [
				id,
				{ agent: 'codex', binding: 'binding', sequence, state: 'idle' },
			]),
		),
	};
}

/** Holds animation frames until `paint` delivers DOM observers and two frame boundaries. */
function stubFrames(): () => Promise<void> {
	const frames = new Map<number, FrameRequestCallback>();
	let nextFrame = 0;
	vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
		const id = ++nextFrame;
		frames.set(id, callback);
		return id;
	});
	vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
	return async () => {
		await act(async () => {});
		for (let index = 0; index < 2; index += 1) {
			act(() => {
				const pending = [...frames.values()];
				frames.clear();
				for (const callback of pending) callback(index);
			});
		}
	};
}

/** An Agent source whose observation the test replaces. */
function agentSource(initial: WorkbenchAgentSnapshot) {
	let snapshot = initial;
	const listeners = new Set<() => void>();
	return {
		getSnapshot: () => snapshot,
		publish: (next: WorkbenchAgentSnapshot) => {
			snapshot = next;
			for (const listener of listeners) listener();
		},
		subscribe: (listener: () => void) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
	};
}

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
	document.body.replaceChildren();
});

describe('Workbench Agent read visibility', () => {
	it('waits for a ready active terminal, then clears only the unread mark', async () => {
		const paint = stubFrames();
		vi.spyOn(document, 'hasFocus').mockReturnValue(true);
		const element = document.createElement('div');
		const layer = document.createElement('div');
		const viewport = document.createElement('div');
		layer.dataset['presentationMode'] = 'active';
		viewport.dataset['terminalPresentationId'] = PRESENTATION_ID;
		layer.append(viewport);
		element.append(layer);
		document.body.append(element);
		const root = { current: element };
		const terminalContext = activeTerminalContext(
			createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)])),
		);
		let snapshot = observation(1n);
		const listeners = new Set<() => void>();
		const source = {
			getSnapshot: () => snapshot,
			subscribe: (listener: () => void) => {
				listeners.add(listener);
				return () => {
					listeners.delete(listener);
				};
			},
		};
		const painted = vi.fn<(shown: WorkbenchAgentSnapshot, ids: ReadonlySet<string>) => void>();
		const rendered = renderHook(() =>
			useWorkbenchAgentAttention(source, terminalContext, root, painted),
		);
		act(() => {
			snapshot = observation(2n);
			for (const listener of listeners) listener();
		});
		await paint();
		expect(rendered.result.current.unread.size).toBe(1);
		layer.dataset['presentationMode'] = 'incoming';
		viewport.dataset['terminalDisplayReady'] = 'true';
		await paint();
		expect(rendered.result.current.unread.size).toBe(1);
		expect(painted).toHaveBeenLastCalledWith(snapshot, new Set());
		layer.dataset['presentationMode'] = 'active';
		await paint();
		expect(rendered.result.current.unread.size).toBe(0);
		expect(painted).toHaveBeenLastCalledWith(snapshot, new Set([PRESENTATION_ID]));
		expect(rendered.result.current.observation).toBe(snapshot);
	});

	it('counts only the focused Panel as seen; a visible split sibling stays unread', async () => {
		const paint = stubFrames();
		vi.spyOn(document, 'hasFocus').mockReturnValue(true);
		const [left, right] = [fixturePresentationId(1), fixturePresentationId(2)];
		const element = document.createElement('div');
		for (const id of [left, right]) {
			const layer = document.createElement('div');
			const viewport = document.createElement('div');
			layer.dataset['presentationMode'] = 'active';
			viewport.dataset['terminalPresentationId'] = id;
			viewport.dataset['terminalDisplayReady'] = 'true';
			layer.append(viewport);
			element.append(layer);
		}
		document.body.append(element);
		const terminalContext = activeTerminalContext(
			createSnapshotRuntime(
				oneWorkspaceSnapshot(1, [
					snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
				]),
			),
		);
		const focus = (sequence: number) =>
			act(() => {
				terminalContext.panelWorkspaceStore.transition({
					panelId: fixturePanelId(sequence),
					tabId: fixtureTabId(1),
					type: 'panel-focused',
				});
			});
		focus(1);
		const source = agentSource(observation(1n, [left, right]));
		const painted = vi.fn<(shown: WorkbenchAgentSnapshot, ids: ReadonlySet<string>) => void>();
		const rendered = renderHook(() =>
			useWorkbenchAgentAttention(source, terminalContext, { current: element }, painted),
		);

		act(() => source.publish(observation(2n, [left, right])));
		await paint();
		expect(rendered.result.current.unread).toEqual(new Set([right]));
		// Both painted, so neither needs a notification.
		expect(painted).toHaveBeenLastCalledWith(source.getSnapshot(), new Set([left, right]));

		focus(2);
		await paint();
		expect(rendered.result.current.unread.size).toBe(0);
	});
});
