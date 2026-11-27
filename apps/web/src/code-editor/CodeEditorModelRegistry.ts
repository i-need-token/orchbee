/** Content an editor shows, which its registry disposes of after the last tab lets go. */
interface CodeEditorModelEntry {
	dispose(): void;
}

/** One tab's hold on shared content, which lives while any hold does. */
interface CodeEditorModelHold<Entry extends CodeEditorModelEntry> {
	readonly entry: Entry;
	/** Ends this hold; releasing it again does nothing. */
	release(): void;
}

interface HeldEntry<Entry extends CodeEditorModelEntry> {
	readonly entry: Entry;
	holders: number;
}

/**
 * The content behind file tabs, one entry per resource key and counted by the tabs that show it:
 * a file's text tab and the modified side of its diff share one model, so a later edit in one
 * shows in the other. The first hold creates an entry and the last release disposes of it.
 */
export class CodeEditorModelRegistry<Entry extends CodeEditorModelEntry> {
	private readonly held = new Map<string, HeldEntry<Entry>>();

	public hold(key: string, create: () => Entry): CodeEditorModelHold<Entry> {
		let held = this.held.get(key);
		if (held === undefined) {
			held = { entry: create(), holders: 0 };
			this.held.set(key, held);
		}
		held.holders += 1;
		const current = held;
		let released = false;
		return {
			entry: current.entry,
			release: () => {
				if (released) return;
				released = true;
				current.holders -= 1;
				if (current.holders > 0) return;
				this.held.delete(key);
				current.entry.dispose();
			},
		};
	}
}
