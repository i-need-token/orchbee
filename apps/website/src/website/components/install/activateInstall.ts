type Os = 'mac' | 'win' | 'linux';

function detectOs(userAgent: string): Os {
	if (/Mac/iu.test(userAgent) && !/iPhone|iPad/iu.test(userAgent)) return 'mac';
	if (/Linux|X11/iu.test(userAgent) && !/Android/iu.test(userAgent)) return 'linux';
	return 'win';
}

// The install window opens on the visitor's own system; its tabs switch the shell and steps.
export function activateInstall(document: Document): Os {
	const os = detectOs(navigator.userAgent);
	const root = document.querySelector<HTMLElement>('[data-install]');
	if (root === null) return os;
	const show = (picked: string): void => {
		for (const tab of root.querySelectorAll<HTMLElement>('[data-os-tab]'))
			tab.setAttribute('aria-pressed', String(tab.dataset['osTab'] === picked));
		for (const panel of root.querySelectorAll<HTMLElement>('[data-os-panel]'))
			panel.hidden = panel.dataset['osPanel'] !== picked;
		for (const title of root.querySelectorAll<HTMLElement>('[data-os-title]'))
			title.hidden = title.dataset['osTitle'] !== picked;
	};
	for (const tab of root.querySelectorAll<HTMLElement>('[data-os-tab]'))
		tab.addEventListener('click', () => show(tab.dataset['osTab'] ?? 'win'));
	show(os);
	return os;
}
