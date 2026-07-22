import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import {
	readWorkbenchLeftPanelWidth,
	readWorkbenchSidebarVisibility,
	writeWorkbenchLeftPanelWidth,
	writeWorkbenchSidebarVisibility,
	type WorkbenchLayoutStorage,
} from '../../workbench-layout-storage';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { formatWorkbenchShortcut, WORKBENCH_SHORTCUTS } from '../utils/workbench-shortcuts';
import { useWorkbenchShortcuts } from './useWorkbenchShortcuts';

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

interface WorkbenchView {
	readonly closeTabShortcutLabel: string;
	readonly isSidebarVisible: boolean;
	readonly leftPanelSize: string;
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
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string;
	readonly settingsLabel: string;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string;
	readonly tabListLabel: string;
	readonly tabCloseLayoutResetSequence: number;
	readonly toggleSidebar: () => void;
}

interface UseWorkbenchOptions {
	readonly activeTabId: string | undefined;
	readonly onNewTerminal: (() => void) | undefined;
	readonly onSearch: (() => void) | undefined;
	readonly onSettings: (() => void) | undefined;
	readonly onTabClose: ((tabId: string) => void) | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly storage: WorkbenchLayoutStorage | undefined;
}

export function useWorkbench({
	activeTabId,
	onNewTerminal,
	onSearch,
	onSettings,
	onTabClose,
	shortcutPlatform,
	storage,
}: UseWorkbenchOptions): WorkbenchView {
	const intl = useOrchbeeIntl();
	const [isSidebarVisible, setIsSidebarVisible] = React.useState(() =>
		readWorkbenchSidebarVisibility(storage),
	);
	const [defaultLeftPanelWidth] = React.useState(() => readWorkbenchLeftPanelWidth(storage));
	const leftPanelWidth = React.useRef(defaultLeftPanelWidth);
	const [tabCloseLayoutResetSequence, setTabCloseLayoutResetSequence] = React.useState(0);
	const toggleSidebar = React.useCallback(() => {
		setIsSidebarVisible((currentVisibility) => {
			const nextVisibility = !currentVisibility;
			writeWorkbenchSidebarVisibility(storage, nextVisibility);
			return nextVisibility;
		});
	}, [storage]);

	const closeActiveTab = React.useMemo(
		() =>
			activeTabId === undefined || onTabClose === undefined
				? undefined
				: () => {
						setTabCloseLayoutResetSequence((currentSequence) => currentSequence + 1);
						onTabClose(activeTabId);
					},
		[activeTabId, onTabClose],
	);

	useWorkbenchShortcuts(
		shortcutPlatform,
		closeActiveTab,
		onNewTerminal,
		onSearch,
		onSettings,
		toggleSidebar,
	);

	return {
		closeTabShortcutLabel: formatWorkbenchShortcut('closeTab', shortcutPlatform),
		isSidebarVisible,
		leftPanelSize: `${leftPanelWidth.current}px`,
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
		searchLabel: intl.formatMessage(workbenchMessages.search),
		searchShortcutId: WORKBENCH_SHORTCUTS.search.id,
		searchShortcutLabel: formatWorkbenchShortcut('search', shortcutPlatform),
		settingsLabel: intl.formatMessage(workbenchMessages.settings),
		settingsShortcutId: WORKBENCH_SHORTCUTS.settings.id,
		settingsShortcutLabel: formatWorkbenchShortcut('settings', shortcutPlatform),
		tabListLabel: intl.formatMessage(workbenchMessages.tabsLabel),
		tabCloseLayoutResetSequence,
		toggleSidebar,
	};
}
