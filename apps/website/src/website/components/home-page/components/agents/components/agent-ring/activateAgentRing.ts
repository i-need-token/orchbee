import type { ActivationContext } from '../../../../../../activation';
import type { AgentState } from '../../../../../state-glyph/StateGlyph';
import { RING_CELLS, RING_LINES, RING_STEP_MS, type RingCell } from '../../agent-ring';

interface LiveCell {
	readonly cell: RingCell;
	readonly glyph: SVGElement;
	readonly word: HTMLElement;
	readonly lines: HTMLElement;
	state: AgentState;
	printed: number;
}

// Lets the ring's working Agents print on: every step one of them, picked at random, prints its
// next line, and an Agent holds another state for a while where its script pauses. It steps while
// the ring is on screen and the page is visible, never under reduced motion. Lines are clones of
// the ring's line template; state words come from the page.
export function activateAgentRing({ document, reducedMotion }: ActivationContext): void {
	const root = document.querySelector<HTMLElement>('[data-agent-ring]');
	if (root === null || reducedMotion) return;
	const lineTemplate = root.querySelector<HTMLTemplateElement>('template[data-ring-line]');
	const line = lineTemplate?.content.firstElementChild;
	const words = root.querySelector<HTMLTemplateElement>('template[data-ring-words]')?.content;
	if (line === null || line === undefined || words === undefined) return;

	const cells: LiveCell[] = [];
	for (const cell of RING_CELLS) {
		const element = root.querySelector<HTMLElement>(`[data-ring-cell="${cell.id}"]`);
		const glyph = element?.querySelector<SVGElement>('[data-state]');
		const word = element?.querySelector<HTMLElement>('[data-ring-state]');
		const lines = element?.querySelector<HTMLElement>('[data-ring-lines]');
		if (glyph === null || glyph === undefined) return;
		if (word === null || word === undefined || lines === null || lines === undefined) return;
		cells.push({ cell, glyph, word, lines, state: cell.state, printed: cell.shown });
	}

	const show = (live: LiveCell, state: AgentState): void => {
		live.state = state;
		live.glyph.setAttribute('data-state', state);
		live.word.textContent = words.querySelector(`[data-key="${state}"]`)?.textContent ?? '';
	};
	const step = (): void => {
		const working = cells.filter((live) => live.state === 'working');
		const live = working[Math.floor(Math.random() * working.length)];
		if (live === undefined) return;
		const { lines, pause } = live.cell;
		const index = live.printed % lines.length;
		live.printed += 1;
		const row = line.cloneNode(true);
		row.textContent = lines[index] ?? '';
		live.lines.append(row);
		while (live.lines.children.length > RING_LINES) live.lines.firstElementChild?.remove();
		if (pause === undefined || pause.after !== index) return;
		show(live, pause.state);
		window.setTimeout(() => show(live, 'working'), pause.ms);
	};

	let timer = 0;
	let onScreen = false;
	const update = (): void => {
		const run = onScreen && document.visibilityState === 'visible';
		if (run && timer === 0) timer = window.setInterval(step, RING_STEP_MS);
		if (!run && timer !== 0) {
			window.clearInterval(timer);
			timer = 0;
		}
	};
	new IntersectionObserver(([entry]) => {
		onScreen = entry?.isIntersecting ?? false;
		update();
	}).observe(root);
	document.addEventListener('visibilitychange', update);
}
