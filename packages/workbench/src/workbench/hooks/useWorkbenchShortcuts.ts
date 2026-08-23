import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import {
	resolveWorkbenchShortcut,
	type WorkbenchShortcutBinding,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';

export function useWorkbenchShortcuts(
	active: boolean,
	platform: WorkbenchShortcutPlatform,
	bindings: WorkbenchShortcutBindings,
	invokeShortcut: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean,
): React.RefObject<HTMLDivElement | null> {
	const rootReference = React.useRef<HTMLDivElement>(null);
	const handleKeyDown = useMemoizedFn((event: KeyboardEvent): void => {
		if (!active) return;
		const binding = resolveWorkbenchShortcut(event, platform, bindings);
		if (binding === null || !invokeShortcut(binding, 'keyboard')) return;
		event.preventDefault();
		event.stopPropagation();
	});
	React.useEffect(() => {
		const ownerDocument = rootReference.current?.ownerDocument;
		if (ownerDocument === undefined) return;
		ownerDocument.addEventListener('keydown', handleKeyDown, true);
		return () => ownerDocument.removeEventListener('keydown', handleKeyDown, true);
	}, [handleKeyDown]);
	return rootReference;
}
