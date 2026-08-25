// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, cleanup, render } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../../terminal-presentation';
import type { WorkbenchTerminalSurfaceProps } from '../../../types/workbench-terminal-surface';
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

beforeEach(() => {
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
});

afterEach(() => {
	cleanup();
	surfaces.clear();
	vi.unstubAllGlobals();
});

describe('WorkbenchPanelWorkspace', () => {
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
