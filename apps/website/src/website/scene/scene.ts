import type { AgentState } from '../components/state-glyph/StateGlyph';
import type { TerminalRow } from '../components/terminal-text/terminal-row';

// A scripted scene: a state, and the steps that change it over time. The static page renders the
// final state; the player replays the steps from the initial state. A scene's module turns a
// state into a frame, the values the page binds by data attributes.

/** Dispatched on the scene root each time the scene starts again; `detail` counts the restarts. */
export const SCENE_RESTART_EVENT = 'scenerestart';

export interface SceneStep<State> {
	/** Milliseconds after the scene starts. */
	readonly at: number;
	readonly apply: (state: State) => State;
}

export interface Scene<State> {
	readonly initial: State;
	/** In time order; steps with the same time apply in list order. */
	readonly steps: readonly SceneStep<State>[];
	/** When the terminals fade out before the scene starts again. */
	readonly fadeAt: number;
	/** When the scene starts again. */
	readonly length: number;
	/**
	 * The state each later run starts from, given the state the last run ended in. Without it every
	 * run starts from `initial`; a scene that carries on where it left off returns its argument.
	 */
	readonly carryOver?: (state: State) => State;
}

/**
 * What a state shows, by binding key. An element names its key in an attribute:
 * `data-scene-rows` (terminal rows), `data-scene-state` (a state glyph's `data-state`),
 * `data-scene-text` (text), `data-scene-show` (shown when true, else `hidden`) and
 * `data-scene-open` (carries `data-open` when true).
 */
export interface SceneFrame {
	readonly rows: Readonly<Record<string, readonly TerminalRow[]>>;
	readonly states: Readonly<Record<string, AgentState>>;
	readonly texts: Readonly<Record<string, string>>;
	readonly flags: Readonly<Record<string, boolean>>;
}

/** The state after every step: what the static page and reduced motion show. */
export function sceneFinalState<State>(scene: Scene<State>): State {
	return scene.steps.reduce((state, step) => step.apply(state), scene.initial);
}

/** Orders steps by time, keeping the written order of steps at the same time. */
export function timeOrdered<State>(steps: readonly SceneStep<State>[]): SceneStep<State>[] {
	const ordered = [...steps];
	ordered.sort((left, right) => left.at - right.at);
	return ordered;
}
