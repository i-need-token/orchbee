import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { Workbench } from '@orchbee/workbench';
import * as React from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const responsiveTabs = [
	{ id: 'orchbee', label: '~/github/orchbee' },
	{ id: 'terminal', label: 'terminal/session-alpha' },
	{ id: 'frontend', label: 'agent/review-frontend' },
	{ id: 'localization', label: 'feature/localization-contract' },
	{ id: 'tests', label: 'tests/workbench' },
	{ id: 'docs', label: 'docs/frontend' },
	{ id: 'build', label: 'build/web-production' },
	{ id: 'research', label: 'research/icon-systems' },
] as const;

function ResponsiveTabsStory(): React.ReactNode {
	const [workspace, setWorkspace] = React.useState<{
		readonly activeTabId: string | undefined;
		readonly tabs: readonly { readonly id: string; readonly label: string }[];
	}>({ activeTabId: 'frontend', tabs: responsiveTabs });
	const nextTabNumber = React.useRef(responsiveTabs.length + 1);

	return (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench
				activeTabId={workspace.activeTabId}
				onActiveTabChange={(activeTabId) => {
					setWorkspace((currentWorkspace) => ({ ...currentWorkspace, activeTabId }));
				}}
				onTabClose={(tabId) => {
					setWorkspace((currentWorkspace) => {
						const tabIndex = currentWorkspace.tabs.findIndex((tab) => tab.id === tabId);
						if (tabIndex === -1) return currentWorkspace;

						const tabs = currentWorkspace.tabs.filter((tab) => tab.id !== tabId);
						const activeTabId =
							currentWorkspace.activeTabId === tabId
								? tabs[Math.min(tabIndex, tabs.length - 1)]?.id
								: currentWorkspace.activeTabId;
						return { activeTabId, tabs };
					});
				}}
				onTabCreate={() => {
					const tabNumber = nextTabNumber.current;
					nextTabNumber.current += 1;
					const tab = { id: `preview-${tabNumber}`, label: `preview/tab-${tabNumber}` };
					setWorkspace((currentWorkspace) => ({
						activeTabId: tab.id,
						tabs: [...currentWorkspace.tabs, tab],
					}));
				}}
				onTabMove={(tabId, targetIndex) => {
					setWorkspace((currentWorkspace) => {
						const sourceIndex = currentWorkspace.tabs.findIndex((tab) => tab.id === tabId);
						if (sourceIndex === -1 || sourceIndex === targetIndex) return currentWorkspace;

						const tabs = [...currentWorkspace.tabs];
						const [tab] = tabs.splice(sourceIndex, 1);
						if (tab === undefined) return currentWorkspace;
						tabs.splice(targetIndex, 0, tab);
						return { ...currentWorkspace, tabs };
					});
				}}
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
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
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
