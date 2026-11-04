// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { TabPanelWorkspace } from '../../model/panel-workspace-state';
import { useTerminalTabPresentationHandoff } from '../useTerminalTabPresentationHandoff';

function tab(tabId: string, presentationId: string): TabPanelWorkspace {
	return {
		focusedPanelId: `${tabId}-panel`,
		maximizedPanelId: null,
		panelFocusHistory: [`${tabId}-panel`],
		root: {
			activeChildPresentationId: null,
			childPresentations: [],
			id: `${tabId}-panel`,
			kind: 'panel',
			primaryPresentation: {
				currentCwd: null,
				lifecycle: { kind: 'running' },
				naturalTitle: null,
				presentationId: terminalPresentationId(presentationId),
				title: presentationId,
			},
		},
		tabId,
	};
}

describe('useTerminalTabPresentationHandoff', () => {
	it('retains the outgoing Tab until both target readiness barriers complete', () => {
		const first = tab('first', 'presentation-first');
		const second = tab('second', 'presentation-second');
		const tabs = [first, second];
		const { result, rerender } = renderHook(
			({ activeTab }) =>
				useTerminalTabPresentationHandoff({
					activeTab,
					presentationScopeId: 'scope-a',
					tabs,
					terminalSurfaceAvailable: true,
				}),
			{ initialProps: { activeTab: first } },
		);

		expect(result.current.state).toMatchObject({ displayedTabId: 'first', kind: 'settled' });
		rerender({ activeTab: second });
		expect(result.current.state).toMatchObject({
			displayedTabId: 'first',
			kind: 'preparing',
			targetTabId: 'second',
		});
		const epoch = result.current.state.epoch;
		if (result.current.state.kind === 'settled') throw new Error('Expected a pending target.');
		const targetRevision = result.current.state.targetRevision;
		act(() => {
			result.current.reportPresentationReady(
				epoch,
				'second',
				targetRevision,
				terminalPresentationId('presentation-second'),
				'display-ready',
			);
		});
		expect(result.current.state).toMatchObject({ displayedTabId: 'second', kind: 'activating' });
		act(() => {
			result.current.reportPresentationReady(
				epoch,
				'second',
				targetRevision,
				terminalPresentationId('presentation-second'),
				'activation-ready',
			);
		});
		expect(result.current.state).toMatchObject({ displayedTabId: 'second', kind: 'settled' });
	});

	it('commits synchronously when no terminal surface is installed', () => {
		const first = tab('first', 'presentation-first');
		const second = tab('second', 'presentation-second');
		const { result, rerender } = renderHook(
			({ activeTab }) =>
				useTerminalTabPresentationHandoff({
					activeTab,
					presentationScopeId: 'scope-a',
					tabs: [first, second],
					terminalSurfaceAvailable: false,
				}),
			{ initialProps: { activeTab: first } },
		);

		rerender({ activeTab: second });
		expect(result.current.state).toMatchObject({ displayedTabId: 'second', kind: 'settled' });
	});

	it('retains the prior same-Tab layout until the replacement revision is ready', () => {
		const initial = tab('first', 'presentation-first');
		const maximized = { ...initial, maximizedPanelId: initial.root.id };
		const { result, rerender } = renderHook(
			({ activeTab, tabs }) =>
				useTerminalTabPresentationHandoff({
					activeTab,
					presentationScopeId: 'scope-a',
					tabs,
					terminalSurfaceAvailable: true,
				}),
			{ initialProps: { activeTab: initial, tabs: [initial] } },
		);

		rerender({ activeTab: maximized, tabs: [maximized] });
		expect(result.current.displayedTab).toBe(initial);
		expect(result.current.state).toMatchObject({
			displayedTabId: 'first',
			kind: 'preparing',
			targetTabId: 'first',
		});
		if (result.current.state.kind === 'settled') throw new Error('Expected a pending target.');
		const epoch = result.current.state.epoch;
		const targetRevision = result.current.state.targetRevision;
		act(() => {
			result.current.reportPresentationReady(
				epoch,
				'first',
				targetRevision,
				terminalPresentationId('presentation-first'),
				'activation-ready',
			);
		});
		expect(result.current.displayedTab).toBe(maximized);
		expect(result.current.state).toMatchObject({ kind: 'settled' });
	});

	it('starts a handoff when the Workspace scope changes around the same Tab identity', () => {
		const firstScopeTab = tab('first', 'presentation-first');
		const secondScopeTab = tab('first', 'presentation-second');
		const { result, rerender } = renderHook(
			({ activeTab, presentationScopeId }) =>
				useTerminalTabPresentationHandoff({
					activeTab,
					presentationScopeId,
					tabs: [activeTab],
					terminalSurfaceAvailable: true,
				}),
			{
				initialProps: { activeTab: firstScopeTab, presentationScopeId: 'scope-a' },
			},
		);

		rerender({ activeTab: secondScopeTab, presentationScopeId: 'scope-b' });
		expect(result.current.displayedTab).toBe(firstScopeTab);
		expect(result.current.state).toMatchObject({
			displayedTabId: 'first',
			kind: 'preparing',
			targetTabId: 'first',
		});
	});
});
