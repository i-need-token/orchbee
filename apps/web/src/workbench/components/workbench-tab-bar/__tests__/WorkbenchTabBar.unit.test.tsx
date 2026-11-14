// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { cleanup, render, screen } from '@testing-library/react';
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

function HarnessWorkbenchTabBar({
	actionRuntime,
	creationEnabled = true,
	fill = false,
	tabModel,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly creationEnabled?: boolean | undefined;
	/** The mobile layout's vertical Tab list, which draws each title whole. */
	readonly fill?: boolean | undefined;
	readonly tabModel: TabModel;
}) {
	const messages = useWorkbenchTabStripMessages();
	const parentActionScope = actionRuntime.createScope('workbench');

	return (
		<WorkbenchTabBar
			actionRuntime={actionRuntime}
			fill={fill}
			focusRequest={0}
			getTabCreationAvailability={() =>
				creationEnabled
					? { kind: 'enabled' }
					: { kind: 'disabled', reason: { id: 'creation-unavailable' } }
			}
			onNewTerminal={undefined}
			onSetTabTitle={() => undefined}
			onTabsChange={undefined}
			parentActionScope={parentActionScope}
			reserveSidebarToggleSpace={false}
			tabModel={tabModel}
			tabStripMessages={messages}
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
						<HarnessWorkbenchTabBar actionRuntime={actionRuntime} fill tabModel={tabModel} />
					</OrchbeeI18nProvider>,
				);
				const labels = () => screen.getAllByRole('tab').map((tab) => tab.textContent);
				expect(labels()).toEqual([emptyTitle, loadingTitle, 'User title']);
				mounted.rerender(
					<OrchbeeI18nProvider locale={locale === 'en-US' ? 'zh-CN' : 'en-US'}>
						<HarnessWorkbenchTabBar actionRuntime={actionRuntime} fill tabModel={tabModel} />
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
				<HarnessWorkbenchTabBar
					actionRuntime={actionRuntime}
					creationEnabled={false}
					tabModel={tabModel}
				/>
			</OrchbeeI18nProvider>,
		);

		expect(screen.getByRole('button', { name: 'New Tab' }).hasAttribute('disabled')).toBe(true);
	});
});
