import type { ActivationContext } from '../../../../activation';
import { visitorPlatform } from '../../../../release';

// Activates the platform tabs in the install section. The section opens on the visitor's platform when it has a
// tab, and on the first platform otherwise.
export function activateInstall({ document }: ActivationContext): void {
	const view = document.defaultView;
	const root = document.querySelector<HTMLElement>('[data-install]');
	if (view === null || root === null) return;
	const tabBar = root.querySelector<HTMLElement>('[data-install-tabs]');
	if (tabBar === null) return;
	const tabs = [...tabBar.querySelectorAll<HTMLButtonElement>('button[data-install-tab]')];
	const rows = [...root.querySelectorAll<HTMLElement>('[data-install-group]')];
	const notes = [...root.querySelectorAll<HTMLElement>('[data-install-note]')];
	if (tabs.length === 0) return;

	const select = (chosen: string): void => {
		for (const tab of tabs) {
			tab.setAttribute('aria-pressed', String(tab.dataset['installTab'] === chosen));
		}
		for (const row of rows) {
			const group = row.dataset['installGroup'];
			row.hidden = group !== 'all' && group !== chosen;
		}
		for (const note of notes) {
			note.hidden = note.dataset['installNote'] !== chosen;
		}
	};

	for (const tab of tabs) {
		tab.addEventListener('click', () => {
			select(tab.dataset['installTab'] ?? '');
		});
	}

	tabBar.hidden = false;
	const preferred = visitorPlatform(view.navigator.platform || view.navigator.userAgent);
	const opening = tabs.find((tab) => tab.dataset['installTab'] === preferred) ?? tabs[0];
	select(opening?.dataset['installTab'] ?? '');
}
