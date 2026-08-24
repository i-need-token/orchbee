// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, cleanup, render } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../../terminal-presentation';
import type { WorkbenchTerminalSurfaceProps } from '../../../types/workbench-terminal-surface';
import { PanelWorkspaceStore } from '../model/PanelWorkspaceStore';
import { collectPanelLeaves, getDisplayedTerminal } from '../model/panel-workspace-state';
import { WorkbenchPanelWorkspace } from '../WorkbenchPanelWorkspace';

const surfaces = new Map<string, WorkbenchTerminalSurfaceProps>();

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

afterEach(() => {
	cleanup();
	surfaces.clear();
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
});
