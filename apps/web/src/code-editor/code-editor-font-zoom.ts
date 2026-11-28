/** Monaco's bounds for its editor font zoom level, which counts steps of a tenth of the font size. */
const LOWEST_LEVEL = -5;
const HIGHEST_LEVEL = 20;

/**
 * The editor font zoom, which Monaco keeps page-wide for every editor and VS Code's Increase,
 * Decrease and Reset Editor Font Size change. It lives apart from Monaco, so the Workbench's Actions
 * change it whether or not the editor has loaded: the editor takes the level as it loads and the two
 * follow each other from then on. As in VS Code, the level lasts until the page reloads.
 */
export class CodeEditorFontZoom {
	#level = 0;
	readonly #listeners = new Set<() => void>();

	public readonly getLevel = (): number => this.#level;

	public readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	/** Moves to `level`, kept within Monaco's bounds. */
	public setLevel(level: number): void {
		const next = Math.min(Math.max(LOWEST_LEVEL, level), HIGHEST_LEVEL);
		if (next === this.#level) return;
		this.#level = next;
		for (const listener of this.#listeners) listener();
	}
}

/** The page's editor font zoom. */
export const codeEditorFontZoom = new CodeEditorFontZoom();
