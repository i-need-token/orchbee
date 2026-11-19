import type { ActivationContext } from '../../activation';

function isMac(view: Window): boolean {
	const platform = view.navigator.platform || view.navigator.userAgent;
	return /Mac|iPhone|iPad|iPod/i.test(platform);
}

// Highlights the visitor's platform download button as primary and places it first.
export function activateGetOrchbee({ document }: ActivationContext): void {
	const view = document.defaultView;
	if (view === null || !isMac(view)) return;
	for (const group of document.querySelectorAll<HTMLElement>('[data-get-orchbee]')) {
		const win = group.querySelector<HTMLElement>('[data-platform="windows"]');
		const mac = group.querySelector<HTMLElement>('[data-platform="macos"]');
		if (win === null || mac === null) continue;
		win.removeAttribute('data-primary');
		mac.setAttribute('data-primary', '');
		win.before(mac);
	}
}
