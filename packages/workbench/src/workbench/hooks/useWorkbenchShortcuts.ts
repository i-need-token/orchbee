import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import {
	resolveWorkbenchShortcut,
	type WorkbenchShortcutBinding,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';

export function useWorkbenchShortcuts(
	platform: WorkbenchShortcutPlatform,
	bindings: WorkbenchShortcutBindings,
	invokeShortcut: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean,
): React.KeyboardEventHandler<HTMLDivElement> {
	return useMemoizedFn((event: React.KeyboardEvent<HTMLDivElement>): void => {
		const binding = resolveWorkbenchShortcut(event.nativeEvent, platform, bindings);
		if (binding === null || !invokeShortcut(binding, 'keyboard')) return;
		event.preventDefault();
		event.stopPropagation();
	});
}
