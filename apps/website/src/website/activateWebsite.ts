import { readClientStrings } from './readClientStrings';
import { activateHero } from './components/hero/activateHero';
import { activateInstall } from './components/install/activateInstall';
import { activateSwarm } from './components/page-shell/activateSwarm';
import { activateRecordings } from './components/product-recording/activateRecordings';
import { activateHeader } from './components/site-header/activateHeader';

const COPIED_FEEDBACK_MS = 1600;

// Adds the behavior the static pages cannot carry: the hero story, the screen recordings, the
// scroll-driven header, the background swarm, copy buttons, and a language link that keeps the
// visitor's place.
export function activateWebsite(): void {
	for (const link of document.querySelectorAll<HTMLAnchorElement>('[data-language-link]')) {
		link.addEventListener('click', () => {
			link.hash = window.location.hash;
		});
	}
	const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
	activateSwarm(document, reducedMotion);
	activateRecordings(document, reducedMotion);
	const strings = readClientStrings(document);
	activateHeader(document, strings?.scenes ?? []);
	if (strings !== undefined) {
		activateHero(document, strings, reducedMotion);
		activateInstall(document);
	}
	if (!window.isSecureContext || navigator.clipboard === undefined) return;
	for (const button of document.querySelectorAll<HTMLButtonElement>('[data-copy]')) {
		let reset: number | undefined;
		button.hidden = false;
		button.addEventListener('click', () => {
			void navigator.clipboard.writeText(button.dataset['copy'] ?? '').then(() => {
				button.dataset['copied'] = '';
				window.clearTimeout(reset);
				reset = window.setTimeout(() => {
					delete button.dataset['copied'];
				}, COPIED_FEEDBACK_MS);
			});
		});
	}
}
