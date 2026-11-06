// @vitest-environment jsdom

import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePanelId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
	type FixtureRecord,
} from '../../../../../__tests__/server-workbench-snapshot';
import type { WorkbenchSharedPanelTerminalCreate } from '../../../../actions/createSharedPanelTerminal';
import type { WorkbenchTerminalContext } from '../../../../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../../../../types/workbench-authority';
import { PANEL_ACTION_IDS } from '../../actions/panel-action-contracts';
import type { PanelWorkspaceStore } from '../../model/PanelWorkspaceStore';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import { useWorkbenchPanelWorkspace } from '../useWorkbenchPanelWorkspace';
import { inertWorkbenchAuthority } from '../../../../../__tests__/inertWorkbenchAuthority';

const TWO_PANEL_ROOT = snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2));

function unexpected(): Promise<never> {
	return Promise.reject(new Error('The test did not expect this authority call.'));
}

const createPanelTerminal: WorkbenchSharedPanelTerminalCreate = unexpected;

/** The terminal context a window holds when the Server publishes Workspace 1 with `tabs`. */
function publishedContext(tabs: readonly FixtureRecord[]): WorkbenchTerminalContext {
	return activeTerminalContext(createSnapshotRuntime(oneWorkspaceSnapshot(1, tabs)));
}

function HookProjection({
	panelWorkspaceStore,
}: {
	readonly panelWorkspaceStore: PanelWorkspaceStore;
}) {
	const actionRuntime = new ActionRuntime();
	const view = useWorkbenchPanelWorkspace({
		actionRuntime,
		authority: inertWorkbenchAuthority(),
		createPanelTerminal,
		panelWorkspaceStore,
		parentActionScope: actionRuntime.createScope('workbench'),
		presentationAgentStatuses: new Map(),
		shortcutLabelForAction: () => undefined,
		terminalContextId: 'terminal-context-1',
	});
	return createElement(
		'div',
		null,
		`${view.activeTab?.tabId}:${view.messages.returnToPrimary}:${view.state.tabs.length}:${
			'layoutKey' in view.panelDrag
		}`,
	);
}

function InteractiveHarness({
	actionRuntime,
	authority,
	capture,
	panelWorkspaceStore,
	parentActionScope,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly authority: WorkbenchAuthority;
	readonly capture: (view: WorkbenchPanelWorkspaceView) => void;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly parentActionScope: ActionScope;
}) {
	const view = useWorkbenchPanelWorkspace({
		actionRuntime,
		authority,
		createPanelTerminal,
		panelWorkspaceStore,
		parentActionScope,
		presentationAgentStatuses: new Map(),
		shortcutLabelForAction: () => undefined,
		terminalContextId: 'terminal-context-1',
	});
	capture(view);
	return null;
}

interface RenderedWorkspace {
	readonly actionRuntime: ActionRuntime;
	readonly context: WorkbenchTerminalContext;
	readonly view: () => WorkbenchPanelWorkspaceView;
}

function renderWorkspace(
	tabs: readonly FixtureRecord[],
	authority: WorkbenchAuthority = inertWorkbenchAuthority(),
): RenderedWorkspace {
	const actionRuntime = new ActionRuntime();
	const context = publishedContext(tabs);
	let view: WorkbenchPanelWorkspaceView | undefined;
	render(
		createElement(
			OrchbeeI18nProvider,
			{ locale: 'en-US' },
			createElement(InteractiveHarness, {
				actionRuntime,
				authority,
				capture: (candidate) => {
					view = candidate;
				},
				panelWorkspaceStore: context.panelWorkspaceStore,
				parentActionScope: actionRuntime.createScope('workbench'),
			}),
		),
	);
	return {
		actionRuntime,
		context,
		view: () => {
			if (view === undefined) throw new Error('Expected the panel workspace view to be captured');
			return view;
		},
	};
}

afterEach(() => {
	cleanup();
	document.body.replaceChildren();
});

describe('useWorkbenchPanelWorkspace', () => {
	it('projects the active tab and localized panel actions on the first render', () => {
		const context = publishedContext([snapshotTab(1)]);
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'zh-CN' },
				createElement(HookProjection, { panelWorkspaceStore: context.panelWorkspaceStore }),
			),
		);

		expect(markup).toContain(`${fixtureTabId(1)}:返回主终端:1:false`);
	});

	it('keeps the panel drag entry point stable across workspace updates', () => {
		const { context, view } = renderWorkspace([snapshotTab(1, TWO_PANEL_ROOT)]);
		const startPanelDrag = view().startPanelDrag;

		act(() => {
			context.panelWorkspaceStore.transition({
				panelId: fixturePanelId(2),
				tabId: fixtureTabId(1),
				type: 'panel-focused',
			});
		});

		expect(view().activeTab?.focusedPanelId).toBe(fixturePanelId(2));
		expect(view().startPanelDrag).toBe(startPanelDrag);
	});

	it('keeps the complete panel tree displayed while one panel is maximized', () => {
		const { view } = renderWorkspace([snapshotTab(1, TWO_PANEL_ROOT)]);
		const maximizedPanelId = fixturePanelId(2);

		act(() => {
			view().togglePanelMaximization(maximizedPanelId);
		});

		expect(view().maximizedPanelId).toBe(maximizedPanelId);
		expect(view().displayedRoot).toBe(view().activeTab?.root);
		expect(view().displayedRoot?.kind).toBe('split');
	});

	it('cancels an owned panel drag when the active Tab changes', () => {
		const { context, view } = renderWorkspace([snapshotTab(1, TWO_PANEL_ROOT), snapshotTab(3)]);
		const sourcePanelId = fixturePanelId(1);

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
			view().startPanelDrag(sourcePanelId, {
				button: 0,
				clientX: 50,
				clientY: 25,
				currentTarget: headerElement,
				pointerId: 7,
				preventDefault: vi.fn<() => void>(),
			} as unknown as Parameters<WorkbenchPanelWorkspaceView['startPanelDrag']>[1]);
		});
		expect(setPointerCapture).toHaveBeenCalledWith(7);

		act(() => context.tabModel.selectTab(fixtureTabId(3)));

		expect(releasePointerCapture).toHaveBeenCalledWith(7);
	});

	it('captures one resize target, commits through the authority, and closes on a Tab change', async () => {
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const { actionRuntime, context, view } = renderWorkspace(
			[snapshotTab(1, TWO_PANEL_ROOT), snapshotTab(3)],
			inertWorkbenchAuthority({ change }),
		);
		const resizePanelId = fixturePanelId(2);
		const panelScope = view().actionScope;
		act(() => {
			actionRuntime.execute(
				PANEL_ACTION_IDS.startResize,
				{ panelId: resizePanelId },
				{ scope: panelScope, source: 'programmatic', target: { id: resizePanelId, type: 'panel' } },
			);
		});
		expect(view().pendingResize).toEqual({ panelId: resizePanelId, tabId: fixtureTabId(1) });
		expect(view().canResizePanel(resizePanelId, 'wider')).toBe(true);

		let resized: Promise<boolean> | undefined;
		act(() => {
			resized = view().resizePanel(resizePanelId, 'wider', 1);
		});
		await expect(resized).resolves.toBe(true);
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			adjustment: 'wider',
			multiplier: 1,
			panelId: resizePanelId,
			tabId: fixtureTabId(1),
			terminalContextId: 'terminal-context-1',
			type: 'panel-size-adjusted',
		});

		act(() => context.tabModel.selectTab(fixtureTabId(3)));
		expect(view().pendingResize).toBeNull();
		await expect(view().resizePanel(resizePanelId, 'wider', 1)).resolves.toBe(false);
	});

	it('shares one terminal bell admission ledger across the panel workspace lifetime', () => {
		const { view } = renderWorkspace([snapshotTab(1)]);

		expect(view().admitTerminalBellEffect({ sequence: '5', terminalId: 'terminal-a' })).toBe(true);
		expect(view().admitTerminalBellEffect({ sequence: '5', terminalId: 'terminal-a' })).toBe(false);
		expect(view().admitTerminalBellEffect({ sequence: '7', terminalId: 'terminal-a' })).toBe(true);
		expect(view().admitTerminalBellEffect({ sequence: '6', terminalId: 'terminal-a' })).toBe(true);
		expect(view().admitTerminalBellEffect({ sequence: '5', terminalId: 'terminal-b' })).toBe(true);
	});
});
