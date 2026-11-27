/** How many files' view states the overlay keeps, as VS Code's text editor memento does. */
const VIEW_STATE_LIMIT = 100;

/**
 * Where each file's editor was scrolled and what it selected, so a file shows as it was left when
 * its tab returns, even after the tab closed. Past the limit, the state saved longest ago goes.
 */
export class CodeEditorViewStates<State> {
	private readonly states = new Map<string, State>();

	public get(key: string): State | undefined {
		return this.states.get(key);
	}

	public set(key: string, state: State): void {
		this.states.delete(key);
		this.states.set(key, state);
		if (this.states.size <= VIEW_STATE_LIMIT) return;
		for (const oldest of this.states.keys()) {
			this.states.delete(oldest);
			return;
		}
	}
}
