import type { ActivationContext } from '../../../../../../activation';

const SECOND_MS = 1000;

function pad(value: number): string {
	return String(value).padStart(2, '0');
}

function clockText(seconds: number): string {
	return `${pad(Math.floor(seconds / 60))}:${pad(seconds % 60)}`;
}

// Shows the stage controls and runs them: the client switch, Close and Reattach with the clock of
// how long the window has been closed.
export function activateHeroStage({ document }: ActivationContext): void {
	const view = document.defaultView;
	const root = document.querySelector('[data-hero-stage]');
	if (view === null || root === null) return;
	const bar = root.querySelector<HTMLElement>('[data-stage-bar]');
	const stage = root.querySelector('[data-stage]');
	const close = root.querySelector<HTMLButtonElement>('[data-stage-close]');
	const reattach = root.querySelector<HTMLButtonElement>('[data-stage-reattach]');
	const clock = root.querySelector('[data-stage-clock]');
	if (bar === null || stage === null || close === null || reattach === null || clock === null)
		return;
	const buttons = [...bar.querySelectorAll<HTMLButtonElement>('button[data-view]')];
	const views = [...stage.querySelectorAll('[data-view]')];

	let closedAt = 0;
	let timer = 0;
	const show = (name: string): void => {
		for (const element of views)
			element.toggleAttribute('data-on', element.getAttribute('data-view') === name);
		for (const button of buttons)
			button.setAttribute('aria-pressed', String(button.dataset['view'] === name));
	};
	const paint = (): void => {
		clock.textContent = clockText(Math.floor((Date.now() - closedAt) / SECOND_MS));
	};
	const setClosed = (closed: boolean): void => {
		if (stage.hasAttribute('data-closed') === closed) return;
		stage.toggleAttribute('data-closed', closed);
		close.disabled = closed;
		view.clearInterval(timer);
		if (!closed) return;
		closedAt = Date.now();
		paint();
		timer = view.setInterval(paint, SECOND_MS);
	};

	for (const button of buttons) {
		button.addEventListener('click', () => {
			setClosed(false);
			show(button.dataset['view'] ?? '');
		});
	}
	// Focus follows the control that replaces the one pressed.
	close.addEventListener('click', () => {
		setClosed(true);
		reattach.focus();
	});
	reattach.addEventListener('click', () => {
		setClosed(false);
		close.focus();
	});
	bar.hidden = false;
}
