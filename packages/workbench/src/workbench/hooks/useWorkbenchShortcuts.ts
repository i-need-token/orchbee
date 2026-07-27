import * as React from 'react';

import type { WorkbenchActionId } from '../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { resolveWorkbenchShortcut } from '../utils/workbench-shortcuts';

export function useWorkbenchShortcuts(
	platform: WorkbenchShortcutPlatform,
	invokeIfEnabled: (actionId: WorkbenchActionId, source: 'keyboard') => boolean,
): void {
	React.useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent): void => {
			const actionId = resolveWorkbenchShortcut(event, platform);
			if (actionId === null || !invokeIfEnabled(actionId, 'keyboard')) return;
			event.preventDefault();
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [invokeIfEnabled, platform]);
}
