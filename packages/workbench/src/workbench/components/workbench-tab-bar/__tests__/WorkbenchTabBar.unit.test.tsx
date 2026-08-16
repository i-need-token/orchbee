// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel, TabStripOrientation } from '@orchbee/tab-strip';
import { useState } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { useWorkbenchTabStripMessages } from '../../../hooks/useWorkbenchTabStripMessages';
import { WorkbenchTabBar } from '../WorkbenchTabBar';

afterEach(cleanup);

class TestResizeObserver {
	public constructor(_callback: ResizeObserverCallback) {}

	public disconnect(): void {}

	public observe(_target: Element, _options?: ResizeObserverOptions): void {}

	public unobserve(_target: Element): void {}
}

beforeAll(() => {
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
});

afterAll(() => {
	vi.unstubAllGlobals();
});

function VerticalWorkbenchTabBar({
	actionRuntime,
	tabModel,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly tabModel: TabModel;
}) {
	const messages = useWorkbenchTabStripMessages();
	const parentActionScope = actionRuntime.createScope('workbench');
	const [verticalCollapsed, setVerticalCollapsed] = useState(false);

	return (
		<WorkbenchTabBar
			actionRuntime={actionRuntime}
			createCrossWindowDragRuntime={undefined}
			getCloseTabsConfirmation={() => null}
			onNewTerminal={undefined}
			onOrientationChange={() => undefined}
			onVerticalCollapsedChange={setVerticalCollapsed}
			orientation={TabStripOrientation.Vertical}
			parentActionScope={parentActionScope}
			reserveSidebarToggleSpace={false}
			tabModel={tabModel}
			tabStripMessages={messages}
			verticalCollapsed={verticalCollapsed}
		/>
	);
}

describe('Workbench browser behavior', () => {
	it('owns and updates the vertical tab-strip collapsed state', () => {
		const actionRuntime = new ActionRuntime();
		const tabModel = new TabModel({
			activeTabId: 'terminal-1',
			tabs: [{ group: null, id: 'terminal-1', pinned: false, title: 'Terminal 1' }],
		});

		render(
			<OrchbeeI18nProvider locale='en-US'>
				<VerticalWorkbenchTabBar actionRuntime={actionRuntime} tabModel={tabModel} />
			</OrchbeeI18nProvider>,
		);

		const tabStrip = screen.getByRole('tablist', { name: 'Terminal tabs' });
		const collapseButton = screen.getByRole('button', { name: 'Collapse tab strip' });
		fireEvent.mouseEnter(tabStrip);
		fireEvent.focus(collapseButton);
		fireEvent.click(collapseButton);

		expect(screen.getByRole('button', { name: 'Expand tab strip' })).toBeDefined();
		expect(tabStrip.getAttribute('data-expanded-on-hover')).toBeNull();
		expect(tabStrip.getAttribute('data-collapsed')).toBe('true');
	});
});
