import type { ActivationContext } from '../../../../activation';

function isMac(view: Window): boolean {
	const platform = view.navigator.platform || view.navigator.userAgent;
	return /Mac|iPhone|iPad|iPod/i.test(platform);
}

// Activates the platform tabs in the install section, selecting the visitor's OS by default.
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
			select(tab.dataset['installTab'] ?? 'windows');
		});
	}

	tabBar.hidden = false;
	select(isMac(view) ? 'macos' : 'windows');
}
