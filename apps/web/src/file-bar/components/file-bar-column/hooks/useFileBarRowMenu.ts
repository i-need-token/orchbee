import { useMemoizedFn } from '@orchbee/foundation/react';
import { useRef, useState, type RefObject } from 'react';

import type { FileBarRowMenuEntry } from '../../../file-bar-contract';

type FileBarRowMenuChoice = Exclude<FileBarRowMenuEntry, { readonly kind: 'separator' }>;

interface FileBarRowMenuState {
	readonly entries: readonly FileBarRowMenuEntry[];
	readonly open: boolean;
}

const CLOSED: FileBarRowMenuState = { entries: [], open: false };

interface FileBarRowMenu {
	/** Runs a chosen entry once the keys are back where the menu found them. */
	readonly choose: (entry: FileBarRowMenuChoice) => void;
	/** The entries the menu took as it opened, kept while it closes. */
	readonly entries: readonly FileBarRowMenuEntry[];
	readonly finalFocus: () => HTMLElement | false;
	readonly onOpenChange: (open: boolean) => void;
	readonly open: boolean;
}

/** Whether an element can take the keys back: still in the document and not in a hidden view. */
function canTakeFocus(element: HTMLElement | null): element is HTMLElement {
	return element !== null && element.isConnected && element.closest('[hidden]') === null;
}

/**
 * The toggles row's menu, which a right-click, a long press or the context menu key opens anywhere
 * on the row, as VS Code's activity bar menu. VS Code's menu gives the keys back to where they were
 * before its action runs, so a chosen entry runs from there: hiding the File Bar from the menu then
 * hands the keys to the terminal as the File Bar leaves. Keys whose place went away with the entry,
 * as a toggle it hides, go to the selected toggle.
 */
export function useFileBarRowMenu(
	rowReference: RefObject<HTMLElement | null>,
	rowMenu: () => readonly FileBarRowMenuEntry[],
): FileBarRowMenu {
	const [menu, setMenu] = useState<FileBarRowMenuState>(CLOSED);
	const focusBeforeMenu = useRef<HTMLElement | null>(null);
	const onOpenChange = useMemoizedFn((open: boolean) => {
		if (!open) {
			setMenu((current) => ({ ...current, open: false }));
			return;
		}
		const row = rowReference.current;
		const activeElement = row?.ownerDocument.activeElement ?? null;
		focusBeforeMenu.current =
			activeElement instanceof HTMLElement && activeElement !== row?.ownerDocument.body
				? activeElement
				: null;
		setMenu({ entries: rowMenu(), open: true });
	});
	const choose = useMemoizedFn((entry: FileBarRowMenuChoice) => {
		const before = focusBeforeMenu.current;
		if (canTakeFocus(before)) before.focus({ preventScroll: true });
		entry.run();
	});
	const finalFocus = useMemoizedFn((): HTMLElement | false => {
		const row = rowReference.current;
		if (row === null) return false;
		const { activeElement, body } = row.ownerDocument;
		// A chosen entry has placed the keys already; Base UI would take them back.
		const placed =
			activeElement !== null &&
			activeElement !== body &&
			activeElement.closest('[role="menu"]') === null;
		if (placed) return false;
		const before = focusBeforeMenu.current;
		if (canTakeFocus(before)) return before;
		return row.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]') ?? false;
	});
	return { choose, entries: menu.entries, finalFocus, onOpenChange, open: menu.open };
}
