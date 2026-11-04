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
	creationEnabled = true,
	tabModel,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly creationEnabled?: boolean | undefined;
	readonly tabModel: TabModel;
}) {
	const messages = useWorkbenchTabStripMessages();
	const parentActionScope = actionRuntime.createScope('workbench');
	const [verticalCollapsed, setVerticalCollapsed] = useState(false);

	return (
		<WorkbenchTabBar
			actionRuntime={actionRuntime}
			focusRequest={0}
			getTabCreationAvailability={() =>
				creationEnabled
					? { kind: 'enabled' }
					: { kind: 'disabled', reason: { id: 'creation-unavailable' } }
			}
			onNewTerminal={undefined}
			onOrientationChange={() => undefined}
			onSetTabTitle={() => undefined}
			onTabsChange={undefined}
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
	it.each([
		['en-US', 'New Tab', 'Loading…'],
		['zh-CN', '新标签页', '正在加载…'],
	] as const)(
		'localizes empty and loading Tab labels in %s',
		(locale, emptyTitle, loadingTitle) => {
			const actionRuntime = new ActionRuntime();
			const tabModel = new TabModel({
				activeTabId: 'empty',
				tabs: [
					{ id: 'empty', title: '', pinned: false, group: null },
					{
						id: 'loading',
						title: '',
						pinned: false,
						group: null,
						shouldRenderLoadingTitle: true,
						networkState: 'loading',
					},
					{ id: 'named', title: 'User title', pinned: false, group: null },
				],
			});
			try {
				const mounted = render(
					<OrchbeeI18nProvider locale={locale}>
						<VerticalWorkbenchTabBar actionRuntime={actionRuntime} tabModel={tabModel} />
					</OrchbeeI18nProvider>,
				);
				const labels = () => screen.getAllByRole('tab').map((tab) => tab.textContent);
				expect(labels()).toEqual([emptyTitle, loadingTitle, 'User title']);
				mounted.rerender(
					<OrchbeeI18nProvider locale={locale === 'en-US' ? 'zh-CN' : 'en-US'}>
						<VerticalWorkbenchTabBar actionRuntime={actionRuntime} tabModel={tabModel} />
					</OrchbeeI18nProvider>,
				);
				expect(labels()).toEqual(
					locale === 'en-US'
						? ['新标签页', '正在加载…', 'User title']
						: ['New Tab', 'Loading…', 'User title'],
				);
			} finally {
				cleanup();
				actionRuntime.dispose();
			}
		},
	);

	it('disables the visible new-tab control when creation is unavailable', () => {
		const actionRuntime = new ActionRuntime();
		const tabModel = new TabModel({ activeTabId: null, tabs: [] });

		render(
			<OrchbeeI18nProvider locale='en-US'>
				<VerticalWorkbenchTabBar
					actionRuntime={actionRuntime}
					creationEnabled={false}
					tabModel={tabModel}
				/>
			</OrchbeeI18nProvider>,
		);

		expect(screen.getByRole('button', { name: 'New Tab' }).hasAttribute('disabled')).toBe(true);
	});

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
