import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { isWorkbenchExclusiveFocusTarget } from '../utils/workbench-focus-dom';
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
	invokeConfiguredCommandKey: (event: KeyboardEvent) => boolean,
): React.RefObject<HTMLDivElement | null> {
	const rootReference = React.useRef<HTMLDivElement>(null);
	const handleKeyDown = useMemoizedFn((event: KeyboardEvent): void => {
		if (!active) return;
		if (isWorkbenchExclusiveFocusTarget(event.target)) return;
		const binding = resolveWorkbenchShortcut(event, platform, bindings);
		if (binding === null || !invokeShortcut(binding, 'keyboard')) return;
		event.preventDefault();
		event.stopPropagation();
	});
	// The window's capture runs before every document listener, so a configured command's chord
	// reaches neither a window default it took nor the terminal.
	const handleConfiguredCommandKeyDown = useMemoizedFn((event: KeyboardEvent): void => {
		if (!active) return;
		if (isWorkbenchExclusiveFocusTarget(event.target)) return;
		if (!invokeConfiguredCommandKey(event)) return;
		event.preventDefault();
		event.stopPropagation();
	});
	React.useEffect(() => {
		const ownerDocument = rootReference.current?.ownerDocument;
		if (ownerDocument === undefined) return;
		const ownerWindow = ownerDocument.defaultView;
		ownerWindow?.addEventListener('keydown', handleConfiguredCommandKeyDown, true);
		ownerDocument.addEventListener('keydown', handleKeyDown, true);
		return () => {
			ownerWindow?.removeEventListener('keydown', handleConfiguredCommandKeyDown, true);
			ownerDocument.removeEventListener('keydown', handleKeyDown, true);
		};
	}, [handleConfiguredCommandKeyDown, handleKeyDown]);
	return rootReference;
}
