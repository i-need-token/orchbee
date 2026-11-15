import { SCENE_RESTART_EVENT, type Scene } from './scene';

/** The attribute the scene root carries while its terminals fade out before a restart. */
const FADING_ATTRIBUTE = 'data-scene-fading';

interface Cue<State> {
	readonly at: number;
	readonly run: (state: State) => State;
}

/**
 * Replays `scene` in a loop from its initial state, rendering each state it reaches. It plays only
 * while one of the `watch` elements is on screen and the page is visible, and continues where it
 * paused. Until then the page keeps showing what it showed, the final state on a static page.
 * Callers do not start it under reduced motion.
 */
export function playScene<State>(
	scene: Scene<State>,
	{
		root,
		watch,
		render,
	}: {
		/** Carries the fading attribute and receives the restart event. */
		root: HTMLElement;
		watch: readonly Element[];
		render: (state: State) => void;
	},
): void {
	const view = root.ownerDocument.defaultView;
	if (view === null || watch.length === 0) return;
	const { document } = view;

	let loop = 0;
	const restart = (): State => {
		root.toggleAttribute(FADING_ATTRIBUTE, false);
		if (loop > 0) root.dispatchEvent(new CustomEvent(SCENE_RESTART_EVENT, { detail: loop }));
		loop += 1;
		return scene.initial;
	};
	const cues: Cue<State>[] = [
		...scene.steps.map((step) => ({ at: step.at, run: step.apply })),
		{
			at: scene.fadeAt,
			run: (state: State) => {
				root.toggleAttribute(FADING_ATTRIBUTE, true);
				return state;
			},
		},
	];
	cues.sort((left, right) => left.at - right.at);

	let state = scene.initial;
	let next = 0;
	let startedAt = 0;
	let elapsed = 0;
	let timer = 0;
	let playing = false;
	let begun = false;
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
	const play = (): void => {
		if (playing) return;
		playing = true;
		if (!begun) {
			begun = true;
			state = restart();
		}
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

	const observer = new view.IntersectionObserver((entries) => {
		for (const entry of entries) {
			if (entry.isIntersecting) visible.add(entry.target);
			else visible.delete(entry.target);
		}
		update();
	});
	for (const element of watch) observer.observe(element);
	document.addEventListener('visibilitychange', update);
}
