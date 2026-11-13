import { renderTuiLines } from '../tui-screen/renderTuiLines';
import { mirrorFrame } from './mirrorFrame';

const WORKING_MS = 5000;

// One claude-1 shown by three clients: whatever the visitor types into any prompt appears in all
// of them, and Enter sends it, so every client shows claude-1 working for a moment.
export function activateMirror(document: Document): void {
	const devices = document.querySelector<HTMLElement>('[data-mirror]');
	const screen = document.querySelector('[data-tui="mirror"]');
	if (devices === null || screen === null) return;
	const inputs = [...devices.querySelectorAll<HTMLInputElement>('[data-mirror-input]')];
	const all = (selector: string): HTMLElement[] => [
		...devices.querySelectorAll<HTMLElement>(selector),
	];
	let draft = '';
	let sent = '';
	let focused = false;
	let working = false;
	let workingTimer: number | undefined;

	const render = (): void => {
		devices.toggleAttribute('data-active', draft !== '' || focused || sent !== '');
		for (const input of inputs) if (input.value !== draft) input.value = draft;
		for (const view of all('[data-mirror-draft]')) view.textContent = draft;
		for (const row of all('[data-mirror-sent]')) row.hidden = sent === '';
		for (const text of all('[data-mirror-sent-text]')) text.textContent = sent;
		for (const row of all('[data-mirror-working]')) row.hidden = !working;
		for (const dot of all('[data-hook="mirror-dot"]'))
			dot.dataset['state'] = working ? 'working' : 'done';
	};
	const setWorking = (next: boolean): void => {
		if (working === next) return;
		working = next;
		renderTuiLines(screen, mirrorFrame(next ? 'working' : 'done'));
	};

	for (const input of inputs) {
		input.addEventListener('input', () => {
			draft = input.value.slice(0, input.maxLength > 0 ? input.maxLength : undefined);
			render();
		});
		input.addEventListener('focus', () => {
			focused = true;
			render();
		});
		input.addEventListener('blur', () => {
			focused = false;
			render();
		});
		input.addEventListener('keydown', (event) => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			const value = draft.trim();
			if (value === '') return;
			sent = value;
			draft = '';
			setWorking(true);
			window.clearTimeout(workingTimer);
			workingTimer = window.setTimeout(() => {
				setWorking(false);
				render();
			}, WORKING_MS);
			render();
		});
	}
	render();
}
