import { describe, expect, it } from 'vitest';

import { FILE_TAB_EDITOR_MENU_LABELS, fileTabEditorMenuGroups } from '../file-tab-editor-menu';
import {
	VSCODE_EDITOR_CONTEXT,
	type VSCodeEditorCondition,
	type VSCodeEditorMenuItem,
} from './vscode-editor-context-menu';

/** What VS Code's menu knows of the editor it opens for. */
interface EditorFacts {
	readonly writable: boolean;
}

/** One item as the menu shows it: its command and English label. */
interface ShownItem {
	readonly command: string;
	readonly label: string;
}

/**
 * VS Code's menu order: the `navigation` group first, an ungrouped item last and the other groups by
 * locale order, then by order, then by title.
 */
function compareVSCodeMenuItems(first: VSCodeEditorMenuItem, second: VSCodeEditorMenuItem): number {
	if (first.group !== second.group) {
		if (first.group === '') return 1;
		if (second.group === '') return -1;
		if (first.group === 'navigation') return -1;
		if (second.group === 'navigation') return 1;
		const groups = first.group.localeCompare(second.group);
		if (groups !== 0) return groups;
	}
	if (first.order !== second.order) return first.order - second.order;
	return first.title.localeCompare(second.title);
}

/**
 * Evaluates a condition with the context keys VS Code sets as the menu opens: the right click or menu
 * key gives the text focus first, and the overlay's editor is a full editor.
 */
function holds(condition: VSCodeEditorCondition | undefined, facts: EditorFacts): boolean {
	if (condition === undefined) return true;
	if ('and' in condition) return condition.and.every((part) => holds(part, facts));
	if ('not' in condition) return !holds(condition.not, facts);
	switch (condition.key) {
		case 'editorReadonly':
			return !facts.writable;
		case 'editorSimpleInput':
			return false;
		case 'editorTextFocus':
			return true;
	}
}

/** The items Orchbee offers that VS Code shows for the editor, group by group. */
function expectedGroups(facts: EditorFacts): ShownItem[][] {
	const groups = new Map<string, ShownItem[]>();
	for (const item of VSCODE_EDITOR_CONTEXT) {
		if (item.orchbee.kind !== 'offered' || !holds(item.when, facts)) continue;
		const label = 'label' in item.orchbee ? item.orchbee.label : item.title;
		groups.set(item.group, [...(groups.get(item.group) ?? []), { command: item.command, label }]);
	}
	return [...groups.values()];
}

describe('the text editor’s menu', () => {
	it('keeps the transcription in VS Code’s menu order', () => {
		expect([...VSCODE_EDITOR_CONTEXT].sort(compareVSCodeMenuItems)).toEqual(VSCODE_EDITOR_CONTEXT);
	});

	it('shows VS Code’s editor context menu for an editor that takes edits and one that does not', () => {
		for (const writable of [true, false]) {
			const facts = { writable };
			// The editor offers Change All Occurrences as the menu item's own condition reads.
			const changeAll = holds(
				VSCODE_EDITOR_CONTEXT.find(({ command }) => command === 'editor.action.changeAll')?.when,
				facts,
			);
			const shown = fileTabEditorMenuGroups({ changeAll, writable }).map((group) =>
				group.map((command) => ({
					command,
					label: FILE_TAB_EDITOR_MENU_LABELS[command].defaultMessage,
				})),
			);
			expect({ shown, writable }).toEqual({ shown: expectedGroups(facts), writable });
		}
	});
});
