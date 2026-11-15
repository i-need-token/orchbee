import type { ActivationContext } from '../../../../activation';
import { applyTerminalTheme } from '../../../terminal-text/applyTerminalTheme';
import { bindOptionSwitch } from '../option-switch/bindOptionSwitch';

// Long enough for the window's colors to finish their cross-fade.
const FADE_MS = 400;

// Shows the appearance switch and runs it: the drawn window takes the theme the pressed button
// carries, the product's light or dark look with the Orchbee terminal theme of the same polarity,
// cross-fading unless the visitor asked for reduced motion.
export function activateDesktopStage({ document, reducedMotion }: ActivationContext): void {
	const view = document.defaultView;
	const stage = document.querySelector<HTMLElement>('[data-desktop-stage]');
	const window = stage?.querySelector('[data-window-fit]')?.firstElementChild;
	if (view === null || stage === null || stage === undefined || !(window instanceof HTMLElement))
		return;
	let timer = 0;
	bindOptionSwitch(stage, 'appearance', (option, button) => {
		if (window.getAttribute('data-theme') === option) return;
		if (!reducedMotion) {
			view.clearTimeout(timer);
			stage.toggleAttribute('data-fading', true);
			timer = view.setTimeout(() => stage.toggleAttribute('data-fading', false), FADE_MS);
		}
		applyTerminalTheme(window, button);
	});
}
