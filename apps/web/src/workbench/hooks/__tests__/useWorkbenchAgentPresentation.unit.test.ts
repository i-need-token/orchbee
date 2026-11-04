// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, render, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchAgentSnapshot } from '../../types/workbench-agent-observation';
import { useWorkbenchAgentPresentation } from '../useWorkbenchAgentPresentation';

describe('useWorkbenchAgentPresentation', () => {
	it('shows a Tab’s Agent and state only while the Server observes one in it', () => {
		const presentationId = fixturePresentationId(1);
		const tabId = fixtureTabId(1);
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const terminalContext = activeTerminalContext(projectWorkspaceRuntime);
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
					projectWorkspaceRuntime,
					root: { current: null },
					source,
					terminalContext,
				}),
			{
				wrapper: ({ children }: { readonly children: ReactNode }) =>
					createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children),
			},
		);

		expect(rendered.result.current.tabStatusAdornments.get(tabId)).toMatchObject({
			accessibleLabel: 'Claude Code: Working',
			kind: 'active',
		});
		const activeIcon = render(
			createElement('div', null, rendered.result.current.tabIdentityIcons.get(tabId)),
		);
		expect(activeIcon.container.querySelector('[data-agent-provider-icon="claude"]')).toBeTruthy();

		// The Agent exited, so the Server reports none in the terminal.
		act(() => {
			snapshot = { complete: true, presentations: new Map() };
			for (const listener of listeners) listener();
		});

		expect(rendered.result.current.tabStatusAdornments.has(tabId)).toBe(false);
		expect(rendered.result.current.tabIdentityIcons.has(tabId)).toBe(false);
	});
});
