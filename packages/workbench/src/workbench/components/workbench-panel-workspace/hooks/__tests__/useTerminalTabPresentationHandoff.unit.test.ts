// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../../../terminal-presentation';
import type { TabPanelWorkspace } from '../../model/panel-workspace-state';
import { useTerminalTabPresentationHandoff } from '../useTerminalTabPresentationHandoff';

function tab(tabId: string, presentationId: string): TabPanelWorkspace {
	return {
		focusedPanelId: `${tabId}-panel`,
		maximizedPanelId: null,
		root: {
			activeChildPresentationId: null,
			childPresentations: [],
			id: `${tabId}-panel`,
			kind: 'panel',
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: null,
				launchCwd: null,
				lifecycle: { kind: 'preview' },
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
		act(() => {
			result.current.reportPresentationReady(
				epoch,
				terminalPresentationId('presentation-second'),
				'display-ready',
			);
		});
		expect(result.current.state).toMatchObject({ displayedTabId: 'second', kind: 'activating' });
		act(() => {
			result.current.reportPresentationReady(
				epoch,
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
					tabs: [first, second],
					terminalSurfaceAvailable: false,
				}),
			{ initialProps: { activeTab: first } },
		);

		rerender({ activeTab: second });
		expect(result.current.state).toMatchObject({ displayedTabId: 'second', kind: 'settled' });
	});
});
