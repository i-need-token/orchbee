import { createOrchbeeIntl, type OrchbeeLocale, workbenchMessages } from '@orchbee/i18n';
import type { WorkbenchTab } from '@orchbee/workbench';
import * as React from 'react';

import { moveWorkbenchTab } from '../utils/moveWorkbenchTab';

interface WebApplicationTabs {
	readonly activeTabId: string | undefined;
	readonly onActiveTabChange: (tabId: string) => void;
	readonly onTabClose: (tabId: string) => void;
	readonly onTabCreate: () => void;
	readonly onTabMove: (tabId: string, targetIndex: number) => void;
	readonly tabs: readonly WorkbenchTab[];
}

export function useWebApplicationTabs(
	locale: OrchbeeLocale,
	preparedTabs: readonly WorkbenchTab[],
): WebApplicationTabs {
	const intl = React.useMemo(() => createOrchbeeIntl(locale), [locale]);
	const nextTabNumber = React.useRef(preparedTabs.length + 1);
	const [workspace, setWorkspace] = React.useState(() => ({
		activeTabId: preparedTabs[0]?.id,
		tabs: preparedTabs,
	}));
	const onActiveTabChange = React.useCallback((activeTabId: string) => {
		setWorkspace((currentWorkspace) => ({ ...currentWorkspace, activeTabId }));
	}, []);
	const onTabClose = React.useCallback((tabId: string) => {
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
	}, []);
	const onTabCreate = React.useCallback(() => {
		const tabNumber = nextTabNumber.current;
		nextTabNumber.current += 1;
		const tab = {
			id: `untitled-${tabNumber}`,
			label: intl.formatMessage(workbenchMessages.untitledTab, { tabNumber }),
		};
		setWorkspace((currentWorkspace) => ({
			activeTabId: tab.id,
			tabs: [...currentWorkspace.tabs, tab],
		}));
	}, [intl]);
	const onTabMove = React.useCallback((tabId: string, targetIndex: number) => {
		setWorkspace((currentWorkspace) => {
			const tabs = moveWorkbenchTab(currentWorkspace.tabs, tabId, targetIndex);
			return tabs === currentWorkspace.tabs ? currentWorkspace : { ...currentWorkspace, tabs };
		});
	}, []);

	return { ...workspace, onActiveTabChange, onTabClose, onTabCreate, onTabMove };
}
