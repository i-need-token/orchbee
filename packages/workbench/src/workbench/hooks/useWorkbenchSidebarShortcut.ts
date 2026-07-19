import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { matchesWorkbenchToggleSidebarShortcut } from '../utils/workbench-shortcuts';

export function useWorkbenchSidebarShortcut(
	platform: WorkbenchShortcutPlatform,
	onToggle: () => void,
): void {
	React.useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent): void => {
			if (!matchesWorkbenchToggleSidebarShortcut(event, platform)) return;
			event.preventDefault();
			onToggle();
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [onToggle, platform]);
}
