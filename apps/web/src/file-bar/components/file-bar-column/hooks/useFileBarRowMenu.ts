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
	/** Keeps a chosen entry, which runs as the closed menu hands the keys back. */
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
 * Where the keys go back to: where the menu found them, else the selected toggle. A toggle the chosen
 * entry hides passes them to the selected toggle, or to the first toggle left when the hidden one was
 * selected.
 */
function returnTarget(
	row: HTMLElement,
	before: HTMLElement | null,
	hidden: string | null,
): HTMLElement | null {
	const leaves = (element: HTMLElement): boolean =>
		hidden !== null && element.getAttribute('role') === 'tab' && element.dataset['view'] === hidden;
	if (canTakeFocus(before) && !leaves(before)) return before;
	const toggles = [...row.querySelectorAll<HTMLElement>('[role="tab"]')].filter(
		(toggle) => !leaves(toggle),
	);
	return (
		toggles.find((toggle) => toggle.getAttribute('aria-selected') === 'true') ?? toggles[0] ?? null
	);
}

/**
 * The toggles row's menu, which a right-click, a long press or the context menu key opens anywhere
 * on the row, as VS Code's activity bar menu. VS Code hides its menu and gives the keys back before
 * the chosen action runs, so a chosen entry runs as the closed menu leaves, from where the keys were:
 * hiding the File Bar from the menu then hands the keys to the terminal as the File Bar leaves, and a
 * move takes them along with the column.
 */
export function useFileBarRowMenu(
	rowReference: RefObject<HTMLElement | null>,
	rowMenu: () => readonly FileBarRowMenuEntry[],
): FileBarRowMenu {
	const [menu, setMenu] = useState<FileBarRowMenuState>(CLOSED);
	const focusBeforeMenu = useRef<HTMLElement | null>(null);
	const chosen = useRef<FileBarRowMenuChoice | null>(null);
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
		chosen.current = null;
		setMenu({ entries: rowMenu(), open: true });
	});
	const choose = useMemoizedFn((entry: FileBarRowMenuChoice) => {
		chosen.current = entry;
	});
	// Base UI asks where the keys go as the closed menu leaves the document, after the focus it holds
	// while open could take them back from a chosen entry.
	const finalFocus = useMemoizedFn((): HTMLElement | false => {
		const entry = chosen.current;
		chosen.current = null;
		const row = rowReference.current;
		if (entry === null) {
			if (row === null) return false;
			const { activeElement, body } = row.ownerDocument;
			// A press outside has placed the keys already; Base UI would take them back.
			const placed =
				activeElement !== null &&
				activeElement !== body &&
				activeElement.closest('[role="menu"]') === null;
			return placed ? false : (returnTarget(row, focusBeforeMenu.current, null) ?? false);
		}
		const hidden = entry.kind === 'toggle' && entry.checked ? entry.id : null;
		const target = row === null ? null : returnTarget(row, focusBeforeMenu.current, hidden);
		target?.focus({ preventScroll: true });
		entry.run();
		return false;
	});
	return { choose, entries: menu.entries, finalFocus, onOpenChange, open: menu.open };
}
