// @vitest-environment jsdom

import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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
	type FixtureRecord,
} from '../../../../__tests__/server-workbench-snapshot';
import type { WorkbenchTerminalSurfaceProps } from '../../../types/workbench-terminal-surface';
import type { WorkbenchAuthority } from '../../../types/workbench-authority';
import { registerWorkbenchActiveTerminalActions } from '../../../actions/registerWorkbenchActiveTerminalActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
} from '../../../actions/workbench-action-ids';
import type { WorkbenchProjectWorkspaceRuntime } from '../../../model/WorkbenchProjectWorkspaceRuntime';
import { WorkbenchPanelWorkspace } from '../WorkbenchPanelWorkspace';
import { inertWorkbenchAuthority } from '../../../../__tests__/inertWorkbenchAuthority';

const surfaces = new Map<string, WorkbenchTerminalSurfaceProps>();
const surfacesByMode = new Map<string, WorkbenchTerminalSurfaceProps>();

const TAB_A = fixtureTabId(1);
const TAB_B = fixtureTabId(2);
/** Panel 1 beside Panels 2 and 3, which share the right side. */
const THREE_PANEL_ROOT = snapshotSplit(
	1,
	'horizontal',
	snapshotPanel(1),
	snapshotSplit(2, 'horizontal', snapshotPanel(2), snapshotPanel(3)),
);
const TWO_PANEL_ROOT = snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(3));

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
	return (
		<div
			data-auto-focus={properties.autoFocus}
			data-mode={properties.presentationMode}
			data-surface-presentation-id={properties.presentation.presentationId}
		/>
	);
}

function unexpected(): Promise<never> {
	return Promise.reject(new Error('The test did not expect this authority call.'));
}

/** The runtime a window holds when the Server publishes Workspace 1 with `tabs`. */
function publishedRuntime(tabs: readonly FixtureRecord[]): WorkbenchProjectWorkspaceRuntime {
	return createSnapshotRuntime(oneWorkspaceSnapshot(1, tabs));
}

function renderWorkspace(projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime) {
	const actionRuntime = new ActionRuntime();
	const parentActionScope = actionRuntime.createScope('test.workbench');
	const context = activeTerminalContext(projectWorkspaceRuntime);
	const change = vi.fn<WorkbenchAuthority['change']>(() => Promise.resolve({ kind: 'completed' }));
	const authority = inertWorkbenchAuthority({ change });
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
					authority={authority}
					createPanelTerminal={unexpected}
					panelWorkspaceStore={context.panelWorkspaceStore}
					parentActionScope={parentActionScope}
					presentationAgentStatuses={new Map()}
					terminalContextId={context.terminalContextId}
					shortcutLabelForAction={() => undefined}
					terminalAutoFocusAllowed
					terminalFocusRequest={null}
					terminalSurface={TestTerminalSurface}
				/>
			</TooltipProvider>
		</OrchbeeI18nProvider>,
	);
	return {
		actionRuntime,
		change,
		panelWorkspaceStore: context.panelWorkspaceStore,
		parentActionScope,
		rendered,
		tabModel: context.tabModel,
		terminalContextId: context.terminalContextId,
	};
}

/** Three Panels with Panel 3 focused, as after splitting to the right twice. */
function renderThreePanelWorkspace() {
	const projectWorkspaceRuntime = publishedRuntime([snapshotTab(1, THREE_PANEL_ROOT)]);
	activeTerminalContext(projectWorkspaceRuntime).panelWorkspaceStore.transition({
		panelId: fixturePanelId(3),
		tabId: TAB_A,
		type: 'panel-focused',
	});
	return renderWorkspace(projectWorkspaceRuntime);
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
	vi.restoreAllMocks();
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
		expect(panelWorkspaceStore.getSnapshot().tabs[0]?.focusedPanelId).toBe(fixturePanelId(1));
		expect(workspace.hasAttribute('data-workbench-keyboard-capture')).toBe(false);
		expect(
			rendered.container.querySelector('[data-panel-visual-selection-target="true"]'),
		).toBeNull();
		const activeElement = document.activeElement;
		expect(
			activeElement instanceof HTMLElement ? activeElement.dataset['panelId'] : undefined,
		).toBe(fixturePanelId(1));

		actionRuntime.dispose();
	});

	it('swaps with a primary-click target through the exact lower Panel Action', async () => {
		const {
			actionRuntime,
			change,
			panelWorkspaceStore,
			parentActionScope,
			rendered,
			terminalContextId,
		} = renderThreePanelWorkspace();
		await act(async () => {
			await actionRuntime.execute(WORKBENCH_ACTION_IDS.swapActivePanelVisible, undefined, {
				scope: parentActionScope,
				source: 'keyboard',
			}).completion;
		});
		const target = rendered.container.querySelector<HTMLElement>(
			`[data-panel-visual-selection-target="true"][data-panel-id="${fixturePanelId(2)}"]`,
		);
		if (target === null) throw new Error('Expected the Panel swap target.');

		fireEvent.pointerDown(target, { button: 0 });

		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			panelId: fixturePanelId(3),
			tabId: TAB_A,
			targetPanelId: fixturePanelId(2),
			terminalContextId,
			type: 'panels-swapped',
		});
		expect(panelWorkspaceStore.getSnapshot().tabs[0]?.focusedPanelId).toBe(fixturePanelId(3));
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

		act(() => {
			panelWorkspaceStore.transition({
				panelId: fixturePanelId(1),
				tabId: TAB_A,
				type: 'panel-focused',
			});
		});

		expect(
			rendered.container.querySelector('[data-panel-visual-selection-target="true"]'),
		).toBeNull();
		expect(panelWorkspaceStore.getSnapshot().tabs[0]?.focusedPanelId).toBe(fixturePanelId(1));

		actionRuntime.dispose();
	});

	it('keeps the outgoing frame until display readiness and delays focus until activation', () => {
		const { actionRuntime, rendered, tabModel } = renderWorkspace(
			publishedRuntime([snapshotTab(1), snapshotTab(2)]),
		);
		const targetPresentationId = fixturePresentationId(2);

		act(() => tabModel.selectTab(TAB_B));
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_A}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('retained');
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_B}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('incoming');
		expect(surfaces.get(targetPresentationId)?.autoFocus).toBe(false);

		act(() => surfaces.get(targetPresentationId)?.onPresentationReady('display-ready'));
		expect(rendered.container.querySelector(`[data-terminal-tab-id="${TAB_A}"]`)).toBeNull();
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_B}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('incoming');
		expect(surfaces.get(targetPresentationId)?.autoFocus).toBe(false);

		act(() => surfaces.get(targetPresentationId)?.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_B}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('active');
		expect(surfaces.get(targetPresentationId)?.autoFocus).toBe(true);

		actionRuntime.dispose();
	});

	it('keeps the pre-maximize same-Tab layer visible until the maximized geometry is ready', () => {
		const { actionRuntime, panelWorkspaceStore, rendered } = renderWorkspace(
			publishedRuntime([snapshotTab(1, TWO_PANEL_ROOT)]),
		);
		const maximizedPresentationId = fixturePresentationId(3);

		act(() => {
			panelWorkspaceStore.transition({
				panelId: fixturePanelId(3),
				tabId: TAB_A,
				type: 'panel-maximization-toggled',
			});
		});
		const pendingLayers = [
			...rendered.container.querySelectorAll(`[data-terminal-tab-id="${TAB_A}"]`),
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
			[...rendered.container.querySelectorAll(`[data-terminal-tab-id="${TAB_A}"]`)].map((element) =>
				element.getAttribute('data-presentation-mode'),
			),
		).toEqual(['incoming']);
		act(() => incomingSurface.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_A}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('active');
		expect(surfaces.get(maximizedPresentationId)?.autoFocus).toBe(true);

		actionRuntime.dispose();
	});

	it('commits only the maximized Panel when returning to a split Tab', () => {
		const projectWorkspaceRuntime = publishedRuntime([
			snapshotTab(1, TWO_PANEL_ROOT),
			snapshotTab(2),
		]);
		activeTerminalContext(projectWorkspaceRuntime).panelWorkspaceStore.transition({
			panelId: fixturePanelId(3),
			tabId: TAB_A,
			type: 'panel-maximization-toggled',
		});
		const { actionRuntime, rendered, tabModel } = renderWorkspace(projectWorkspaceRuntime);
		const maximizedPresentationId = fixturePresentationId(3);
		const tabBPresentationId = fixturePresentationId(2);

		act(() => tabModel.selectTab(TAB_B));
		act(() => surfaces.get(tabBPresentationId)?.onPresentationReady('activation-ready'));
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_B}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('active');

		act(() => tabModel.selectTab(TAB_A));
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_B}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('retained');
		const incomingMaximizedSurface = surfacesByMode.get(`${maximizedPresentationId}:incoming`);
		if (incomingMaximizedSurface === undefined) {
			throw new Error('Expected the incoming maximized surface.');
		}
		act(() => incomingMaximizedSurface.onPresentationReady('activation-ready'));

		expect(rendered.container.querySelector(`[data-terminal-tab-id="${TAB_B}"]`)).toBeNull();
		expect(
			rendered.container
				.querySelector(`[data-terminal-tab-id="${TAB_A}"]`)
				?.getAttribute('data-presentation-mode'),
		).toBe('active');

		actionRuntime.dispose();
	});
});
