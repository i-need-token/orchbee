import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import {
	matchesWorkbenchShortcut,
	type WorkbenchShortcutAction,
} from '../utils/workbench-shortcuts';

export function useWorkbenchShortcuts(
	platform: WorkbenchShortcutPlatform,
	onCloseTab: (() => void) | undefined,
	onNewTerminal: (() => void) | undefined,
	onSearch: (() => void) | undefined,
	onSettings: (() => void) | undefined,
	onToggleSidebar: () => void,
): void {
	React.useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent): void => {
			let action: WorkbenchShortcutAction | undefined;
			if (matchesWorkbenchShortcut(event, platform, 'toggleSidebar')) {
				action = 'toggleSidebar';
			} else if (
				onCloseTab !== undefined &&
				matchesWorkbenchShortcut(event, platform, 'closeTab')
			) {
				action = 'closeTab';
			} else if (
				onNewTerminal !== undefined &&
				matchesWorkbenchShortcut(event, platform, 'newTerminal')
			) {
				action = 'newTerminal';
			} else if (onSearch !== undefined && matchesWorkbenchShortcut(event, platform, 'search')) {
				action = 'search';
			} else if (
				onSettings !== undefined &&
				matchesWorkbenchShortcut(event, platform, 'settings')
			) {
				action = 'settings';
			}

			if (action === undefined) return;
			event.preventDefault();

			switch (action) {
				case 'closeTab':
					onCloseTab?.();
					break;
				case 'newTerminal':
					onNewTerminal?.();
					break;
				case 'search':
					onSearch?.();
					break;
				case 'settings':
					onSettings?.();
					break;
				case 'toggleSidebar':
					onToggleSidebar();
					break;
			}
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [onCloseTab, onNewTerminal, onSearch, onSettings, onToggleSidebar, platform]);
}
