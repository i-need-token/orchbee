import styles from './Website.module.css';
import './components/showcase/Showcase.module.css';

export function activateWebsite(): void {
	document.querySelector(`.${styles['appearance']}`)?.removeAttribute('hidden');
	document.querySelector('[data-showcase-controls]')?.removeAttribute('hidden');
	const buttons = document.querySelectorAll<HTMLButtonElement>('[data-demo-button]');
	const panels = document.querySelectorAll<HTMLElement>('[data-demo-panel]');
	for (const button of buttons) {
		button.addEventListener('click', () => {
			for (const peer of buttons) peer.setAttribute('aria-pressed', String(peer === button));
			for (const panel of panels)
				panel.hidden = panel.dataset['demoPanel'] !== button.dataset['demoButton'];
		});
	}
	for (const link of document.querySelectorAll<HTMLAnchorElement>('[data-language-link]')) {
		link.addEventListener('click', () => {
			link.hash = window.location.hash;
		});
	}
	const menu = document.querySelector<HTMLDetailsElement>('[data-mobile-menu]');
	if (menu !== null) {
		menu.addEventListener('click', (event) => {
			if (event.target instanceof HTMLAnchorElement) menu.open = false;
		});
		document.addEventListener('keydown', (event) => {
			if (event.key === 'Escape' && menu.open) {
				menu.open = false;
				menu.querySelector('summary')?.focus();
			}
		});
		document.addEventListener('click', (event) => {
			if (event.target instanceof Node && !menu.contains(event.target)) menu.open = false;
		});
	}
}
