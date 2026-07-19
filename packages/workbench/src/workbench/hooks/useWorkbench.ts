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
import {
	formatWorkbenchToggleSidebarShortcut,
	WORKBENCH_TOGGLE_SIDEBAR_SHORTCUT_ID,
} from '../utils/workbench-shortcuts';
import { useWorkbenchSidebarShortcut } from './useWorkbenchSidebarShortcut';

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

interface WorkbenchView {
	readonly isSidebarVisible: boolean;
	readonly leftPanelSize: string;
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	readonly onLeftPanelResize: (size: ResizablePanelSize) => void;
	readonly sidebarShortcutId: string;
	readonly sidebarShortcutLabel: string;
	readonly sidebarToggleLabel: string;
	readonly tabListLabel: string;
	readonly toggleSidebar: () => void;
}

export function useWorkbench(
	storage: WorkbenchLayoutStorage | undefined,
	shortcutPlatform: WorkbenchShortcutPlatform,
): WorkbenchView {
	const intl = useOrchbeeIntl();
	const [isSidebarVisible, setIsSidebarVisible] = React.useState(() =>
		readWorkbenchSidebarVisibility(storage),
	);
	const [defaultLeftPanelWidth] = React.useState(() => readWorkbenchLeftPanelWidth(storage));
	const leftPanelWidth = React.useRef(defaultLeftPanelWidth);
	const toggleSidebar = React.useCallback(() => {
		setIsSidebarVisible((currentVisibility) => {
			const nextVisibility = !currentVisibility;
			writeWorkbenchSidebarVisibility(storage, nextVisibility);
			return nextVisibility;
		});
	}, [storage]);

	useWorkbenchSidebarShortcut(shortcutPlatform, toggleSidebar);

	return {
		isSidebarVisible,
		leftPanelSize: `${leftPanelWidth.current}px`,
		onLayoutChanged: (_layout, meta) => {
			if (meta.isUserInteraction) {
				writeWorkbenchLeftPanelWidth(storage, leftPanelWidth.current);
			}
		},
		onLeftPanelResize: (size) => {
			leftPanelWidth.current = size.inPixels;
		},
		sidebarShortcutId: WORKBENCH_TOGGLE_SIDEBAR_SHORTCUT_ID,
		sidebarShortcutLabel: formatWorkbenchToggleSidebarShortcut(shortcutPlatform),
		sidebarToggleLabel: intl.formatMessage(
			isSidebarVisible ? workbenchMessages.hideSidebar : workbenchMessages.showSidebar,
		),
		tabListLabel: intl.formatMessage(workbenchMessages.tabsLabel),
		toggleSidebar,
	};
}
