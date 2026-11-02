// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { act, createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { collectPanelLeaves, findPanel } from '../../model/panel-workspace-state';
import { PanelWorkspaceStore } from '../../model/PanelWorkspaceStore';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import { useWorkbenchPanelWorkspace } from '../useWorkbenchPanelWorkspace';

const serverTabModel = new TabModel({
	activeTabId: 'tab-a',
	tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
});
const serverActionRuntime = new ActionRuntime();
const serverParentActionScope = serverActionRuntime.createScope('workbench');
const serverPanelWorkspaceStore = new PanelWorkspaceStore(serverTabModel, {
	composition: createPreviewComposition('server'),
});

function createPreviewComposition(scope: string) {
	let sequence = 1;
	return {
		createPresentationId: () => terminalPresentationId(`${scope}:dynamic:${sequence++}`),
		initialPresentationId: terminalPresentationId(`${scope}:initial`),
		kind: 'preview' as const,
	};
}

function HookProjection() {
	const view = useWorkbenchPanelWorkspace({
		actionRuntime: serverActionRuntime,
		panelWorkspaceStore: serverPanelWorkspaceStore,
		parentActionScope: serverParentActionScope,
		shortcutLabelForAction: () => undefined,
		terminalContextId: 'terminal-context-1',
	});
	return createElement(
		'div',
		null,
		`${view.activeTab?.tabId}:${view.messages.newChildTerminal}:${view.state.tabs.length}:${
			'layoutKey' in view.panelDrag
		}`,
	);
}

function InteractiveHarness({
	actionRuntime,
	capture,
	panelWorkspaceStore,
	parentActionScope,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly capture: (view: WorkbenchPanelWorkspaceView) => void;
	readonly parentActionScope: ReturnType<ActionRuntime['createScope']>;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
}) {
	const view = useWorkbenchPanelWorkspace({
		actionRuntime,
		panelWorkspaceStore,
		parentActionScope,
		shortcutLabelForAction: () => undefined,
		terminalContextId: 'terminal-context-1',
	});
	capture(view);
	return null;
}

afterEach(() => {
	cleanup();
	document.body.replaceChildren();
});

describe('useWorkbenchPanelWorkspace', () => {
	it('projects the active tab and localized panel actions on the first render', () => {
		const markup = renderToStaticMarkup(
			createElement(OrchbeeI18nProvider, { locale: 'zh-CN' }, createElement(HookProjection)),
		);

		expect(markup).toContain('tab-a:新建子终端:1:false');
	});

	it('keeps the panel drag entry point stable across workspace updates', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		let view: WorkbenchPanelWorkspaceView | undefined;
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(InteractiveHarness, {
					actionRuntime: runtime,
					capture: (candidate) => {
						view = candidate;
					},
					parentActionScope: parentScope,
					panelWorkspaceStore: new PanelWorkspaceStore(model, {
						composition: createPreviewComposition('stable-drag'),
					}),
				}),
			),
		);
		if (view === undefined) {
			throw new Error('Expected the panel workspace view to be captured');
		}
		const startPanelDrag = view.startPanelDrag;

		await act(async () => {
			view?.createChildTerminal('panel-1');
		});

		expect(view.startPanelDrag).toBe(startPanelDrag);
	});

	it('keeps the complete panel tree displayed while one panel is maximized', () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		let view: WorkbenchPanelWorkspaceView | undefined;
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(InteractiveHarness, {
					actionRuntime: runtime,
					capture: (candidate) => {
						view = candidate;
					},
					parentActionScope: parentScope,
					panelWorkspaceStore: new PanelWorkspaceStore(model, {
						composition: createPreviewComposition('maximize-tree'),
					}),
				}),
			),
		);
		if (view === undefined) {
			throw new Error('Expected the panel workspace view to be captured');
		}

		let newPanelId: string | null = null;
		act(() => {
			newPanelId = view?.splitPanel('panel-1', 'right') ?? null;
		});
		if (newPanelId === null) {
			throw new Error('Expected a split panel');
		}
		const maximizedPanelId = newPanelId;
		act(() => {
			view?.togglePanelMaximization(maximizedPanelId);
		});

		expect(view.maximizedPanelId).toBe(maximizedPanelId);
		expect(view.displayedRoot).toBe(view.activeTab?.root);
		expect(view.displayedRoot?.kind).toBe('split');
	});

	it('cancels an owned panel drag when the active Tab changes', () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-a',
			tabs: [
				{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' },
				{ group: null, id: 'tab-b', pinned: false, title: 'Terminal B' },
			],
		});
		let view: WorkbenchPanelWorkspaceView | undefined;
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(InteractiveHarness, {
					actionRuntime: runtime,
					capture: (candidate) => {
						view = candidate;
					},
					parentActionScope: parentScope,
					panelWorkspaceStore: new PanelWorkspaceStore(model, {
						composition: createPreviewComposition('drag-tab-change'),
					}),
				}),
			),
		);
		if (view === undefined || view.activeTab === null) {
			throw new Error('Expected the panel workspace view to be captured');
		}
		const sourcePanelId = collectPanelLeaves(view.activeTab.root)[0]?.id;
		if (sourcePanelId === undefined) throw new Error('Expected a source Panel');
		act(() => {
			view?.splitPanel(sourcePanelId, 'right');
		});

		const workspaceElement = document.createElement('div');
		workspaceElement.dataset['slot'] = 'workbench-panel-workspace';
		const sourceElement = document.createElement('section');
		sourceElement.dataset['slot'] = 'workbench-panel';
		sourceElement.dataset['panelId'] = sourcePanelId;
		const headerElement = document.createElement('button');
		sourceElement.append(headerElement);
		workspaceElement.append(sourceElement);
		document.body.append(workspaceElement);
		vi.spyOn(sourceElement, 'getBoundingClientRect').mockReturnValue({
			bottom: 100,
			height: 100,
			left: 0,
			right: 200,
			top: 0,
			width: 200,
			x: 0,
			y: 0,
			toJSON: () => ({}),
		});
		const setPointerCapture = vi.fn<(pointerId: number) => void>();
		const releasePointerCapture = vi.fn<(pointerId: number) => void>();
		Object.defineProperties(workspaceElement, {
			hasPointerCapture: { value: () => true },
			releasePointerCapture: { value: releasePointerCapture },
			setPointerCapture: { value: setPointerCapture },
		});

		act(() => {
			view?.startPanelDrag(sourcePanelId, {
				button: 0,
				clientX: 50,
				clientY: 25,
				currentTarget: headerElement,
				pointerId: 7,
				preventDefault: vi.fn<() => void>(),
			} as unknown as Parameters<WorkbenchPanelWorkspaceView['startPanelDrag']>[1]);
		});
		expect(setPointerCapture).toHaveBeenCalledWith(7);

		act(() => model.selectTab('tab-b'));

		expect(releasePointerCapture).toHaveBeenCalledWith(7);
	});

	it('captures one resize target, commits through the Action, and closes on a Tab change', () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-a',
			tabs: [
				{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' },
				{ group: null, id: 'tab-b', pinned: false, title: 'Terminal B' },
			],
		});
		let view: WorkbenchPanelWorkspaceView | undefined;
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(InteractiveHarness, {
					actionRuntime: runtime,
					capture: (candidate) => {
						view = candidate;
					},
					parentActionScope: parentScope,
					panelWorkspaceStore: new PanelWorkspaceStore(model, {
						composition: createPreviewComposition('resize-target'),
					}),
				}),
			),
		);
		if (view === undefined) throw new Error('Expected the panel workspace view to be captured');

		let panelId: string | null = null;
		act(() => {
			panelId = view?.splitPanel('panel-1', 'right') ?? null;
		});
		if (panelId === null) throw new Error('Expected a split panel');
		const resizePanelId = panelId;
		act(() => view?.startPanelResize(resizePanelId));
		expect(view.pendingResize).toEqual({ panelId: resizePanelId, tabId: 'tab-a' });
		expect(view.canResizePanel(resizePanelId, 'wider')).toBe(true);

		let resized = false;
		act(() => {
			resized = view?.resizePanel(resizePanelId, 'wider', 1) ?? false;
		});
		expect(resized).toBe(true);
		expect(view.activeTab?.root).toMatchObject({
			children: [{ weight: 45 }, { weight: 55 }],
			kind: 'split',
		});

		act(() => model.selectTab('tab-b'));
		expect(view.pendingResize).toBeNull();
		expect(view.resizePanel(resizePanelId, 'wider', 1)).toBe(false);
	});

	it('shares one terminal bell admission ledger across the panel workspace lifetime', () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		let view: WorkbenchPanelWorkspaceView | undefined;
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(InteractiveHarness, {
					actionRuntime: runtime,
					capture: (candidate) => {
						view = candidate;
					},
					parentActionScope: parentScope,
					panelWorkspaceStore: new PanelWorkspaceStore(model, {
						composition: createPreviewComposition('bell-effects'),
					}),
				}),
			),
		);
		if (view === undefined) throw new Error('Expected the panel workspace view to be captured');

		expect(view.admitTerminalBellEffect({ sequence: '5', terminalId: 'terminal-a' })).toBe(true);
		expect(view.admitTerminalBellEffect({ sequence: '5', terminalId: 'terminal-a' })).toBe(false);
		expect(view.admitTerminalBellEffect({ sequence: '7', terminalId: 'terminal-a' })).toBe(true);
		expect(view.admitTerminalBellEffect({ sequence: '6', terminalId: 'terminal-a' })).toBe(true);
		expect(view.admitTerminalBellEffect({ sequence: '5', terminalId: 'terminal-b' })).toBe(true);
	});

	it('does not close a different child terminal after confirmation changes the active target', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		let view: WorkbenchPanelWorkspaceView | undefined;
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(InteractiveHarness, {
					actionRuntime: runtime,
					capture: (candidate) => {
						view = candidate;
					},
					parentActionScope: parentScope,
					panelWorkspaceStore: new PanelWorkspaceStore(model, {
						composition: createPreviewComposition('close-target'),
					}),
				}),
			),
		);
		if (view === undefined) {
			throw new Error('Expected the panel workspace view to be captured');
		}

		await act(async () => {
			view?.createChildTerminal('panel-1');
		});
		await act(async () => {
			view?.createChildTerminal('panel-1');
		});
		const panelWithChildren =
			view.activeTab === null ? null : findPanel(view.activeTab.root, 'panel-1');
		const firstChildId = panelWithChildren?.childPresentations[0]?.presentationId;
		const secondChildId = panelWithChildren?.childPresentations[1]?.presentationId;
		if (firstChildId === undefined || secondChildId === undefined) {
			throw new Error('Expected two child terminals');
		}

		await act(async () => {
			view?.activateChildTerminal('panel-1', firstChildId);
		});
		await act(async () => {
			view?.requestClose('panel-1');
		});
		expect(view.pendingClose).toEqual({ panelId: 'panel-1', runningCount: 1 });

		await act(async () => {
			view?.activateChildTerminal('panel-1', secondChildId);
		});
		await act(async () => {
			view?.confirmClose();
		});

		const panelAfterConfirmation =
			view.activeTab === null ? null : findPanel(view.activeTab.root, 'panel-1');
		expect(panelAfterConfirmation?.activeChildPresentationId).toBe(secondChildId);
		expect(
			panelAfterConfirmation?.childPresentations.map((terminal) => terminal.presentationId),
		).toEqual([firstChildId, secondChildId]);
	});
});
