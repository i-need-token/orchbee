import { codeEditorMessages, type MessageDescriptor } from '@orchbee/i18n';

/** A command the text editor's menu shows, by VS Code's identity for it. */
export type FileTabEditorMenuCommand =
	| 'editor.action.changeAll'
	| 'editor.action.clipboardCopyAction'
	| 'editor.action.clipboardCutAction'
	| 'editor.action.clipboardPasteAction'
	| 'workbench.action.showCommands';

/** What decides the menu's items as it opens. */
export interface FileTabEditorMenuFacts {
	/** Whether the editor offers Change All Occurrences, which needs it to take edits and the keys. */
	readonly changeAll: boolean;
	/** Whether the editor takes edits. */
	readonly writable: boolean;
}

/**
 * Each item's label: VS Code's own, except at Command Palette's place, which names Action search,
 * the palette's counterpart.
 */
export const FILE_TAB_EDITOR_MENU_LABELS: Readonly<
	Record<FileTabEditorMenuCommand, MessageDescriptor>
> = {
	'editor.action.changeAll': codeEditorMessages.changeAll,
	'editor.action.clipboardCopyAction': codeEditorMessages.copy,
	'editor.action.clipboardCutAction': codeEditorMessages.cut,
	'editor.action.clipboardPasteAction': codeEditorMessages.paste,
	'workbench.action.showCommands': codeEditorMessages.searchActions,
};

/**
 * The text editor's menu as VS Code's editor context menu shows the items Orchbee offers: by VS
 * Code's groups in its order, each item where VS Code shows it, Cut and Paste only where the editor
 * takes edits. Command Palette's place opens Action search on Actions.
 */
export function fileTabEditorMenuGroups({
	changeAll,
	writable,
}: FileTabEditorMenuFacts): readonly (readonly FileTabEditorMenuCommand[])[] {
	const groups: readonly (readonly FileTabEditorMenuCommand[])[] = [
		changeAll ? ['editor.action.changeAll'] : [],
		writable
			? [
					'editor.action.clipboardCutAction',
					'editor.action.clipboardCopyAction',
					'editor.action.clipboardPasteAction',
				]
			: ['editor.action.clipboardCopyAction'],
		['workbench.action.showCommands'],
	];
	return groups.filter((group) => group.length > 0);
}
