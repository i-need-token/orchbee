import '@fontsource-variable/geist/wght.css';
import '@fontsource-variable/geist-mono/wght.css';
import '@fontsource-variable/jetbrains-mono/wght.css';
import '@orchbee/design-tokens/tokens.css';

import { activations } from '../website';

// The static pages carry only class names, so the browser bundle loads every component stylesheet.
const stylesheets = import.meta.webpackContext('../website', {
	recursive: true,
	regExp: /\.module\.css$/u,
});
for (const key of stylesheets.keys()) stylesheets(key);

// This script runs in the document head, before the body renders, so these marks are in place for
// the first paint: data-js lets the page style script-only states, and data-motion lets content
// start hidden for its entrance. Under reduced motion every scene keeps its final state.
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
document.documentElement.toggleAttribute('data-js', true);
document.documentElement.toggleAttribute('data-motion', !reducedMotion);

async function start(): Promise<void> {
	if (import.meta.env.DEV) (await import('./render-development-page')).renderDevelopmentPage();
	for (const activate of activations) activate({ document, reducedMotion });
}
if (document.readyState === 'loading')
	document.addEventListener(
		'DOMContentLoaded',
		() => {
			void start();
		},
		{ once: true },
	);
else void start();
