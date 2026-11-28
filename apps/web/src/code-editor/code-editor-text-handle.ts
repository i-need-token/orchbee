/** One of the editor's own actions, by Monaco's identity, as a command palette lists it. */
export interface CodeEditorActionEntry {
	/** The action's English name, which a search in any language matches as well. */
	readonly alias: string;
	/** Monaco's identity for the action, which is VS Code's command identity. */
	readonly id: string;
	/** The action's name in the editor's language. */
	readonly label: string;
}

/**
 * What a text view's owner may do with its editor while the editor exists: list and run the
 * editor's own actions, and give it the keys. It holds no Monaco object, so it crosses into code
 * that never loads Monaco.
 */
export interface CodeEditorTextHandle {
	/** Whether `node` lies in the editor, its widgets included. */
	contains(node: Node): boolean;
	/** Gives the editor the keys. */
	focus(): void;
	/**
	 * Runs the editor's command `id`, as its key would: one of its actions, or a command such as
	 * `editor.action.clipboardCopyAction` that acts on the editor with the keys. Nothing happens where
	 * the editor does not offer it now.
	 */
	run(id: string): void;
	/**
	 * The actions the editor offers now, which a command palette lists, as Monaco's
	 * `getSupportedActions` reads them. Some offer themselves only while the editor has the keys, or
	 * only in some states, so the list holds as it is read.
	 */
	supportedActions(): readonly CodeEditorActionEntry[];
	/**
	 * Whether the editor offers the action `id` now. Some offer themselves only while the editor has
	 * the keys, so the answer holds as it is read.
	 */
	supports(id: string): boolean;
	/** Whether the editor takes edits, which is when VS Code shows Cut and Paste. */
	writable(): boolean;
}
