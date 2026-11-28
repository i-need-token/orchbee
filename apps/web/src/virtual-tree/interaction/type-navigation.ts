import type { TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeRow } from '../model/virtual-tree-types';

/** How long VS Code's type navigation waits for the next character before a word starts over. */
const TYPE_NAVIGATION_DELAY_MILLISECONDS = 800;

/** A row's text as type navigation matches it, such as a file's name. */
export type VirtualTreeKeyboardNavigationLabel<K, T> = (row: VirtualTreeRow<K, T>) => string;

/** Whether a key press types a character: one printable character, without Ctrl, Cmd or Alt. */
export function typesCharacter(event: KeyboardEvent): boolean {
	return (
		!event.ctrlKey &&
		!event.metaKey &&
		!event.altKey &&
		[...event.key].length === 1 &&
		!/\s/u.test(event.key)
	);
}

/**
 * VS Code's type navigation: characters typed within 800 ms of each other form a word, and focus
 * moves to the next row whose label starts with it, ignoring case. A word's first character looks
 * from the row after the focused one, so typing it again cycles through the rows it starts; the
 * characters after it look from the focused row, so a longer word stays on a row that still matches.
 * The search wraps past the last row, and focus moves without changing the selection or the anchor.
 */
export class TreeTypeNavigation<K, T> {
	private word = '';
	private timer: ReturnType<typeof setTimeout> | undefined;

	type(
		viewport: TreeViewport<K, T>,
		label: VirtualTreeKeyboardNavigationLabel<K, T>,
		character: string,
	): void {
		const continuing = this.word !== '';
		this.word += character;
		clearTimeout(this.timer);
		this.timer = setTimeout(() => {
			this.word = '';
		}, TYPE_NAVIGATION_DELAY_MILLISECONDS);
		const { model } = viewport;
		const count = model.rowCount;
		if (count === 0) return;
		const start = Math.max(viewport.focusIndex, 0) + (continuing ? 0 : 1);
		const word = this.word.toLowerCase();
		for (let offset = 0; offset < count; offset += 1) {
			const row = model.rowAt((start + offset) % count);
			if (!label(row).slice(0, word.length).toLowerCase().startsWith(word)) continue;
			model.setFocus(row.key);
			model.reveal(row.key);
			return;
		}
	}

	dispose(): void {
		clearTimeout(this.timer);
		this.word = '';
	}
}
