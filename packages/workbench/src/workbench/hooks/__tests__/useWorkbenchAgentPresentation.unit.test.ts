// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { act, render, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { PanelWorkspaceStore } from '../../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentSnapshot } from '../../types/workbench-agent-observation';
import { useWorkbenchAgentPresentation } from '../useWorkbenchAgentPresentation';

describe('useWorkbenchAgentPresentation', () => {
	it('shows a Tab’s Agent and state only while the Server observes one in it', () => {
		const presentationId = terminalPresentationId('agent-presentation-hook');
		const tabModel = new TabModel({
			activeTabId: 'agent-tab-hook',
			tabs: [{ group: null, id: 'agent-tab-hook', pinned: false, title: 'Terminal' }],
		});
		const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId('agent-presentation-dynamic'),
				initialPresentationId: presentationId,
				kind: 'preview',
			},
		});
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal',
			terminalPresentation: {
				createPresentationId: () => terminalPresentationId('agent-project-dynamic'),
				initialPresentationId: terminalPresentationId('agent-project-initial'),
				kind: 'preview',
			},
		});
		let snapshot: WorkbenchAgentSnapshot = {
			complete: true,
			presentations: new Map([
				[presentationId, { agent: 'claude', binding: 'terminal', sequence: 1n, state: 'working' }],
			]),
		};
		const listeners = new Set<() => void>();
		const source = {
			getSnapshot: () => snapshot,
			subscribe: (listener: () => void) => {
				listeners.add(listener);
				return () => {
					listeners.delete(listener);
				};
			},
		};

		const rendered = renderHook(
			() =>
				useWorkbenchAgentPresentation({
					navigateAgent: () => undefined,
					notificationObserver: undefined,
					panelWorkspaceStore,
					projectWorkspaceRuntime,
					root: { current: null },
					source,
					tabModel,
				}),
			{
				wrapper: ({ children }: { readonly children: ReactNode }) =>
					createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children),
			},
		);

		expect(rendered.result.current.tabStatusAdornments.get('agent-tab-hook')).toMatchObject({
			accessibleLabel: 'Claude Code: Working',
			kind: 'active',
		});
		const activeIcon = render(
			createElement('div', null, rendered.result.current.tabIdentityIcons.get('agent-tab-hook')),
		);
		expect(activeIcon.container.querySelector('[data-agent-provider-icon="claude"]')).toBeTruthy();

		// The Agent exited, so the Server reports none in the terminal.
		act(() => {
			snapshot = { complete: true, presentations: new Map() };
			for (const listener of listeners) listener();
		});

		expect(rendered.result.current.tabStatusAdornments.has('agent-tab-hook')).toBe(false);
		expect(rendered.result.current.tabIdentityIcons.has('agent-tab-hook')).toBe(false);
	});
});
