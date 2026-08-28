// @vitest-environment jsdom

import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../../terminal-presentation';
import type { WorkbenchTerminalSurfaceProps } from '../../../types/workbench-terminal-surface';
import { registerWorkbenchActiveTerminalActions } from '../../../actions/registerWorkbenchActiveTerminalActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
} from '../../../actions/workbench-action-ids';
import { WorkbenchProjectWorkspaceRuntime } from '../../../model/WorkbenchProjectWorkspaceRuntime';
import { PanelWorkspaceStore } from '../model/PanelWorkspaceStore';
import { collectPanelLeaves, getDisplayedTerminal } from '../model/panel-workspace-state';
import { WorkbenchPanelWorkspace } from '../WorkbenchPanelWorkspace';

const surfaces = new Map<string, WorkbenchTerminalSurfaceProps>();

class TestResizeObserver {
	public constructor(_callback: ResizeObserverCallback) {}
	public disconnect(): void {}
	public observe(_target: Element, _options?: ResizeObserverOptions): void {}
	public unobserve(_target: Element): void {}
}

function TestTerminalSurface(properties: WorkbenchTerminalSurfaceProps): ReactNode {
	surfaces.set(properties.presentation.presentationId, properties);
	return (
		<div
			data-auto-focus={properties.autoFocus}
			data-mode={properties.presentationMode}
			data-surface-presentation-id={properties.presentation.presentationId}
		/>
	);
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
					shortcutLabelForAction={() => undefined}
					terminalSurface={TestTerminalSurface}
				/>
			</TooltipProvider>
		</OrchbeeI18nProvider>,
	);
	return { actionRuntime, panelWorkspaceStore, parentActionScope, rendered };
}

beforeEach(() => {
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
});

afterEach(() => {
	cleanup();
	surfaces.clear();
	vi.unstubAllGlobals();
});

describe('WorkbenchPanelWorkspace', () => {
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
						shortcutLabelForAction={() => undefined}
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
						shortcutLabelForAction={() => undefined}
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
						shortcutLabelForAction={() => undefined}
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
		act(() => surfaces.get(maximizedPresentationId)?.onPresentationReady('activation-ready'));

		expect(rendered.container.querySelector('[data-terminal-tab-id="tab-b"]')).toBeNull();
		expect(
			rendered.container
				.querySelector('[data-terminal-tab-id="tab-a"]')
				?.getAttribute('data-presentation-mode'),
		).toBe('active');

		actionRuntime.dispose();
	});
});
