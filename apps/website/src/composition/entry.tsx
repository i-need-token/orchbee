import '@orchbee/design-tokens/tokens.css';

import {
	readAppearance,
	writeAppearance,
	type Appearance,
	type PreferenceStorage,
} from '../preferences';
import { activateWebsite } from '../website';

// The static pages carry only class names, so the browser bundle loads every component stylesheet.
const stylesheets = import.meta.webpackContext('../website', {
	recursive: true,
	regExp: /\.module\.css$/u,
});
for (const key of stylesheets.keys()) stylesheets(key);

const NEXT_APPEARANCE: Readonly<Record<Appearance, Appearance>> = {
	system: 'light',
	light: 'dark',
	dark: 'system',
};
// The header button carries each appearance's localized name in these data attributes.
const LABEL_KEY: Readonly<Record<Appearance, string>> = {
	system: 'labelSystem',
	light: 'labelLight',
	dark: 'labelDark',
};

const storage: PreferenceStorage = {
	getItem: (key) => window.localStorage.getItem(key),
	setItem: (key, value) => window.localStorage.setItem(key, value),
};
let appearance = readAppearance(storage);
const system = window.matchMedia('(prefers-color-scheme: dark)');
function applyAppearance(): void {
	document.documentElement.dataset['theme'] =
		appearance === 'system' ? (system.matches ? 'dark' : 'light') : appearance;
}
applyAppearance();
system.addEventListener('change', applyAppearance);

function activateAppearanceButton(button: HTMLButtonElement): void {
	const show = (): void => {
		const label = button.dataset[LABEL_KEY[appearance]];
		button.dataset['current'] = appearance;
		if (label !== undefined) {
			button.setAttribute('aria-label', label);
			button.title = label;
		}
	};
	show();
	button.hidden = false;
	button.addEventListener('click', () => {
		appearance = NEXT_APPEARANCE[appearance];
		applyAppearance();
		writeAppearance(storage, appearance);
		show();
	});
}

async function start(): Promise<void> {
	if (import.meta.env.DEV) {
		const [{ createRoot }, { OrchbeeI18nProvider }, { release, Website }] = await Promise.all([
			import('react-dom/client'),
			import('@orchbee/i18n'),
			import('../website'),
		]);
		const root = document.getElementById('root');
		if (root === null) throw new Error('The website mount point is missing.');
		const { pathname } = window.location;
		const locale = pathname.startsWith('/zh-CN') ? 'zh-CN' : 'en-US';
		const known = pathname === '/' || pathname === '/index.html' || /^\/zh-CN\/?$/u.test(pathname);
		document.documentElement.lang = locale;
		const { flushSync } = await import('react-dom');
		flushSync(() =>
			createRoot(root).render(
				<OrchbeeI18nProvider locale={locale}>
					<Website locale={locale} release={release} page={known ? 'home' : 'not-found'} />
				</OrchbeeI18nProvider>,
			),
		);
	}
	activateWebsite();
	const button = document.querySelector<HTMLButtonElement>('[data-appearance]');
	if (button !== null) activateAppearanceButton(button);
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
