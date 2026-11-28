import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl } from '@orchbee/i18n';
import {
	formatKeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import { useState } from 'react';

import type { CodeEditorTextHandle } from '../../../../../../../../../../../code-editor';
import type {
	FileOverlayEditorHost,
	FileOverlayMenuCommand,
	FileOverlayMenuEntry,
} from '../../../../../../../../../file-overlay-contract';
import {
	FILE_TAB_EDITOR_MENU_LABELS,
	fileTabEditorMenuGroups,
	type FileTabEditorMenuCommand,
} from '../utils/file-tab-editor-menu';

interface FileTabEditorMenuState {
	readonly entries: readonly FileOverlayMenuEntry[];
	readonly open: boolean;
}

const CLOSED: FileTabEditorMenuState = { entries: [], open: false };

/**
 * The keys VS Code's menu shows beside the editor's items, with the platform's command modifier:
 * the editor binds Change All Occurrences, and the browser the clipboard's.
 */
const KEY_CODES: Readonly<
	Record<Exclude<FileTabEditorMenuCommand, 'workbench.action.showCommands'>, string>
> = {
	'editor.action.changeAll': 'F2',
	'editor.action.clipboardCopyAction': 'KeyC',
	'editor.action.clipboardCutAction': 'KeyX',
	'editor.action.clipboardPasteAction': 'KeyV',
};

interface FileTabEditorMenu {
	/** Runs a chosen item once the editor has the keys. */
	readonly choose: (entry: FileOverlayMenuCommand) => void;
	/** The items as the menu opened, kept while it closes. */
	readonly entries: readonly FileOverlayMenuEntry[];
	readonly finalFocus: () => false;
	readonly onOpenChange: (open: boolean, details: { readonly event: Event }) => void;
	readonly open: boolean;
}

/**
 * The text editor's menu, in place of Monaco's, as VS Code's editor context menu shows the items
 * Orchbee offers. It opens only where the editor lets a right click or the menu key through, and
 * takes its items as it opens; a chosen item runs with the keys in the editor, as a dismissed menu
 * returns them there.
 */
export function useFileTabEditorMenu(
	handle: CodeEditorTextHandle | null,
	host: FileOverlayEditorHost,
	platform: KeyboardShortcutPlatform,
): FileTabEditorMenu {
	const intl = useOrchbeeIntl();
	const [menu, setMenu] = useState<FileTabEditorMenuState>(CLOSED);
	const item = (
		editor: CodeEditorTextHandle,
		command: FileTabEditorMenuCommand,
	): FileOverlayMenuCommand => {
		const label = intl.formatMessage(FILE_TAB_EDITOR_MENU_LABELS[command]);
		if (command === 'workbench.action.showCommands') {
			return {
				disabled: false,
				id: command,
				kind: 'command',
				label,
				run: host.searchActions,
				shortcut: host.searchActionsShortcut,
			};
		}
		return {
			disabled: false,
			id: command,
			kind: 'command',
			label,
			run: () => editor.run(command),
			shortcut: formatKeyboardShortcutChord(
				{ code: KEY_CODES[command], modifiers: ['mod'] },
				platform,
			),
		};
	};
	const onOpenChange = useMemoizedFn((open: boolean, details: { readonly event: Event }) => {
		if (!open) {
			setMenu((current) => ({ ...current, open: false }));
			return;
		}
		const { target } = details.event;
		if (handle === null || !(target instanceof Node) || !handle.contains(target)) return;
		const groups = fileTabEditorMenuGroups({
			changeAll: handle.supports('editor.action.changeAll'),
			writable: handle.writable(),
		});
		const entries = groups.flatMap((group, index): FileOverlayMenuEntry[] => [
			...(index === 0 ? [] : [{ id: `separator-${index}`, kind: 'separator' as const }]),
			...group.map((command) => item(handle, command)),
		]);
		setMenu({ entries, open: true });
	});
	const choose = useMemoizedFn((entry: FileOverlayMenuCommand) => {
		handle?.focus();
		entry.run();
	});
	const finalFocus = useMemoizedFn((): false => {
		const { activeElement, body } = document;
		// A chosen item or a press outside has placed the keys already; Base UI would take them back.
		const placed =
			activeElement !== null &&
			activeElement !== body &&
			activeElement.closest('[role="menu"]') === null;
		if (!placed) handle?.focus();
		return false;
	});
	return { choose, entries: menu.entries, finalFocus, onOpenChange, open: menu.open };
}
