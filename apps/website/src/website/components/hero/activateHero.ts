import type { ClientStrings } from '../../readClientStrings';

const AUTO_CLOSE_FROM_S = 6.2;
const AUTO_CLOSE_UNTIL_S = 13.4;
const TICK_MS = 250;

// The hero story. Once the window comes into view it closes itself for a while, unless the
// visitor prefers reduced motion, to show that the Agents keep working; the visitor can close and
// reopen it at any time, and replay the story when it ends.
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
	const replay = hook('replay');
	const clock = hook('closed-for');
	const liveText = hook('live-text');
	const close = document.querySelector<HTMLElement>('[data-action="close-window"]');
	if (
		stage === null ||
		demo === null ||
		tryHint === null ||
		replay === null ||
		clock === null ||
		liveText === null ||
		close === null
	)
		return;

	let storyStart: number | undefined;
	let userClosed: boolean | undefined;
	let closedAt = 0;
	let tried = false;
	let lastSignature = '';
	tryHint.hidden = false;

	const render = (now: number): void => {
		const seconds = storyStart === undefined ? 0 : (now - storyStart) / 1000;
		const auto =
			storyStart !== undefined &&
			!reducedMotion &&
			seconds >= AUTO_CLOSE_FROM_S &&
			seconds < AUTO_CLOSE_UNTIL_S;
		const closed = userClosed ?? auto;
		const done = storyStart !== undefined && seconds >= AUTO_CLOSE_UNTIL_S && !closed;
		const closedSince =
			userClosed === undefined && storyStart !== undefined
				? storyStart + AUTO_CLOSE_FROM_S * 1000
				: closedAt || now;
		const since = closed ? Math.max(0, Math.floor((now - closedSince) / 1000)) : 0;
		const signature = [closed, done, tried, storyStart !== undefined, since].join('|');
		if (signature === lastSignature) return;
		lastSignature = signature;
		demo.toggleAttribute('data-closed', closed);
		tryHint.toggleAttribute('data-off', closed || tried);
		close.toggleAttribute('data-hint', storyStart !== undefined && !closed && !tried);
		liveText.textContent = closed ? strings.live.closed : strings.live.working;
		replay.hidden = !done;
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
