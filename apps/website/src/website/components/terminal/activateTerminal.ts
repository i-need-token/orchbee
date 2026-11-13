import { encodeKittyKey, type KittyKey } from './encodeKittyKey';
import styles from './Terminal.module.css';

const DEFAULT_SPLIT = 46;
const MIN_SPLIT = 25;
const MAX_SPLIT = 75;
const SPLIT_STEP = 5;
const TOTAL_COLUMNS = 120;
const SHOWN_KEYS = 4;

// The terminal section: the split follows a drag or the arrow keys and reports both panes' sizes,
// and the key pane lists the Kitty keyboard sequence of each key pressed in it.
export function activateTerminal(document: Document, mac: boolean): void {
	const split = document.querySelector<HTMLElement>('[data-split]');
	const left = document.querySelector<HTMLElement>('[data-split-left]');
	const gutter = document.querySelector<HTMLElement>('[data-split-gutter]');
	const size = document.querySelector<HTMLElement>('[data-split-size]');
	if (split !== null && left !== null && gutter !== null && size !== null) {
		const apply = (value: number): void => {
			const next = Math.round(Math.min(MAX_SPLIT, Math.max(MIN_SPLIT, value)));
			const columns = Math.round((TOTAL_COLUMNS / 100) * next);
			left.style.width = `${next}%`;
			gutter.setAttribute('aria-valuenow', String(next));
			gutter.dataset['value'] = String(next);
			size.textContent = `${columns}×32 · ${TOTAL_COLUMNS - columns}×32`;
		};
		const current = (): number => Number(gutter.dataset['value'] ?? DEFAULT_SPLIT);
		apply(DEFAULT_SPLIT);
		gutter.addEventListener('keydown', (event) => {
			const to: number | undefined = {
				ArrowLeft: current() - SPLIT_STEP,
				ArrowRight: current() + SPLIT_STEP,
				Home: MIN_SPLIT,
				End: MAX_SPLIT,
			}[event.key];
			if (to === undefined) return;
			event.preventDefault();
			apply(to);
		});
		gutter.addEventListener('pointerdown', (event) => {
			event.preventDefault();
			const move = (moved: PointerEvent): void => {
				const box = split.getBoundingClientRect();
				if (box.width > 0) apply(((moved.clientX - box.left) / box.width) * 100);
			};
			const end = (): void => {
				window.removeEventListener('pointermove', move);
				window.removeEventListener('pointerup', end);
				window.removeEventListener('pointercancel', end);
				delete gutter.dataset['dragging'];
			};
			gutter.dataset['dragging'] = '';
			window.addEventListener('pointermove', move);
			window.addEventListener('pointerup', end);
			window.addEventListener('pointercancel', end);
		});
	}

	const keys = document.querySelector<HTMLElement>('[data-keys]');
	const rows = keys?.querySelector<HTMLElement>('[data-key-rows]');
	if (keys === null || rows === null || rows === undefined) return;
	if (mac) {
		// The second demo key is Alt + ←, which a Mac keyboard labels Option.
		const alt = rows.children[1]?.querySelector('b');
		if (alt !== null && alt !== undefined) alt.textContent = 'Option + ←';
	}
	let pressed: KittyKey[] = [];
	keys.addEventListener('keydown', (event) => {
		const key = encodeKittyKey(event, mac);
		if (key === undefined) return;
		event.preventDefault();
		pressed = [key, ...pressed].slice(0, SHOWN_KEYS);
		rows.replaceChildren(
			...pressed.map((entry) => {
				const row = document.createElement('div');
				row.className = styles['key'] ?? '';
				const name = document.createElement('b');
				name.textContent = entry.name;
				const sequence = document.createElement('code');
				sequence.textContent = entry.sequence;
				row.append(name, sequence);
				return row;
			}),
		);
	});
}
