import type { ClientStrings } from '../../readClientStrings';
import { renderTuiLines } from '../tui-screen/renderTuiLines';
import { compactHeroFrame, heroFrame, type HeroPhase } from './hero-frames';

const PHASE_STARTS = [2.6, 5.2, 9.4] as const;
const AUTO_CLOSE_FROM = 6.2;
const AUTO_CLOSE_UNTIL = 13.4;
const TICK_MS = 250;

function phaseAt(seconds: number): HeroPhase {
	if (seconds >= PHASE_STARTS[2]) return 3;
	if (seconds >= PHASE_STARTS[1]) return 2;
	return seconds >= PHASE_STARTS[0] ? 1 : 0;
}

interface Story {
	readonly phase: HeroPhase;
	readonly closed: boolean;
	readonly done: boolean;
	readonly noted: boolean;
	readonly closedAt: number;
}

// The hero story. The session keeps its own clock from the moment the demo comes into view; the
// window only decides whether the visitor can see it. It closes itself for a while unless the
// visitor prefers reduced motion, and the visitor can close and reopen it at any time.
export function activateHero(
	document: Document,
	strings: ClientStrings,
	reducedMotion: boolean,
): void {
	const hook = (name: string): HTMLElement | null =>
		document.querySelector<HTMLElement>(`[data-hook="${name}"]`);
	const stage = hook('stage');
	const demo = hook('demo');
	const tryHint = hook('try');
	const note = hook('note');
	const replay = hook('replay');
	const clock = hook('closed-for');
	const liveDot = hook('live-dot');
	const liveText = hook('live-text');
	const close = document.querySelector<HTMLElement>('[data-action="close-window"]');
	const wide = document.querySelector('[data-tui="hero"]');
	const narrow = document.querySelector('[data-tui="hero-compact"]');
	if (
		stage === null ||
		demo === null ||
		tryHint === null ||
		note === null ||
		replay === null ||
		clock === null ||
		liveDot === null ||
		liveText === null ||
		close === null ||
		wide === null ||
		narrow === null
	)
		return;

	let storyStart: number | undefined;
	let userClosed: boolean | undefined;
	let closedAt = 0;
	let tried = false;
	let shownPhase: HeroPhase = 0;
	let lastSignature = '';
	tryHint.hidden = false;

	const story = (now: number): Story => {
		if (storyStart === undefined)
			return {
				phase: 0,
				closed: userClosed === true,
				done: false,
				noted: false,
				closedAt: closedAt || now,
			};
		const seconds = (now - storyStart) / 1000;
		const phase = phaseAt(seconds);
		const auto = !reducedMotion && seconds >= AUTO_CLOSE_FROM && seconds < AUTO_CLOSE_UNTIL;
		const closed = userClosed ?? auto;
		return {
			phase,
			closed,
			done: seconds >= AUTO_CLOSE_UNTIL && !closed,
			noted: closed && phase === 3,
			closedAt: userClosed === undefined ? storyStart + AUTO_CLOSE_FROM * 1000 : closedAt || now,
		};
	};

	const render = (now: number): void => {
		const current = story(now);
		const since = current.closed ? Math.max(0, Math.floor((now - current.closedAt) / 1000)) : 0;
		const signature = [
			current.phase,
			current.closed,
			current.done,
			tried,
			storyStart !== undefined,
			since,
		].join('|');
		if (signature === lastSignature) return;
		lastSignature = signature;
		if (current.phase !== shownPhase) {
			shownPhase = current.phase;
			renderTuiLines(wide, heroFrame(current.phase));
			renderTuiLines(narrow, compactHeroFrame(current.phase));
		}
		demo.toggleAttribute('data-closed', current.closed);
		tryHint.toggleAttribute('data-off', current.closed || tried);
		close.toggleAttribute('data-hint', storyStart !== undefined && !current.closed && !tried);
		const waiting = current.phase === 3;
		liveDot.dataset['state'] = waiting ? 'attention' : 'working';
		let live = current.closed ? strings.live.closed : strings.live.working;
		if (waiting) live = strings.live.attention;
		liveText.textContent = live;
		const states = [
			waiting ? 'attention' : 'working',
			current.phase < 2 ? 'working' : 'done',
			'working',
		];
		const activities = [
			strings.claude[current.phase],
			strings.reviewer[current.phase],
			strings.server,
		];
		states.forEach((state, index) => {
			const dot = hook(`hive-dot-${index}`);
			const activity = hook(`hive-activity-${index}`);
			if (dot !== null) dot.dataset['state'] = state;
			if (activity !== null) activity.textContent = activities[index] ?? '';
		});
		note.hidden = !current.noted;
		replay.hidden = !current.done;
		clock.textContent = strings.closedFor.replace(
			'{clock}',
			`${Math.floor(since / 60)}:${String(since % 60).padStart(2, '0')}`,
		);
	};

	const watchStage = (): void => {
		if (storyStart !== undefined) return;
		const box = stage.getBoundingClientRect();
		const height = window.innerHeight;
		const seen = Math.min(box.bottom, height) - Math.max(box.top, 0);
		if (seen >= Math.min(box.height * 0.6, height * 0.6)) {
			storyStart = Date.now();
			render(storyStart);
		}
	};

	close.addEventListener('click', () => {
		const now = Date.now();
		userClosed = true;
		closedAt = now;
		tried = true;
		storyStart ??= now;
		render(now);
	});
	for (const opener of demo.querySelectorAll('[data-action="open-window"]')) {
		opener.addEventListener('click', () => {
			userClosed = false;
			render(Date.now());
		});
	}
	replay.addEventListener('click', () => {
		const now = Date.now();
		storyStart = now;
		userClosed = undefined;
		render(now);
	});
	window.addEventListener('scroll', watchStage, { passive: true });
	window.addEventListener('resize', watchStage);
	watchStage();
	render(Date.now());
	window.setInterval(() => render(Date.now()), TICK_MS);
}
