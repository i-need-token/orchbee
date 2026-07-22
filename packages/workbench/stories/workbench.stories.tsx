import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	createWorkbenchTabsState,
	reduceWorkbenchTabs,
	Workbench,
	type WorkbenchTabsAction,
	type WorkbenchTabsState,
} from '@orchbee/workbench';
import * as React from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const responsiveTabs = [
	{ icon: 'terminal', id: 'orchbee', label: '~/github/orchbee' },
	{ icon: 'terminal', id: 'terminal', label: 'terminal/session-alpha' },
	{ icon: 'terminal', id: 'frontend', label: 'agent/review-frontend' },
	{ icon: 'terminal', id: 'localization', label: 'feature/localization-contract' },
	{ icon: 'terminal', id: 'tests', label: 'tests/workbench' },
	{ icon: 'terminal', id: 'docs', label: 'docs/frontend' },
	{ icon: 'terminal', id: 'build', label: 'build/web-production' },
	{ icon: 'terminal', id: 'research', label: 'research/icon-systems' },
] as const;

function ResponsiveTabsStory(): React.ReactNode {
	const [workspace, setWorkspace] = React.useState<WorkbenchTabsState>(() =>
		createWorkbenchTabsState({ activeTabId: 'frontend', tabs: responsiveTabs }),
	);
	const nextTabNumber = React.useRef(responsiveTabs.length + 1);
	const dispatch = (action: WorkbenchTabsAction) => {
		setWorkspace((currentState) => reduceWorkbenchTabs(currentState, action));
	};

	return (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench
				activeTabId={workspace.activeTabId}
				onActiveTabChange={(activeTabId) => {
					dispatch({ kind: 'activate-tab', tabId: activeTabId });
				}}
				onTabClose={(tabId) => {
					dispatch({ kind: 'close-tabs', tabIds: [tabId] });
				}}
				onTabColorChange={(tabId, color) => {
					dispatch({ color, kind: 'set-tab-color', tabId });
				}}
				onTabCreate={() => {
					const tabNumber = nextTabNumber.current;
					nextTabNumber.current += 1;
					const tab = {
						icon: 'terminal' as const,
						id: `preview-${tabNumber}`,
						label: `preview/tab-${tabNumber}`,
					};
					dispatch({ groupId: undefined, kind: 'insert-tab', tab });
				}}
				onTabMove={(tabId, targetIndex, groupId) => {
					dispatch({ groupId, kind: 'move-tab', tabId, targetIndex });
				}}
				onTabPinChange={(tabId, isPinned) => {
					dispatch({ isPinned, kind: 'set-tab-pinned', tabId });
				}}
				onTabRename={(tabId, label) => {
					dispatch({ kind: 'rename-tab', label, tabId });
				}}
				onTabsClose={(tabIds) => {
					dispatch({ kind: 'close-tabs', tabIds });
				}}
				onSearch={() => undefined}
				onSettings={() => undefined}
				shortcutPlatform='apple'
				tabs={workspace.tabs}
			/>
		</OrchbeeI18nProvider>
	);
}

const meta = {
	title: 'Workbench',
	component: Workbench,
	parameters: {
		layout: 'fullscreen',
	},
} satisfies Meta<typeof Workbench>;

export default meta;

type Story = StoryObj<typeof meta>;

export const EmptyPanels: Story = {
	args: {
		onSearch: () => undefined,
		onSettings: () => undefined,
		onTabCreate: () => undefined,
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const SimplifiedChineseSidebar: Story = {
	args: {
		onSearch: () => undefined,
		onSettings: () => undefined,
		onTabCreate: () => undefined,
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const ResponsiveTabs: Story = {
	args: {
		shortcutPlatform: 'apple',
	},
	render: () => <ResponsiveTabsStory />,
};
