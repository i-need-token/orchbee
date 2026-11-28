import { useMemoizedFn } from '@orchbee/foundation/react';
import { useState, type RefObject } from 'react';

import type {
	FileTabMenuCommand,
	FileTabMenuEntry,
} from '../../../../../../../file-overlay-contract';
import { useFileOverlayHostCommands } from '../../../../../contexts/file-overlay-host-store';
import { activeFileTab } from '../utils/activeFileTab';

interface FileTabMenuState {
	readonly entries: readonly FileTabMenuEntry[];
	readonly open: boolean;
}

const CLOSED: FileTabMenuState = { entries: [], open: false };

interface FileTabMenu {
	/** Runs a chosen item once the active tab has the keys. */
	readonly choose: (entry: FileTabMenuCommand) => void;
	/** The items of the tab the menu opened on, kept while the menu closes. */
	readonly entries: readonly FileTabMenuEntry[];
	readonly finalFocus: () => HTMLElement | false;
	readonly onOpenChange: (open: boolean, details: { readonly event: Event }) => void;
	readonly open: boolean;
}

/**
 * The menu of the tab that a right-click, a long press or the context menu key reaches, as VS
 * Code's tab menu: it opens only on a tab, takes that tab's items as it opens, and acts on the tab
 * without showing it. VS Code focuses the active editor once its tab menu closes, so the active tab
 * takes the keys: before a chosen item runs, so that a tab the item closes hands them on as it
 * leaves, or as a dismissed menu returns them.
 */
export function useFileTabMenu(
	rowReference: RefObject<HTMLElement | null>,
	listReference: RefObject<HTMLElement | null>,
	tabs: readonly { readonly id: string }[],
): FileTabMenu {
	const commands = useFileOverlayHostCommands();
	const [menu, setMenu] = useState<FileTabMenuState>(CLOSED);
	const onOpenChange = useMemoizedFn((open: boolean, details: { readonly event: Event }) => {
		if (!open) {
			setMenu((current) => ({ ...current, open: false }));
			return;
		}
		const list = listReference.current;
		const { target } = details.event;
		if (list === null || !(target instanceof Element)) return;
		const tabElement = target.closest('[role="tab"]');
		const index = [...list.querySelectorAll('[role="tab"]')].findIndex(
			(element) => element === tabElement,
		);
		const tab = tabs[index];
		// VS Code's menu for the row's free end has no item Orchbee offers yet: its New Text File
		// waits for untitled files and its Open File... for Quick Open.
		if (tab === undefined) return;
		setMenu({ entries: commands.tabMenu(tab.id), open: true });
	});
	const choose = useMemoizedFn((entry: FileTabMenuCommand) => {
		const row = rowReference.current;
		if (row !== null) activeFileTab(row)?.focus({ preventScroll: true });
		entry.run();
	});
	const finalFocus = useMemoizedFn((): HTMLElement | false => {
		const row = rowReference.current;
		if (row === null) return false;
		const { activeElement, body } = row.ownerDocument;
		// A chosen item or a press outside has placed the keys already; Base UI would take them back.
		const placed =
			activeElement !== null &&
			activeElement !== body &&
			activeElement.closest('[role="menu"]') === null;
		return placed ? false : (activeFileTab(row) ?? false);
	});
	return { choose, entries: menu.entries, finalFocus, onOpenChange, open: menu.open };
}
