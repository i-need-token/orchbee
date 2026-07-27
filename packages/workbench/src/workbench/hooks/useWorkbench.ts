import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { ActionExecution, ActionRuntime, ActionScope } from '@orchbee/actions';
import type { TabStripNewTabRequest, TabStripOrientation } from '@orchbee/tab-strip';
import * as React from 'react';

import {
	readWorkbenchLeftPanelWidth,
	readWorkbenchSidebarVisibility,
	readWorkbenchTabStripOrientation,
	writeWorkbenchLeftPanelWidth,
	writeWorkbenchSidebarVisibility,
	writeWorkbenchTabStripOrientation,
	type WorkbenchLayoutStorage,
} from '../../workbench-layout-storage';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type {
	WorkbenchHostActionCallback,
	WorkbenchTerminalCreateCallback,
} from '../types/workbench-host-actions';
import { formatWorkbenchShortcut, WORKBENCH_SHORTCUTS } from '../utils/workbench-shortcuts';
import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';
import { useWorkbenchActions } from './useWorkbenchActions';
import { useWorkbenchShortcuts } from './useWorkbenchShortcuts';

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

interface WorkbenchView {
	readonly actionScope: ActionScope;
	readonly isSidebarVisible: boolean;
	readonly leftPanelSize: string;
	readonly newTerminal: (() => void) | undefined;
	readonly newTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly newTerminalLabel: string;
	readonly newTerminalShortcutId: string;
	readonly newTerminalShortcutLabel: string;
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	readonly onLeftPanelResize: (size: ResizablePanelSize) => void;
	readonly sidebarShortcutId: string;
	readonly sidebarShortcutLabel: string;
	readonly sidebarToggleLabel: string;
	readonly sidebarLabel: string;
	readonly projectLabel: string;
	readonly searchLabel: string;
	readonly search: (() => void) | undefined;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string;
	readonly settingsLabel: string;
	readonly settings: (() => void) | undefined;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string;
	readonly tabStripOrientation: TabStripOrientation;
	readonly onTabStripOrientationChange: (orientation: TabStripOrientation) => void;
	readonly verticalTabStripCollapsed: boolean;
	readonly onVerticalTabStripCollapsedChange: (collapsed: boolean) => void;
	readonly toggleSidebar: () => void;
}

interface UseWorkbenchOptions {
	readonly actionRuntime: ActionRuntime;
	readonly onNewTerminal: WorkbenchTerminalCreateCallback | undefined;
	readonly onSearch: WorkbenchHostActionCallback | undefined;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly storage: WorkbenchLayoutStorage | undefined;
}

export function useWorkbench({
	actionRuntime,
	onNewTerminal,
	onSearch,
	onSettings,
	shortcutPlatform,
	storage,
}: UseWorkbenchOptions): WorkbenchView {
	const intl = useOrchbeeIntl();
	const [isSidebarVisible, setIsSidebarVisible] = React.useState(() =>
		readWorkbenchSidebarVisibility(storage),
	);
	const [tabStripOrientation, setTabStripOrientationState] = React.useState<TabStripOrientation>(
		() => readWorkbenchTabStripOrientation(storage),
	);
	const [verticalTabStripCollapsed, setVerticalTabStripCollapsed] = React.useState(false);
	const [defaultLeftPanelWidth] = React.useState(() => readWorkbenchLeftPanelWidth(storage));
	const leftPanelWidth = React.useRef(defaultLeftPanelWidth);
	const toggleSidebar = React.useCallback(() => {
		setIsSidebarVisible((currentVisibility) => {
			const nextVisibility = !currentVisibility;
			writeWorkbenchSidebarVisibility(storage, nextVisibility);
			return nextVisibility;
		});
	}, [storage]);
	const setTabStripOrientation = React.useCallback(
		(orientation: TabStripOrientation) => {
			setTabStripOrientationState(orientation);
			writeWorkbenchTabStripOrientation(storage, orientation);
		},
		[storage],
	);
	const actions = useWorkbenchActions({
		isSidebarVisible,
		onNewTerminal,
		onSearch,
		onSettings,
		runtime: actionRuntime,
		setTabStripOrientation,
		tabStripOrientation,
		toggleSidebar,
	});

	useWorkbenchShortcuts(shortcutPlatform, actions.invokeIfEnabled);

	return {
		actionScope: actions.scope,
		isSidebarVisible,
		leftPanelSize: `${leftPanelWidth.current}px`,
		newTerminal:
			onNewTerminal === undefined
				? undefined
				: () => {
						actions.invoke(WORKBENCH_ACTION_IDS.createTerminal, { groupId: null }, 'button');
					},
		newTab:
			onNewTerminal === undefined
				? undefined
				: (request) =>
						actions.invoke(
							WORKBENCH_ACTION_IDS.createTerminal,
							{
								groupId: request.groupId,
								...(request.index === undefined ? {} : { index: request.index }),
							},
							request.source,
						),
		newTerminalLabel: intl.formatMessage(workbenchMessages.newTerminal),
		newTerminalShortcutId: WORKBENCH_SHORTCUTS.newTerminal.id,
		newTerminalShortcutLabel: formatWorkbenchShortcut('newTerminal', shortcutPlatform),
		onLayoutChanged: (_layout, meta) => {
			if (meta.isUserInteraction) {
				writeWorkbenchLeftPanelWidth(storage, leftPanelWidth.current);
			}
		},
		onLeftPanelResize: (size) => {
			leftPanelWidth.current = size.inPixels;
		},
		sidebarShortcutId: WORKBENCH_SHORTCUTS.toggleSidebar.id,
		sidebarShortcutLabel: formatWorkbenchShortcut('toggleSidebar', shortcutPlatform),
		sidebarToggleLabel: intl.formatMessage(
			isSidebarVisible ? workbenchMessages.hideSidebar : workbenchMessages.showSidebar,
		),
		sidebarLabel: intl.formatMessage(workbenchMessages.sidebarLabel),
		projectLabel: intl.formatMessage(workbenchMessages.project),
		search:
			onSearch === undefined
				? undefined
				: () => {
						actions.invoke(WORKBENCH_ACTION_IDS.openSearch, undefined, 'button');
					},
		searchLabel: intl.formatMessage(workbenchMessages.search),
		searchShortcutId: WORKBENCH_SHORTCUTS.search.id,
		searchShortcutLabel: formatWorkbenchShortcut('search', shortcutPlatform),
		settings:
			onSettings === undefined
				? undefined
				: () => {
						actions.invoke(WORKBENCH_ACTION_IDS.openSettings, undefined, 'button');
					},
		settingsLabel: intl.formatMessage(workbenchMessages.settings),
		settingsShortcutId: WORKBENCH_SHORTCUTS.settings.id,
		settingsShortcutLabel: formatWorkbenchShortcut('settings', shortcutPlatform),
		tabStripOrientation,
		onTabStripOrientationChange: (orientation) => {
			actions.invoke(WORKBENCH_ACTION_IDS.setTabStripOrientation, { orientation }, 'context-menu');
		},
		verticalTabStripCollapsed,
		onVerticalTabStripCollapsedChange: setVerticalTabStripCollapsed,
		toggleSidebar: () => {
			actions.invoke(WORKBENCH_ACTION_IDS.toggleSidebar, undefined, 'button');
		},
	};
}
