// @vitest-environment jsdom

import { AgentRuntimeSnapshotSchema } from '@orchbee/agent';
import { TabModel } from '@orchbee/tab-strip';
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { PanelWorkspaceStore } from '../../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { useWorkbenchAgentAttention } from '../useWorkbenchAgentAttention';

function observation(generation: number) {
	return AgentRuntimeSnapshotSchema.parse({
		revision: generation,
		sessions: [
			{
				activityHint: null,
				capabilities: ['sessionLifecycle', 'turnLifecycle'],
				executionLocation: { executionLocationId: 'execution' },
				lastOutcome: null,
				lifecycle: 'live',
				observation: {
					fidelity: 'rich',
					freshness: 'fresh',
					generation,
					sequence: 1,
					source: 'hook',
				},
				parentSessionId: null,
				pendingRequests: [],
				presentationAttachments: [
					{
						generation: 'binding',
						navigable: true,
						presentationId: 'terminal',
						visibility: 'visible',
					},
				],
				providerId: 'codex',
				sessionId: 'session',
				turn: { kind: 'idle' },
			},
		],
	});
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
		let snapshot = observation(0);
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
		const rendered = renderHook(() => useWorkbenchAgentAttention(source, panels, tabModel, root));
		act(() => {
			snapshot = observation(1);
			for (const listener of listeners) listener();
		});
		await paint();
		expect(rendered.result.current.unread.size).toBe(1);
		layer.dataset['presentationMode'] = 'incoming';
		viewport.dataset['terminalDisplayReady'] = 'true';
		await paint();
		expect(rendered.result.current.unread.size).toBe(1);
		layer.dataset['presentationMode'] = 'active';
		await paint();
		expect(rendered.result.current.unread.size).toBe(0);
		expect(rendered.result.current.observation).toBe(snapshot);
		expect(snapshot.sessions[0]?.turn).toEqual({ kind: 'idle' });
	});
});
