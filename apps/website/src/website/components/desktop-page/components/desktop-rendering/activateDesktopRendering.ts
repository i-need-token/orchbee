import type { ActivationContext } from '../../../../activation';
import { bindOptionSwitch } from '../option-switch/bindOptionSwitch';

// Shows the ligature switch and runs it on the sample.
export function activateDesktopRendering({ document }: ActivationContext): void {
	const root = document.querySelector('[data-desktop-rendering]');
	const sample = root?.querySelector('[data-ligature-sample]');
	if (root === null || root === undefined || sample === null || sample === undefined) return;
	bindOptionSwitch(root, 'ligatures', (option) => {
		sample.toggleAttribute('data-off', option === 'off');
	});
}
