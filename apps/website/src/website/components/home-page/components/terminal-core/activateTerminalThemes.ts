import type { ActivationContext } from '../../../../activation';
import { applyTerminalTheme } from '../../../terminal-text/applyTerminalTheme';

// How long each theme stays in the window while the themes take turns.
const TURN_MS = 2600;

// Enables the theme buttons: a pressed one gives the window its theme and title for good. Until
// the visitor picks one, the themes take turns in list order while at least half of the list is
// on screen and the page is visible, never under reduced motion.
export function activateTerminalThemes({ document, reducedMotion }: ActivationContext): void {
	const view = document.defaultView;
	const root = document.querySelector<HTMLElement>('[data-terminal-themes]');
	const frame = root?.querySelector<HTMLElement>('[data-theme-window]');
	const title = root?.querySelector<HTMLElement>('[data-theme-name]');
	const list = root?.querySelector<HTMLElement>('[data-theme-list]');
	if (view === null || root === null || root === undefined) return;
	if (frame === null || frame === undefined || title === null || title === undefined) return;
	if (list === null || list === undefined) return;
	const buttons = [...root.querySelectorAll<HTMLButtonElement>('button[data-terminal-theme]')];
	let shown = Math.max(
		0,
		buttons.findIndex((button) => button.getAttribute('aria-pressed') === 'true'),
	);

	const show = (index: number): void => {
		const button = buttons[index];
		if (button === undefined) return;
		shown = index;
		applyTerminalTheme(frame, button);
		title.textContent = button.querySelector('[data-theme-label]')?.textContent ?? '';
		for (const other of buttons) other.setAttribute('aria-pressed', String(other === button));
	};

	let timer = 0;
	let onScreen = false;
	let picked = false;
	const update = (): void => {
		const run = !reducedMotion && !picked && onScreen && document.visibilityState === 'visible';
		if (run && timer === 0)
			timer = view.setInterval(() => show((shown + 1) % buttons.length), TURN_MS);
		if (!run && timer !== 0) {
			view.clearInterval(timer);
			timer = 0;
		}
	};

	for (const [index, button] of buttons.entries()) {
		button.disabled = false;
		button.addEventListener('click', () => {
			picked = true;
			update();
			show(index);
		});
	}
	if (reducedMotion) return;
	new IntersectionObserver(
		([entry]) => {
			onScreen = entry?.isIntersecting ?? false;
			update();
		},
		{ threshold: 0.5 },
	).observe(list);
	document.addEventListener('visibilitychange', update);
}
