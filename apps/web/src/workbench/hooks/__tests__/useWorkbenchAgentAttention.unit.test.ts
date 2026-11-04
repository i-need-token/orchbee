// @vitest-environment jsdom

import { TabModel } from '@orchbee/tab-strip';
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { PanelWorkspaceStore } from '../../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import type { WorkbenchAgentSnapshot } from '../../types/workbench-agent-observation';
import { useWorkbenchAgentAttention } from '../useWorkbenchAgentAttention';

function observation(sequence: bigint): WorkbenchAgentSnapshot {
	return {
		complete: true,
		presentations: new Map([
			['terminal', { agent: 'codex', binding: 'binding', sequence, state: 'idle' }],
		]),
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
		const frames = new Map<number, FrameRequestCallback>();
		let nextFrame = 0;
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			const id = ++nextFrame;
			frames.set(id, callback);
			return id;
		});
		vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
		vi.spyOn(document, 'hasFocus').mockReturnValue(true);
		const paint = async () => {
			// Deliver DOM observers, followed by the two browser frame boundaries.
			await act(async () => {});
			for (let index = 0; index < 2; index += 1) {
				act(() => {
					const pending = [...frames.values()];
					frames.clear();
					for (const callback of pending) callback(index);
				});
			}
		};
		const element = document.createElement('div');
		const layer = document.createElement('div');
		const viewport = document.createElement('div');
		layer.dataset['presentationMode'] = 'active';
		viewport.dataset['terminalPresentationId'] = 'terminal';
		layer.append(viewport);
		element.append(layer);
		document.body.append(element);
		const root = { current: element };
		const tabModel = new TabModel({
			activeTabId: 'tab',
			tabs: [{ group: null, id: 'tab', pinned: false, title: 'Terminal' }],
		});
		const panels = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId('next'),
				initialPresentationId: terminalPresentationId('terminal'),
				kind: 'preview',
			},
		});
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
			useWorkbenchAgentAttention(source, panels, tabModel, root, painted),
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
		expect(painted).toHaveBeenLastCalledWith(snapshot, new Set(['terminal']));
		expect(rendered.result.current.observation).toBe(snapshot);
	});
});
