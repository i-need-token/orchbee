// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { act, createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { findPanel } from '../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import { useWorkbenchPanelWorkspace } from '../useWorkbenchPanelWorkspace';

const serverTabModel = new TabModel({
	activeTabId: 'tab-a',
	tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
});
const serverActionRuntime = new ActionRuntime();
const serverParentActionScope = serverActionRuntime.createScope('workbench');

function HookProjection() {
	const view = useWorkbenchPanelWorkspace({
		actionRuntime: serverActionRuntime,
		parentActionScope: serverParentActionScope,
		tabModel: serverTabModel,
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
	parentActionScope,
	tabModel,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly capture: (view: WorkbenchPanelWorkspaceView) => void;
	readonly parentActionScope: ReturnType<ActionRuntime['createScope']>;
	readonly tabModel: TabModel;
}) {
	const view = useWorkbenchPanelWorkspace({
		actionRuntime,
		parentActionScope,
		tabModel,
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
					tabModel: model,
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
					tabModel: model,
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
		const firstChildId = panelWithChildren?.childTerminals[0]?.id;
		const secondChildId = panelWithChildren?.childTerminals[1]?.id;
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
		expect(panelAfterConfirmation?.activeChildTerminalId).toBe(secondChildId);
		expect(panelAfterConfirmation?.childTerminals.map((terminal) => terminal.id)).toEqual([
			firstChildId,
			secondChildId,
		]);
	});
});
