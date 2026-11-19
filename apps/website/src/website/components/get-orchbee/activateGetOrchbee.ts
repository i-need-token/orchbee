import type { ActivationContext } from '../../activation';
import { visitorFirst, visitorPlatform } from '../../release';

// Moves the visitor's platform button to the front of each call to action and marks it primary. A call to action
// without a button for the visitor's platform keeps the catalog order.
export function activateGetOrchbee({ document }: ActivationContext): void {
	const view = document.defaultView;
	if (view === null) return;
	const preferred = visitorPlatform(view.navigator.platform || view.navigator.userAgent);
	if (preferred === undefined) return;
	for (const group of document.querySelectorAll<HTMLElement>('[data-get-orchbee]')) {
		const buttons = [...group.querySelectorAll<HTMLElement>('[data-platform]')];
		const ordered: readonly HTMLElement[] = visitorFirst(
			buttons,
			(button) => button.dataset['platform'],
			preferred,
		);
		const leader: HTMLElement | undefined = ordered[0];
		if (leader === undefined || leader.dataset['platform'] !== preferred) continue;
		for (const button of ordered) {
			button.removeAttribute('data-primary');
			button.parentElement?.append(button);
		}
		leader.setAttribute('data-primary', '');
	}
}
