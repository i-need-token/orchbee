import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import {
	resolveWorkbenchShortcut,
	type WorkbenchShortcutBinding,
} from '../utils/workbench-shortcuts';

export function useWorkbenchShortcuts(
	platform: WorkbenchShortcutPlatform,
	invokeShortcut: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean,
): void {
	React.useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent): void => {
			const binding = resolveWorkbenchShortcut(event, platform);
			if (binding === null || !invokeShortcut(binding, 'keyboard')) return;
			event.preventDefault();
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [invokeShortcut, platform]);
}
