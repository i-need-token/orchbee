import { SCENE_RESTART_EVENT, type Scene } from './scene';

/** The attribute the scene root carries while its terminals fade out before a restart. */
const FADING_ATTRIBUTE = 'data-scene-fading';

interface Cue<State> {
	readonly at: number;
	readonly run: (state: State) => State;
}

/** What an activation can do with a scene that plays. */
export interface ScenePlayer<State> {
	/** Stops the scene for good and returns the state it showed last. */
	stop: () => State;
	/**
	 * Moves the scene to `at` milliseconds into its run, showing the state the earlier steps make,
	 * and plays on from there while it is on screen.
	 */
	seek: (at: number) => void;
}

/**
 * Replays `scene` in a loop from its initial state, rendering each state it reaches. It plays only
 * while one of the `watch` elements is on screen (`threshold`: the share of it that must show) and
 * the page is visible, and continues where it paused, or where `resumeAt` moves the paused time.
 * Until then the page keeps showing what it showed, the final state on a static page. Callers do
 * not start it under reduced motion.
 */
export function playScene<State>(
	scene: Scene<State>,
	{
		root,
		watch,
		render,
		threshold = 0,
		resumeAt,
	}: {
		/** Carries the fading attribute and receives the restart event. */
		root: HTMLElement;
		watch: readonly Element[];
		render: (state: State) => void;
		threshold?: number;
		resumeAt?: (paused: number) => number;
	},
): ScenePlayer<State> {
	const view = root.ownerDocument.defaultView;
	let state = scene.initial;
	if (view === null || watch.length === 0) return { stop: () => state, seek: () => undefined };
	const { document } = view;

	let loop = 0;
	let runStart = scene.initial;
	const restart = (): State => {
		root.toggleAttribute(FADING_ATTRIBUTE, false);
		if (loop > 0) root.dispatchEvent(new CustomEvent(SCENE_RESTART_EVENT, { detail: loop }));
		runStart = loop > 0 && scene.carryOver !== undefined ? scene.carryOver(state) : scene.initial;
		loop += 1;
		return runStart;
	};
	const cues: Cue<State>[] = [
		...scene.steps.map((step) => ({ at: step.at, run: step.apply })),
		{
			at: scene.fadeAt,
			run: (current: State) => {
				root.toggleAttribute(FADING_ATTRIBUTE, true);
				return current;
			},
		},
	];
	cues.sort((left, right) => left.at - right.at);

	let next = 0;
	let startedAt = 0;
	let elapsed = 0;
	let timer = 0;
	let playing = false;
	let begun = false;
	let stopped = false;
	const visible = new Set<Element>();

	const tick = (): void => {
		let now = view.performance.now() - startedAt;
		if (now >= scene.length) {
			state = restart();
			next = 0;
			startedAt += scene.length;
			now -= scene.length;
		}
		for (let cue = cues[next]; cue !== undefined && cue.at <= now; cue = cues[next]) {
			state = cue.run(state);
			next += 1;
		}
		render(state);
		const due = cues[next]?.at ?? scene.length;
		timer = view.setTimeout(tick, Math.max(8, due - now));
	};
	// The state `at` milliseconds into the current run, and the first cue still to come.
	const rewind = (at: number): void => {
		root.toggleAttribute(FADING_ATTRIBUTE, false);
		state = runStart;
		next = 0;
		for (let cue = cues[next]; cue !== undefined && cue.at < at; cue = cues[next]) {
			state = cue.run(state);
			next += 1;
		}
		elapsed = at;
	};
	const begin = (): void => {
		if (begun) return;
		begun = true;
		state = restart();
	};
	const play = (): void => {
		if (playing || stopped) return;
		playing = true;
		if (begun && resumeAt !== undefined) rewind(resumeAt(elapsed));
		begin();
		startedAt = view.performance.now() - elapsed;
		tick();
	};
	const pause = (): void => {
		if (!playing) return;
		playing = false;
		view.clearTimeout(timer);
		elapsed = view.performance.now() - startedAt;
	};
	const update = (): void => {
		if (visible.size > 0 && document.visibilityState === 'visible') play();
		else pause();
	};

	const observer = new view.IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				if (entry.isIntersecting) visible.add(entry.target);
				else visible.delete(entry.target);
			}
			update();
		},
		{ threshold },
	);
	for (const element of watch) observer.observe(element);
	document.addEventListener('visibilitychange', update);

	return {
		stop: () => {
			pause();
			stopped = true;
			observer.disconnect();
			document.removeEventListener('visibilitychange', update);
			return state;
		},
		seek: (at) => {
			if (stopped) return;
			begin();
			view.clearTimeout(timer);
			rewind(at);
			if (playing) {
				startedAt = view.performance.now() - at;
				tick();
			} else render(state);
		},
	};
}
