import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import {
	resolveWorkbenchShortcut,
	type WorkbenchShortcutBinding,
} from '../utils/workbench-shortcuts';

export function useWorkbenchShortcuts(
	platform: WorkbenchShortcutPlatform,
	invokeShortcut: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean,
): React.KeyboardEventHandler<HTMLDivElement> {
	return useMemoizedFn((event: React.KeyboardEvent<HTMLDivElement>): void => {
		const binding = resolveWorkbenchShortcut(event.nativeEvent, platform);
		if (binding === null || !invokeShortcut(binding, 'keyboard')) return;
		event.preventDefault();
		event.stopPropagation();
	});
}
