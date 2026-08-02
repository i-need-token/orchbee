// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { act, createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../../../terminal-presentation';
import { findPanel } from '../../model/panel-workspace-state';
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
	});
	capture(view);
	return null;
}

afterEach(cleanup);

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
