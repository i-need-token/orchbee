import { createOrchbeeIntl, type OrchbeeLocale, workbenchMessages } from '@orchbee/i18n';
import {
	createWorkbenchTabsState,
	reduceWorkbenchTabs,
	type WorkbenchTab,
	type WorkbenchTabColor,
	type WorkbenchTabGroup,
	type WorkbenchTabGroupColor,
	type WorkbenchTabGroupUpdate,
	type WorkbenchTabsAction,
	type WorkbenchTabsState,
} from '@orchbee/workbench';
import * as React from 'react';

interface WebApplicationTabs {
	readonly activeTabId: string | undefined;
	readonly onActiveTabChange: (tabId: string) => void;
	readonly onTabClose: (tabId: string) => void;
	readonly onTabColorChange: (tabId: string, color: WorkbenchTabColor) => void;
	readonly onTabCreate: (groupId?: string) => void;
	readonly onTabGroupAssign: (tabId: string, groupId: string | undefined) => void;
	readonly onTabGroupChange: (groupId: string, update: WorkbenchTabGroupUpdate) => void;
	readonly onTabGroupClose: (groupId: string) => void;
	readonly onTabGroupCollapseChange: (groupId: string, isCollapsed: boolean) => void;
	readonly onTabGroupCreate: (tabId: string) => void;
	readonly onTabGroupDelete: (groupId: string) => void;
	readonly onTabGroupMove: (groupId: string, targetIndex: number) => void;
	readonly onTabGroupUngroup: (groupId: string) => void;
	readonly onTabMove: (tabId: string, targetIndex: number, groupId: string | undefined) => void;
	readonly onTabPinChange: (tabId: string, isPinned: boolean) => void;
	readonly onTabRename: (tabId: string, label: string) => void;
	readonly onTabsClose: (tabIds: readonly string[]) => void;
	readonly tabGroups: readonly WorkbenchTabGroup[];
	readonly tabs: readonly WorkbenchTab[];
}

const TAB_GROUP_COLORS: readonly WorkbenchTabGroupColor[] = [
	'blue',
	'purple',
	'pink',
	'red',
	'orange',
	'yellow',
	'green',
	'cyan',
	'gray',
];

export function useWebApplicationTabs(
	locale: OrchbeeLocale,
	preparedTabs: readonly WorkbenchTab[],
): WebApplicationTabs {
	const intl = React.useMemo(() => createOrchbeeIntl(locale), [locale]);
	const nextGroupNumber = React.useRef(1);
	const nextTabNumber = React.useRef(preparedTabs.length + 1);
	const [workspace, setWorkspace] = React.useState<WorkbenchTabsState>(() =>
		createWorkbenchTabsState({ activeTabId: preparedTabs[0]?.id, tabs: preparedTabs }),
	);
	const dispatch = React.useCallback((action: WorkbenchTabsAction) => {
		setWorkspace((currentState) => reduceWorkbenchTabs(currentState, action));
	}, []);
	const onActiveTabChange = React.useCallback(
		(tabId: string) => dispatch({ kind: 'activate-tab', tabId }),
		[dispatch],
	);
	const onTabsClose = React.useCallback(
		(tabIds: readonly string[]) => dispatch({ kind: 'close-tabs', tabIds }),
		[dispatch],
	);
	const onTabClose = React.useCallback(
		(tabId: string) => dispatch({ kind: 'close-tabs', tabIds: [tabId] }),
		[dispatch],
	);
	const onTabColorChange = React.useCallback(
		(tabId: string, color: WorkbenchTabColor) => dispatch({ color, kind: 'set-tab-color', tabId }),
		[dispatch],
	);
	const onTabCreate = React.useCallback(
		(groupId?: string) => {
			const tabNumber = nextTabNumber.current;
			nextTabNumber.current += 1;
			dispatch({
				groupId,
				kind: 'insert-tab',
				tab: {
					icon: 'terminal',
					id: `untitled-${tabNumber}`,
					label: intl.formatMessage(workbenchMessages.untitledTab, { tabNumber }),
				},
			});
		},
		[dispatch, intl],
	);
	const onTabGroupAssign = React.useCallback(
		(tabId: string, groupId: string | undefined) =>
			dispatch({ groupId, kind: 'assign-tab-group', tabId }),
		[dispatch],
	);
	const onTabGroupChange = React.useCallback(
		(groupId: string, update: WorkbenchTabGroupUpdate) =>
			dispatch({ groupId, kind: 'update-tab-group', update }),
		[dispatch],
	);
	const onTabGroupCollapseChange = React.useCallback(
		(groupId: string, isCollapsed: boolean) =>
			dispatch({ groupId, isCollapsed, kind: 'set-tab-group-collapsed' }),
		[dispatch],
	);
	const onTabGroupClose = React.useCallback(
		(groupId: string) => dispatch({ groupId, kind: 'close-tab-group' }),
		[dispatch],
	);
	const onTabGroupCreate = React.useCallback(
		(tabId: string) => {
			const groupNumber = nextGroupNumber.current;
			nextGroupNumber.current += 1;
			dispatch({
				group: {
					color: TAB_GROUP_COLORS[(groupNumber - 1) % TAB_GROUP_COLORS.length] ?? 'blue',
					id: `group-${groupNumber}`,
					label: intl.formatMessage(workbenchMessages.tabGroupDefaultLabel, { groupNumber }),
				},
				kind: 'create-tab-group',
				tabIds: [tabId],
			});
		},
		[dispatch, intl],
	);
	const onTabGroupMove = React.useCallback(
		(groupId: string, targetIndex: number) =>
			dispatch({ groupId, kind: 'move-tab-group', targetIndex }),
		[dispatch],
	);
	const onTabGroupUngroup = React.useCallback(
		(groupId: string) => dispatch({ groupId, kind: 'ungroup-tab-group' }),
		[dispatch],
	);
	const onTabMove = React.useCallback(
		(tabId: string, targetIndex: number, groupId: string | undefined) =>
			dispatch({ groupId, kind: 'move-tab', tabId, targetIndex }),
		[dispatch],
	);
	const onTabPinChange = React.useCallback(
		(tabId: string, isPinned: boolean) => dispatch({ isPinned, kind: 'set-tab-pinned', tabId }),
		[dispatch],
	);
	const onTabRename = React.useCallback(
		(tabId: string, label: string) => dispatch({ kind: 'rename-tab', label, tabId }),
		[dispatch],
	);

	return {
		...workspace,
		onActiveTabChange,
		onTabClose,
		onTabColorChange,
		onTabCreate,
		onTabGroupAssign,
		onTabGroupChange,
		onTabGroupClose,
		onTabGroupCollapseChange,
		onTabGroupCreate,
		onTabGroupDelete: onTabGroupClose,
		onTabGroupMove,
		onTabGroupUngroup,
		onTabMove,
		onTabPinChange,
		onTabRename,
		onTabsClose,
	};
}
