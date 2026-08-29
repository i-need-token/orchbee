// @vitest-environment jsdom

import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useEffect, useMemo, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../../terminal-presentation';
import type {
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputResult,
	WorkbenchTerminalSurfaceProps,
} from '../../../types/workbench-terminal-surface';
import { registerWorkbenchActiveTerminalActions } from '../../../actions/registerWorkbenchActiveTerminalActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
} from '../../../actions/workbench-action-ids';
import { WorkbenchProjectWorkspaceRuntime } from '../../../model/WorkbenchProjectWorkspaceRuntime';
import { PanelWorkspaceStore } from '../model/PanelWorkspaceStore';
import {
	collectPanelLeaves,
	getDisplayedTerminal,
	type PanelLayoutNode,
	type PanelWorkspaceState,
} from '../model/panel-workspace-state';
import { WorkbenchPanelWorkspace } from '../WorkbenchPanelWorkspace';

const surfaces = new Map<string, WorkbenchTerminalSurfaceProps>();
const surfacesByMode = new Map<string, WorkbenchTerminalSurfaceProps>();
const broadcastInputs = new Map<string, WorkbenchTerminalBroadcastInput[]>();
const broadcastSendHandlers = new Map<
	string,
	(input: WorkbenchTerminalBroadcastInput) => Promise<WorkbenchTerminalBroadcastInputResult>
>();

function deferred<Value>(): {
	readonly promise: Promise<Value>;
	resolve(value: Value): void;
} {
	let resolvePromise: ((value: Value) => void) | undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return {
		promise,
		resolve: (value) => resolvePromise?.(value),
	};
}

class TestResizeObserver {
	public constructor(_callback: ResizeObserverCallback) {}
	public disconnect(): void {}
	public observe(_target: Element, _options?: ResizeObserverOptions): void {}
	public unobserve(_target: Element): void {}
}

function createBounds(left: number, width: number): DOMRect {
	return {
		bottom: 100,
		height: 100,
		left,
		right: left + width,
		toJSON: () => ({}),
		top: 0,
		width,
		x: left,
		y: 0,
	};
}

function TestTerminalSurface(properties: WorkbenchTerminalSurfaceProps): ReactNode {
	surfaces.set(properties.presentation.presentationId, properties);
	surfacesByMode.set(
		`${properties.presentation.presentationId}:${properties.presentationMode}`,
		properties,
	);
	const bindBroadcastInputSink = properties.bindBroadcastInputSink;
	const presentationId = properties.presentation.presentationId;
	const sink = useMemo(
		() => ({
			getReadiness: () => ({ kind: 'ready' as const, revision: `${presentationId}:ready` }),
			send: async (input: WorkbenchTerminalBroadcastInput) => {
				const inputs = broadcastInputs.get(presentationId) ?? [];
				inputs.push(input);
				broadcastInputs.set(presentationId, inputs);
				return (
					(await broadcastSendHandlers.get(presentationId)?.(input)) ?? { kind: 'sent' as const }
				);
			},
			subscribe: () => ({ dispose: () => undefined }),
		}),
		[presentationId],
	);
	useEffect(() => {
		const registration = bindBroadcastInputSink(sink);
		return () => registration.dispose();
	}, [bindBroadcastInputSink, sink]);
	return (
		<div
			data-auto-focus={properties.autoFocus}
			data-mode={properties.presentationMode}
			data-surface-presentation-id={properties.presentation.presentationId}
		/>
	);
}

function runningLayout(node: PanelLayoutNode): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			...node,
			childPresentations: node.childPresentations.map((terminal) => ({
				...terminal,
				lifecycle: { kind: 'running' },
			})),
			primaryPresentation: {
				...node.primaryPresentation,
				lifecycle: { kind: 'running' },
			},
		};
	}
	return {
		...node,
		children: node.children.map((child) => ({ ...child, node: runningLayout(child.node) })),
	};
}

function runningWorkspaceState(state: PanelWorkspaceState): PanelWorkspaceState {
	return {
		...state,
		tabs: state.tabs.map((tab) => ({ ...tab, root: runningLayout(tab.root) })),
	};
}

function renderThreePanelWorkspace() {
	const actionRuntime = new ActionRuntime();
	const parentActionScope = actionRuntime.createScope('test.workbench');
	let sequence = 1;
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace',
		standaloneTerminalTitle: 'Terminal A',
		terminalPresentation: {
			createPresentationId: () => terminalPresentationId(`terminal-visual-${sequence++}`),
			initialPresentationId: terminalPresentationId('terminal-visual-initial'),
			kind: 'preview',
		},
	});
	const panelWorkspaceStore =
		projectWorkspaceRuntime.getActiveTerminalContext().panelWorkspaceStore;
	const tabId = panelWorkspaceStore.getActiveTabId();
	if (tabId === null) throw new Error('Expected an active terminal Tab.');
	for (let index = 0; index < 2; index += 1) {
		const tab = panelWorkspaceStore
			.getSnapshot()
			.tabs.find((candidate) => candidate.tabId === tabId);
		if (tab === undefined) throw new Error('Expected the active terminal Tab.');
		panelWorkspaceStore.transition({
			direction: 'right',
			newTerminalTitle: 'Terminal',
			panelId: tab.focusedPanelId,
			presentation: panelWorkspaceStore.createTerminalPresentation(),
			tabId,
			type: 'panel-split',
		});
	}
	registerWorkbenchActiveTerminalActions({
		projectWorkspaceRuntime,
		runtime: actionRuntime,
		scope: parentActionScope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
	});
	const rendered = render(
		<OrchbeeI18nProvider locale='en-US'>
			<TooltipProvider>
				<WorkbenchPanelWorkspace
					actionRuntime={actionRuntime}
					panelWorkspaceStore={panelWorkspaceStore}
					parentActionScope={parentActionScope}
					presentationScopeId='test-context'
					shortcutLabelForAction={() => undefined}
					terminalAutoFocusAllowed
					terminalFocusRequest={null}
					terminalSurface={TestTerminalSurface}
				/>
			</TooltipProvider>
		</OrchbeeI18nProvider>,
	);
	return { actionRuntime, panelWorkspaceStore, parentActionScope, rendered };
}

function renderRunningThreePanelWorkspace() {
	const actionRuntime = new ActionRuntime();
	const parentActionScope = actionRuntime.createScope('test.broadcast-workbench');
	const tabModel = new TabModel({
		activeTabId: 'tab-broadcast',
		tabs: [{ group: null, id: 'tab-broadcast', pinned: false, title: 'Broadcast' }],
	});
	let sequence = 1;
	const composition = {
		createPresentationId: () => terminalPresentationId(`terminal-broadcast-${sequence++}`),
		initialPresentationId: terminalPresentationId('terminal-broadcast-initial'),
		kind: 'preview' as const,
	};
	const layoutBuilder = new PanelWorkspaceStore(tabModel, { composition });
	for (let index = 0; index < 2; index += 1) {
		const tab = layoutBuilder.getSnapshot().tabs[0];
		if (tab === undefined) throw new Error('Expected the broadcast terminal Tab.');
		layoutBuilder.transition({
			direction: 'right',
			newTerminalTitle: `Terminal ${index + 2}`,
			panelId: tab.focusedPanelId,
			presentation: layoutBuilder.createTerminalPresentation(),
			tabId: tab.tabId,
			type: 'panel-split',
		});
	}
	const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
		composition,
		initialState: runningWorkspaceState(layoutBuilder.getSnapshot()),
	});
	const rendered = render(
		<OrchbeeI18nProvider locale='en-US'>
			<TooltipProvider>
				<WorkbenchPanelWorkspace
					actionRuntime={actionRuntime}
					panelWorkspaceStore={panelWorkspaceStore}
					parentActionScope={parentActionScope}
					presentationScopeId='test-context'
					shortcutLabelForAction={() => undefined}
					terminalAutoFocusAllowed
					terminalFocusRequest={null}
					terminalSurface={TestTerminalSurface}
				/>
			</TooltipProvider>
		</OrchbeeI18nProvider>,
	);
	return { actionRuntime, panelWorkspaceStore, rendered };
}

beforeEach(() => {
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
	vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(
		function (this: HTMLElement) {
			switch (this.dataset['slot']) {
				case 'resizable-handle':
					return 10;
				case 'resizable-panel':
					return 500;
				case 'resizable-panel-group':
					return 1_000;
				default:
					return 0;
			}
		},
	);
	vi.spyOn(HTMLElement.prototype, 'offsetLeft', 'get').mockImplementation(
		function (this: HTMLElement) {
			return this.dataset['slot'] === 'resizable-panel' && this.id.endsWith('panel-2') ? 500 : 0;
		},
	);
	vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
		function (this: HTMLElement) {
			switch (this.dataset['slot']) {
				case 'resizable-handle':
					return createBounds(495, 10);
				case 'resizable-panel':
					return createBounds(this.id.endsWith('panel-2') ? 500 : 0, 500);
				case 'resizable-panel-group':
					return createBounds(0, 1_000);
				default:
					return createBounds(0, 0);
			}
		},
	);
});

afterEach(() => {
	cleanup();
	surfaces.clear();
	surfacesByMode.clear();
	broadcastInputs.clear();
	broadcastSendHandlers.clear();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});

describe('WorkbenchPanelWorkspace', () => {
	it('broadcasts committed text and key taps to exactly three displayed Panels in FIFO order', async () => {
		const { actionRuntime, panelWorkspaceStore, rendered } = renderRunningThreePanelWorkspace();
		const tab = panelWorkspaceStore.getSnapshot().tabs[0];
		if (tab === undefined) throw new Error('Expected the broadcast terminal Tab.');
		const targets = collectPanelLeaves(tab.root).map(
			(panel) => getDisplayedTerminal(panel).presentationId,
		);
		const firstDelivery = deferred<void>();
		for (const presentationId of targets) {
			broadcastSendHandlers.set(presentationId, async (input) => {
				if (input.kind === 'text' && input.text === 'first') await firstDelivery.promise;
				return { kind: 'sent' };
			});
		}
		const sourceViewport = rendered.container.querySelector<HTMLElement>(
			`[data-panel-id="${tab.focusedPanelId}"][data-slot="workbench-terminal-viewport"]`,
		);
		if (sourceViewport === null) throw new Error('Expected the focused terminal viewport.');

		sourceViewport.focus();
		fireEvent.contextMenu(sourceViewport, { clientX: 20, clientY: 20 });
		fireEvent.click(screen.getByRole('menuitem', { name: 'Broadcast Input' }));
		const input = await screen.findByRole('textbox', { name: 'Broadcast input' });
		expect(
			rendered.container.querySelectorAll('[data-panel-broadcast-target="true"]'),
		).toHaveLength(3);

		fireEvent.input(input, { target: { value: 'first' } });
		fireEvent.input(input, { target: { value: 'second' } });
		await vi.waitFor(() =>
			expect(targets.map((presentationId) => broadcastInputs.get(presentationId)?.[0])).toEqual(
				targets.map(() => ({ kind: 'text', text: 'first' })),
			),
		);
		expect(
			targets.flatMap((presentationId) => broadcastInputs.get(presentationId)?.slice(1) ?? []),
		).toEqual([]);

		firstDelivery.resolve();
		await vi.waitFor(() =>
			expect(targets.map((presentationId) => broadcastInputs.get(presentationId)?.[1])).toEqual(
				targets.map(() => ({ kind: 'text', text: 'second' })),
			),
		);
		fireEvent.keyDown(input, { code: 'Escape', key: 'Escape' });
		await vi.waitFor(() =>
			expect(targets.map((presentationId) => broadcastInputs.get(presentationId)?.[2])).toEqual(
				targets.map(() => expect.objectContaining({ code: 'Escape', key: 'Escape', kind: 'key' })),
			),
		);
		fireEvent.keyDown(input, { code: 'Escape', ctrlKey: true, key: 'Escape' });
		await vi.waitFor(() =>
			expect(screen.queryByRole('textbox', { name: 'Broadcast input' })).toBeNull(),
		);
		expect(rendered.container.querySelector('[data-panel-broadcast-target="true"]')).toBeNull();

		rendered.unmount();
		actionRuntime.dispose();
	});

	it('ends visibly after a non-atomic partial broadcast delivery and never replays it', async () => {
		const { actionRuntime, panelWorkspaceStore, rendered } = renderRunningThreePanelWorkspace();
		const tab = panelWorkspaceStore.getSnapshot().tabs[0];
		if (tab === undefined) throw new Error('Expected the broadcast terminal Tab.');
		const targets = collectPanelLeaves(tab.root).map(
			(panel) => getDisplayedTerminal(panel).presentationId,
		);
		broadcastSendHandlers.set(targets[1] ?? targets[0] ?? '', async () => ({ kind: 'failed' }));
		const sourceViewport = rendered.container.querySelector<HTMLElement>(
			`[data-panel-id="${tab.focusedPanelId}"][data-slot="workbench-terminal-viewport"]`,
		);
		if (sourceViewport === null) throw new Error('Expected the focused terminal viewport.');

		sourceViewport.focus();
		fireEvent.contextMenu(sourceViewport, { clientX: 20, clientY: 20 });
		fireEvent.click(screen.getByRole('menuitem', { name: 'Broadcast Input' }));
		const input = await screen.findByRole('textbox', { name: 'Broadcast input' });
		fireEvent.input(input, { target: { value: 'unsafe-to-retry' } });

		await screen.findByRole('alert');
		expect(screen.getByRole('alert').textContent).toContain('confirmed by 2 of 3 terminals');
		expect(screen.queryByRole('textbox', { name: 'Broadcast input' })).toBeNull();
		expect(targets.map((presentationId) => broadcastInputs.get(presentationId))).toEqual(
			targets.map(() => [{ kind: 'text', text: 'unsafe-to-retry' }]),
		);

		rendered.unmount();
		actionRuntime.dispose();
	});
	it('owns keyboard selection exclusively and focuses the physically labeled target', async () => {
		const { actionRuntime, panelWorkspaceStore, parentActionScope, rendered } =
			renderThreePanelWorkspace();
		await act(async () => {
			await actionRuntime.execute(WORKBENCH_ACTION_IDS.focusActivePanelVisible, undefined, {
				scope: parentActionScope,
				source: 'keyboard',
			}).completion;
		});
		const workspace = rendered.container.querySelector<HTMLElement>(
			'[data-slot="workbench-panel-workspace"]',
		);
		if (workspace === null) throw new Error('Expected the Panel workspace.');
		expect(workspace.getAttribute('data-workbench-keyboard-capture')).toBe('true');
		expect(workspace.getAttribute('aria-label')).toContain('Choose a panel');
		expect(
			[...rendered.container.querySelectorAll('[data-panel-visual-selection-target="true"]')].map(
				(target) => target.getAttribute('data-label'),
			),
		).toEqual(['1', '2']);

		const invalid = new KeyboardEvent('keydown', {
			bubbles: true,
			cancelable: true,
			code: 'Slash',
		});
		workspace.dispatchEvent(invalid);
		expect(invalid.defaultPrevented).toBe(true);
		expect(workspace.getAttribute('data-workbench-keyboard-capture')).toBe('true');

		fireEvent.keyDown(workspace, { code: 'Digit1', ctrlKey: true, shiftKey: true });
		expect(panelWorkspaceStore.getSnapshot().tabs[0]?.focusedPanelId).toBe('panel-1');
		expect(workspace.hasAttribute('data-workbench-keyboard-capture')).toBe(false);
		expect(
			rendered.container.querySelector('[data-panel-visual-selection-target="true"]'),
		).toBeNull();
		expect((document.activeElement as HTMLElement | null)?.dataset['panelId']).toBe('panel-1');

		actionRuntime.dispose();
	});

	it('swaps with a primary-click target through the exact lower Panel Action', async () => {
		const { actionRuntime, panelWorkspaceStore, parentActionScope, rendered } =
			renderThreePanelWorkspace();
		await act(async () => {
			await actionRuntime.execute(WORKBENCH_ACTION_IDS.swapActivePanelVisible, undefined, {
				scope: parentActionScope,
				source: 'keyboard',
			}).completion;
		});
		const target = rendered.container.querySelector<HTMLElement>(
			'[data-panel-visual-selection-target="true"][data-panel-id="panel-2"]',
		);
		if (target === null) throw new Error('Expected the Panel swap target.');

		fireEvent.pointerDown(target, { button: 0 });

		const tab = panelWorkspaceStore.getSnapshot().tabs[0];
		expect(tab === undefined ? [] : collectPanelLeaves(tab.root).map((panel) => panel.id)).toEqual([
			'panel-1',
			'panel-3',
			'panel-2',
		]);
		expect(tab?.focusedPanelId).toBe('panel-3');
		expect(
			rendered.container.querySelector('[data-panel-visual-selection-target="true"]'),
		).toBeNull();

		actionRuntime.dispose();
	});

	it('closes without restoring focus when the captured Panel workspace becomes stale', async () => {
		const { actionRuntime, panelWorkspaceStore, parentActionScope, rendered } =
			renderThreePanelWorkspace();
		await act(async () => {
			await actionRuntime.execute(WORKBENCH_ACTION_IDS.focusActivePanelVisible, undefined, {
				scope: parentActionScope,
				source: 'keyboard',
			}).completion;
		});
		expect(
			rendered.container.querySelector('[data-panel-visual-selection-target="true"]'),
		).not.toBeNull();
		const tabId = panelWorkspaceStore.getActiveTabId();
		if (tabId === null) throw new Error('Expected an active terminal Tab.');

		act(() => {
			panelWorkspaceStore.transition({
				panelId: 'panel-1',
				tabId,
				type: 'panel-focused',
			});
		});

		expect(
			rendered.container.querySelector('[data-panel-visual-selection-target="true"]'),
		).toBeNull();
		expect(panelWorkspaceStore.getSnapshot().tabs[0]?.focusedPanelId).toBe('panel-1');

		actionRuntime.dispose();
	});

	it('keeps the outgoing frame until display readiness and delays focus until activation', () => {
		const actionRuntime = new ActionRuntime();
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [
				{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' },
				{ group: null, id: 'tab-b', pinned: false, title: 'Terminal B' },
			],
		});
		let sequence = 1;
		const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId(`terminal-${sequence++}`),
				initialPresentationId: terminalPresentationId('terminal-a'),
				kind: 'preview',
			},
		});
		const targetTab = panelWorkspaceStore.getSnapshot().tabs[1];
		const targetPanel = targetTab === undefined ? undefined : collectPanelLeaves(targetTab.root)[0];
		if (targetPanel === undefined) throw new Error('Expected the target Tab workspace.');
		const targetPresentationId = getDisplayedTerminal(targetPanel).presentationId;
		const rendered = render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<WorkbenchPanelWorkspace
						actionRuntime={actionRuntime}
						panelWorkspaceStore={panelWorkspaceStore}
						parentActionScope={actionRuntime.createScope('test.workbench')}
						presentationScopeId='test-context'
						shortcutLabelForAction={() => undefined}
						terminalAutoFocusAllowed
						terminalFocusRequest={null}
						terminalSurface={TestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		act(() => tabModel.selectTab('tab-b'));
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-a"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('retained');
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-b"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('incoming');
		expect(surfaces.get(targetPresentationId)?.autoFocus).toBe(false);

		act(() => surfaces.get(targetPresentationId)?.onPresentationReady('display-ready'));
		expect(rendered.container.querySelector('[data-terminal-tab-id="tab-a"]')).toBeNull();
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-b"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('incoming');
		expect(surfaces.get(targetPresentationId)?.autoFocus).toBe(false);

		act(() => surfaces.get(targetPresentationId)?.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-b"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('active');
		expect(surfaces.get(targetPresentationId)?.autoFocus).toBe(true);

		actionRuntime.dispose();
	});

	it('keeps the pre-maximize same-Tab layer visible until the maximized geometry is ready', () => {
		const actionRuntime = new ActionRuntime();
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		let sequence = 1;
		const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId(`terminal-max-${sequence++}`),
				initialPresentationId: terminalPresentationId('terminal-max-initial'),
				kind: 'preview',
			},
		});
		const initialTab = panelWorkspaceStore.getSnapshot().tabs[0];
		const initialPanel =
			initialTab === undefined ? undefined : collectPanelLeaves(initialTab.root)[0];
		if (initialPanel === undefined) throw new Error('Expected the initial Panel.');
		panelWorkspaceStore.transition({
			direction: 'right',
			newTerminalTitle: 'Terminal',
			panelId: initialPanel.id,
			presentation: panelWorkspaceStore.createTerminalPresentation(),
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const splitTab = panelWorkspaceStore.getSnapshot().tabs[0];
		if (splitTab === undefined) throw new Error('Expected the split Tab.');
		const maximizedPanel = collectPanelLeaves(splitTab.root).find(
			(panel) => panel.id === splitTab.focusedPanelId,
		);
		if (maximizedPanel === undefined) throw new Error('Expected the focused Panel.');
		const maximizedPresentationId = getDisplayedTerminal(maximizedPanel).presentationId;
		const rendered = render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<WorkbenchPanelWorkspace
						actionRuntime={actionRuntime}
						panelWorkspaceStore={panelWorkspaceStore}
						parentActionScope={actionRuntime.createScope('test.workbench')}
						presentationScopeId='test-context'
						shortcutLabelForAction={() => undefined}
						terminalAutoFocusAllowed
						terminalFocusRequest={null}
						terminalSurface={TestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		act(() => {
			panelWorkspaceStore.transition({
				panelId: maximizedPanel.id,
				tabId: 'tab-a',
				type: 'panel-maximization-toggled',
			});
		});
		const pendingLayers = [
			...rendered.container.querySelectorAll('[data-terminal-tab-id="tab-a"]'),
		];
		expect(pendingLayers.map((element) => element.getAttribute('data-presentation-mode'))).toEqual([
			'incoming',
			'retained',
		]);
		const incomingSurface = surfacesByMode.get(`${maximizedPresentationId}:incoming`);
		if (incomingSurface === undefined) throw new Error('Expected the incoming maximized surface.');
		expect(incomingSurface.autoFocus).toBe(false);

		act(() => incomingSurface.onPresentationReady('display-ready'));
		expect(
			[...rendered.container.querySelectorAll('[data-terminal-tab-id="tab-a"]')].map((element) =>
				element.getAttribute('data-presentation-mode'),
			),
		).toEqual(['incoming']);
		act(() => incomingSurface.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-a"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('active');
		expect(surfaces.get(maximizedPresentationId)?.autoFocus).toBe(true);

		actionRuntime.dispose();
	});

	it('keeps a closed active Tab visible until its surviving replacement is display ready', () => {
		const actionRuntime = new ActionRuntime();
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [
				{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' },
				{ group: null, id: 'tab-b', pinned: false, title: 'Terminal B' },
			],
		});
		let sequence = 1;
		const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId(`terminal-close-${sequence++}`),
				initialPresentationId: terminalPresentationId('terminal-a'),
				kind: 'preview',
			},
		});
		const tabA = panelWorkspaceStore.getSnapshot().tabs[0];
		const tabB = panelWorkspaceStore.getSnapshot().tabs[1];
		const tabAPanel = tabA === undefined ? undefined : collectPanelLeaves(tabA.root)[0];
		const tabBPanel = tabB === undefined ? undefined : collectPanelLeaves(tabB.root)[0];
		if (tabAPanel === undefined || tabBPanel === undefined) {
			throw new Error('Expected both Tab workspaces.');
		}
		const tabAPresentationId = getDisplayedTerminal(tabAPanel).presentationId;
		const tabBPresentationId = getDisplayedTerminal(tabBPanel).presentationId;
		const rendered = render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<WorkbenchPanelWorkspace
						actionRuntime={actionRuntime}
						panelWorkspaceStore={panelWorkspaceStore}
						parentActionScope={actionRuntime.createScope('test.workbench')}
						presentationScopeId='test-context'
						shortcutLabelForAction={() => undefined}
						terminalAutoFocusAllowed
						terminalFocusRequest={null}
						terminalSurface={TestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		act(() => tabModel.selectTab('tab-b'));
		act(() => surfaces.get(tabBPresentationId)?.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-b"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('active');

		act(() => panelWorkspaceStore.requestTabClose(['tab-b']));
		expect(tabModel.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['tab-a']);
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-b"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('retained');
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-a"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('incoming');
		expect(surfaces.get(tabAPresentationId)?.autoFocus).toBe(false);

		act(() => surfaces.get(tabAPresentationId)?.onPresentationReady('display-ready'));
		expect(rendered.container.querySelector('[data-terminal-tab-id="tab-b"]')).toBeNull();
		act(() => surfaces.get(tabAPresentationId)?.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-a"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('active');

		actionRuntime.dispose();
	});

	it('commits only the maximized Panel when returning to a split Tab', () => {
		const actionRuntime = new ActionRuntime();
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [
				{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' },
				{ group: null, id: 'tab-b', pinned: false, title: 'Terminal B' },
			],
		});
		let sequence = 1;
		const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId(`terminal-${sequence++}`),
				initialPresentationId: terminalPresentationId('terminal-a'),
				kind: 'preview',
			},
		});
		const initialTabA = panelWorkspaceStore.getSnapshot().tabs[0];
		const sourcePanel =
			initialTabA === undefined ? undefined : collectPanelLeaves(initialTabA.root)[0];
		if (sourcePanel === undefined) throw new Error('Expected the source Tab workspace.');
		panelWorkspaceStore.transition({
			direction: 'right',
			newTerminalTitle: 'Terminal',
			panelId: sourcePanel.id,
			presentation: panelWorkspaceStore.createTerminalPresentation(),
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const splitTabA = panelWorkspaceStore.getSnapshot().tabs[0];
		if (splitTabA === undefined) throw new Error('Expected the split source Tab workspace.');
		const maximizedPanelId = splitTabA.focusedPanelId;
		panelWorkspaceStore.transition({
			panelId: maximizedPanelId,
			tabId: 'tab-a',
			type: 'panel-maximization-toggled',
		});
		const maximizedTabA = panelWorkspaceStore.getSnapshot().tabs[0];
		const tabB = panelWorkspaceStore.getSnapshot().tabs[1];
		const maximizedPanel =
			maximizedTabA === undefined
				? undefined
				: collectPanelLeaves(maximizedTabA.root).find((panel) => panel.id === maximizedPanelId);
		const tabBPanel = tabB === undefined ? undefined : collectPanelLeaves(tabB.root)[0];
		if (maximizedPanel === undefined || tabBPanel === undefined) {
			throw new Error('Expected both Tab workspaces.');
		}
		const maximizedPresentationId = getDisplayedTerminal(maximizedPanel).presentationId;
		const tabBPresentationId = getDisplayedTerminal(tabBPanel).presentationId;
		const rendered = render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<WorkbenchPanelWorkspace
						actionRuntime={actionRuntime}
						panelWorkspaceStore={panelWorkspaceStore}
						parentActionScope={actionRuntime.createScope('test.workbench')}
						presentationScopeId='test-context'
						shortcutLabelForAction={() => undefined}
						terminalAutoFocusAllowed
						terminalFocusRequest={null}
						terminalSurface={TestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		act(() => tabModel.selectTab('tab-b'));
		act(() => surfaces.get(tabBPresentationId)?.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-b"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('active');

		act(() => tabModel.selectTab('tab-a'));
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-b"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('retained');
		const incomingMaximizedSurface = surfacesByMode.get(`${maximizedPresentationId}:incoming`);
		if (incomingMaximizedSurface === undefined) {
			throw new Error('Expected the incoming maximized surface.');
		}
		act(() => incomingMaximizedSurface.onPresentationReady('activation-ready'));

		expect(rendered.container.querySelector('[data-terminal-tab-id="tab-b"]')).toBeNull();
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-a"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('active');

		actionRuntime.dispose();
	});
});
