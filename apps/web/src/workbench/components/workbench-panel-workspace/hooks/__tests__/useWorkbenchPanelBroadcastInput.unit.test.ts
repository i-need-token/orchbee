// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { createElement, createRef, startTransition, Suspense, useLayoutEffect } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	createSnapshotRuntime,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
} from '../../../../../__tests__/server-workbench-snapshot';
import type {
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputResult,
	WorkbenchTerminalBroadcastInputSink,
} from '../../../../types/workbench-terminal-surface';
import type { PanelWorkspaceStore } from '../../model/PanelWorkspaceStore';
import { collectPanelLeaves, getDisplayedTerminal } from '../../model/panel-workspace-state';
import { useWorkbenchPanelBroadcastInput } from '../useWorkbenchPanelBroadcastInput';

afterEach(() => cleanup());

describe('useWorkbenchPanelBroadcastInput', () => {
	it('withdraws a sink whose readiness subscription could not be acquired', () => {
		const fixture = renderBroadcast();
		const secondary = fixture.bind(fixture.secondaryId, readySink());
		const failure = new Error('Readiness subscription failed');
		expect(() =>
			fixture.bind(
				fixture.primaryId,
				readySink(() => {
					throw failure;
				}),
			),
		).toThrow(failure);
		expect(fixture.rendered.result.current.canStart(fixture.tab.focusedPanelId)).toBe(false);
		act(() => {
			expect(fixture.rendered.result.current.start(fixture.tab.focusedPanelId)).toBe(false);
		});
		const primary = fixture.bind(fixture.primaryId, readySink());
		expect(fixture.rendered.result.current.canStart(fixture.tab.focusedPanelId)).toBe(true);
		act(() => {
			primary.dispose();
			secondary.dispose();
		});
	});

	it('preserves a newer sink installed by a subscriber that then fails', () => {
		const fixture = renderBroadcast();
		const secondary = fixture.bind(fixture.secondaryId, readySink());
		let replacement: { dispose(): void } | undefined;
		const failure = new Error('Older subscription failed');
		expect(() =>
			fixture.bind(
				fixture.primaryId,
				readySink(() => {
					replacement = fixture.bind(fixture.primaryId, readySink());
					throw failure;
				}),
			),
		).toThrow(failure);
		expect(replacement).toBeDefined();
		expect(fixture.rendered.result.current.canStart(fixture.tab.focusedPanelId)).toBe(true);
		act(() => {
			replacement?.dispose();
			secondary.dispose();
		});
	});

	it.each([false, true])(
		'retires the sink before releasing its subscription once (release throws: %s)',
		(throws) => {
			const fixture = renderBroadcast();
			const secondary = fixture.bind(fixture.secondaryId, readySink());
			const attemptedStarts: boolean[] = [];
			const failure = new Error('Subscription release failed');
			const dispose = vi.fn<() => void>(() => {
				attemptedStarts.push(fixture.rendered.result.current.start(fixture.tab.focusedPanelId));
				if (throws) throw failure;
			});
			const primary = fixture.bind(
				fixture.primaryId,
				readySink(() => ({ dispose })),
			);
			expect(fixture.rendered.result.current.canStart(fixture.tab.focusedPanelId)).toBe(true);
			let releaseFailure: unknown;
			act(() => {
				try {
					primary.dispose();
				} catch (error) {
					releaseFailure = error;
				}
			});
			expect(releaseFailure).toBe(throws ? failure : undefined);
			expect(attemptedStarts).toEqual([false]);
			expect(fixture.rendered.result.current.canStart(fixture.tab.focusedPanelId)).toBe(false);
			act(() => {
				expect(() => primary.dispose()).not.toThrow();
				secondary.dispose();
			});
			expect(dispose).toHaveBeenCalledOnce();
		},
	);

	it('keeps a replacement installed while releasing an older registration', () => {
		const fixture = renderBroadcast();
		const secondary = fixture.bind(fixture.secondaryId, readySink());
		let replacement: { dispose(): void } | undefined;
		const primary = fixture.bind(
			fixture.primaryId,
			readySink(() => ({
				dispose: () => {
					replacement = fixture.bind(fixture.primaryId, readySink());
				},
			})),
		);
		act(() => primary.dispose());
		expect(replacement).toBeDefined();
		expect(fixture.rendered.result.current.canStart(fixture.tab.focusedPanelId)).toBe(true);
		act(() => {
			replacement?.dispose();
			secondary.dispose();
		});
	});

	it.each([false, true])(
		'restores source focus only while the ended session still owns it (reopened: %s)',
		async (reopened) => {
			const fixture = renderBroadcast();
			const primary = fixture.bind(fixture.primaryId, readySink());
			const secondary = fixture.bind(fixture.secondaryId, readySink());
			act(() => {
				expect(fixture.rendered.result.current.start(fixture.tab.focusedPanelId)).toBe(true);
				fixture.broadcastInput.focus();
			});
			expect(document.activeElement).toBe(fixture.broadcastInput);
			let restartAccepted: boolean | undefined;
			await act(async () => {
				fixture.rendered.result.current.end();
				if (reopened) {
					restartAccepted = fixture.rendered.result.current.start(fixture.tab.focusedPanelId);
				}
				await Promise.resolve();
			});
			expect(restartAccepted).toBe(reopened ? true : undefined);
			expect(document.activeElement).toBe(reopened ? fixture.broadcastInput : fixture.sourceInput);
			expect(fixture.rendered.result.current.state.kind).toBe(reopened ? 'active' : 'idle');
			act(() => {
				primary.dispose();
				secondary.dispose();
			});
		},
	);

	it('keeps an admitted session when an urgent render still projects idle', () => {
		const store = runningTwoPanelStore();
		const tab = store.getSnapshot().tabs[0];
		if (tab === undefined) throw new Error('Expected a two-Panel Tab.');
		const pending = new Promise<void>(() => undefined);
		const committedAvailability: boolean[] = [];
		const rendered = renderHook(
			({ revision }) => {
				const controller = useWorkbenchPanelBroadcastInput({
					activeTabId: store.getActiveTabId(),
					conflictingInteraction: false,
					store,
					workspaceReference: { current: null },
					workspaceState: store.getSnapshot(),
				});
				const { canStart } = controller;
				useLayoutEffect(() => {
					if (revision === 1) committedAvailability.push(canStart(tab.focusedPanelId));
				}, [canStart, revision]);
				if (controller.state.kind === 'active') throw pending;
				return controller;
			},
			{
				initialProps: { revision: 0 },
				wrapper: ({ children }) => createElement(Suspense, { fallback: null }, children),
			},
		);
		const registrations: Array<{ dispose(): void }> = [];
		act(() => {
			for (const panel of collectPanelLeaves(tab.root)) {
				registrations.push(
					rendered.result.current.bindSink(getDisplayedTerminal(panel).presentationId, {
						getReadiness: () => ({ kind: 'ready', revision: 'ready' }),
						send: async () => ({ kind: 'sent' }),
						subscribe: () => ({ dispose: () => undefined }),
					}),
				);
			}
		});
		let started = false;
		act(() =>
			startTransition(() => {
				started = rendered.result.current.start(tab.focusedPanelId);
			}),
		);
		expect(started).toBe(true);
		expect(rendered.result.current.state.kind).toBe('idle');
		act(() => rendered.rerender({ revision: 1 }));
		expect(committedAvailability).toEqual([false]);
		act(() => {
			for (const registration of registrations) registration.dispose();
		});
	});

	it('validates the current admitted session when an older projection commits', async () => {
		const store = runningTwoPanelStore();
		const tab = store.getSnapshot().tabs[0];
		if (tab === undefined) throw new Error('Expected a two-Panel Tab.');
		const pending = deferred<void>();
		let allowReplacement = false;
		let readinessRevision = 'initial';
		let notifyReadiness = (): void => undefined;
		const rendered = renderHook(
			() => {
				const controller = useWorkbenchPanelBroadcastInput({
					activeTabId: store.getActiveTabId(),
					conflictingInteraction: false,
					store,
					workspaceReference: { current: null },
					workspaceState: store.getSnapshot(),
				});
				if (
					controller.state.kind === 'active' &&
					controller.state.epoch === 2 &&
					!allowReplacement
				) {
					throw pending.promise;
				}
				return controller;
			},
			{ wrapper: ({ children }) => createElement(Suspense, { fallback: null }, children) },
		);
		const registrations: Array<{ dispose(): void }> = [];
		act(() => {
			for (const panel of collectPanelLeaves(tab.root)) {
				registrations.push(
					rendered.result.current.bindSink(getDisplayedTerminal(panel).presentationId, {
						getReadiness: () => ({ kind: 'ready', revision: readinessRevision }),
						send: async () => ({ kind: 'sent' }),
						subscribe: (listener) => {
							notifyReadiness = listener;
							return { dispose: () => undefined };
						},
					}),
				);
			}
			expect(rendered.result.current.start(tab.focusedPanelId)).toBe(true);
		});
		act(() =>
			startTransition(() => {
				rendered.result.current.end();
				readinessRevision = 'replacement';
				expect(rendered.result.current.start(tab.focusedPanelId)).toBe(true);
			}),
		);
		act(() => notifyReadiness());
		await act(async () => {
			allowReplacement = true;
			pending.resolve();
			await pending.promise;
		});
		expect(rendered.result.current.state).toMatchObject({ epoch: 2, kind: 'active' });
		act(() => {
			readinessRevision = 'changed-after-admission';
			notifyReadiness();
		});
		expect(rendered.result.current.state).toMatchObject({
			epoch: 2,
			kind: 'stopped',
			reason: 'target-invalidated',
		});
		act(() => {
			for (const registration of registrations) registration.dispose();
		});
	});

	it('serializes queued input through the captured sink revisions', async () => {
		const store = runningTwoPanelStore();
		const tab = store.getSnapshot().tabs[0];
		if (tab === undefined) throw new Error('Expected a two-Panel Tab.');
		const panels = collectPanelLeaves(tab.root);
		const presentations = panels.map((panel) => getDisplayedTerminal(panel).presentationId);
		const deliveries = presentations.map((presentationId) => {
			const first = deferred<WorkbenchTerminalBroadcastInputResult>();
			const revision = `${presentationId}:ready`;
			const send = vi
				.fn<
					(
						input: WorkbenchTerminalBroadcastInput,
						expectedRevision: string,
					) => Promise<WorkbenchTerminalBroadcastInputResult>
				>()
				.mockImplementationOnce(() => first.promise)
				.mockResolvedValue({ kind: 'sent' });
			const sink: WorkbenchTerminalBroadcastInputSink = {
				getReadiness: () => ({ kind: 'ready', revision }),
				send,
				subscribe: () => ({ dispose: () => undefined }),
			};
			return { first, revision, send, sink };
		});
		const workspaceElement = document.createElement('div');
		const rendered = renderHook(() =>
			useWorkbenchPanelBroadcastInput({
				activeTabId: store.getActiveTabId(),
				conflictingInteraction: false,
				store,
				workspaceReference: { current: workspaceElement },
				workspaceState: store.getSnapshot(),
			}),
		);
		const registrations: Array<{ dispose(): void }> = [];
		act(() => {
			for (const [index, presentationId] of presentations.entries()) {
				const delivery = deliveries[index];
				if (delivery === undefined) throw new Error('Expected a delivery sink.');
				registrations.push(rendered.result.current.bindSink(presentationId, delivery.sink));
			}
		});
		const sourcePanel = panels[0];
		if (sourcePanel === undefined) throw new Error('Expected a source Panel.');
		let started = false;
		act(() => {
			started = rendered.result.current.start(sourcePanel.id);
			rendered.result.current.enqueue({ kind: 'text', text: 'first' });
			rendered.result.current.enqueue({ kind: 'text', text: 'second' });
		});
		expect(started).toBe(true);
		await waitFor(() => {
			for (const delivery of deliveries) expect(delivery.send).toHaveBeenCalledOnce();
		});
		for (const delivery of deliveries) {
			expect(delivery.send).toHaveBeenCalledWith(
				{ kind: 'text', text: 'first' },
				delivery.revision,
			);
		}

		act(() => {
			for (const delivery of deliveries) delivery.first.resolve({ kind: 'sent' });
		});
		await waitFor(() => {
			for (const delivery of deliveries) expect(delivery.send).toHaveBeenCalledTimes(2);
		});
		for (const delivery of deliveries) {
			expect(delivery.send).toHaveBeenNthCalledWith(
				2,
				{ kind: 'text', text: 'second' },
				delivery.revision,
			);
		}
		await waitFor(() =>
			expect(rendered.result.current.state).toMatchObject({ inFlight: null, kind: 'active' }),
		);

		act(() => rendered.result.current.end());
		expect(rendered.result.current.state).toEqual({ kind: 'idle' });
		for (const registration of registrations) registration.dispose();
		rendered.unmount();
	});
});

function readySink(
	subscribe: WorkbenchTerminalBroadcastInputSink['subscribe'] = () => ({
		dispose: () => undefined,
	}),
): WorkbenchTerminalBroadcastInputSink {
	return {
		getReadiness: () => ({ kind: 'ready', revision: 'ready' }),
		send: async () => ({ kind: 'sent' }),
		subscribe,
	};
}

function renderBroadcast() {
	const store = runningTwoPanelStore();
	const tab = store.getSnapshot().tabs[0];
	if (tab === undefined) throw new Error('Expected a two-Panel Tab.');
	const [primary, secondary] = collectPanelLeaves(tab.root);
	if (primary === undefined || secondary === undefined) throw new Error('Expected two Panels.');
	const workspaceReference = createRef<HTMLDivElement>();
	const sourceReference = createRef<HTMLTextAreaElement>();
	const broadcastReference = createRef<HTMLInputElement>();
	const rendered = renderHook(
		() =>
			useWorkbenchPanelBroadcastInput({
				activeTabId: store.getActiveTabId(),
				conflictingInteraction: false,
				store,
				workspaceReference,
				workspaceState: store.getSnapshot(),
			}),
		{
			wrapper: ({ children }) =>
				createElement(
					'div',
					{ ref: workspaceReference },
					createElement(
						'div',
						{ 'data-presentation-mode': 'active' },
						createElement(
							'div',
							{
								'data-panel-id': tab.focusedPanelId,
								'data-slot': 'workbench-terminal-viewport',
							},
							createElement('textarea', { ref: sourceReference }),
						),
					),
					createElement('input', { ref: broadcastReference }),
					children,
				),
		},
	);
	const sourceInput = sourceReference.current;
	const broadcastInput = broadcastReference.current;
	if (sourceInput === null || broadcastInput === null) throw new Error('Expected mounted inputs.');
	return {
		bind(presentationId: TerminalPresentationId, sink: WorkbenchTerminalBroadcastInputSink) {
			let registration: { dispose(): void } | undefined;
			act(() => {
				registration = rendered.result.current.bindSink(presentationId, sink);
			});
			if (registration === undefined) throw new Error('Expected a sink registration.');
			return registration;
		},
		broadcastInput,
		primaryId: getDisplayedTerminal(primary).presentationId,
		rendered,
		secondaryId: getDisplayedTerminal(secondary).presentationId,
		sourceInput,
		tab,
	};
}

/** Two running Panels side by side, as the Server publishes them. */
function runningTwoPanelStore(): PanelWorkspaceStore {
	return createSnapshotRuntime(
		oneWorkspaceSnapshot(1, [
			snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
		]),
	).getActiveTerminalContext().panelWorkspaceStore;
}

function deferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise = (_value: Value): void => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
