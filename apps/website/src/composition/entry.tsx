import '@orchbee/design-tokens/tokens.css';

import {
	parseAppearance,
	readAppearance,
	writeAppearance,
	type PreferenceStorage,
} from '../preferences';
import { activateWebsite } from '../website';

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

async function start(): Promise<void> {
	if (import.meta.env.DEV) {
		const [{ createRoot }, { OrchbeeI18nProvider }, { Website }] = await Promise.all([
			import('react-dom/client'),
			import('@orchbee/i18n'),
			import('../website'),
		]);
		const root = document.getElementById('root');
		if (root === null) throw new Error('The website mount point is missing.');
		const locale = window.location.pathname.startsWith('/zh-CN') ? 'zh-CN' : 'en-US';
		document.documentElement.lang = locale;
		const { flushSync } = await import('react-dom');
		flushSync(() =>
			createRoot(root).render(
				<OrchbeeI18nProvider locale={locale}>
					<Website locale={locale} />
				</OrchbeeI18nProvider>,
			),
		);
	}
	activateWebsite();
	const select = document.querySelector<HTMLSelectElement>('[data-appearance]');
	if (select !== null) {
		select.value = appearance;
		select.addEventListener('change', () => {
			appearance = parseAppearance(select.value);
			applyAppearance();
			writeAppearance(storage, appearance);
		});
	}
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
