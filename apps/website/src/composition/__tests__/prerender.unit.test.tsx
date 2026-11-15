import { describe, expect, it } from 'vitest';

import { WEBSITE_PAGES } from '../../website';
import { renderPage } from '../prerender';

const LOCALES = ['en-US', 'zh-CN'] as const;
const PAGES = [...WEBSITE_PAGES, 'not-found'] as const;
const CASES = LOCALES.flatMap((locale) => PAGES.map((page) => [locale, page] as const));
const PUBLISHED = {
	published: true,
	version: '9.9.9-test.1',
	installerUrl: 'https://github.com/i-need-token/orchbee/releases/download/v9.9.9-test.1/test.exe',
} as const;
const UNPUBLISHED = { published: false } as const;

// Every resource a page loads must come from the site itself.
function loadedAddresses(markup: string): string[] {
	return [...markup.matchAll(/\s(?:src|srcset|poster|data)="([^"]+)"/gu)].flatMap((match) =>
		(match[1] ?? '').split(',').map((candidate) => candidate.trim().split(/\s+/u)[0] ?? ''),
	);
}

describe('localized static website', () => {
	it.each(CASES)('renders the %s %s page without a browser runtime', (locale, page) => {
		for (const release of [PUBLISHED, UNPUBLISHED]) {
			const { markup } = renderPage(locale, page, release);
			expect(markup.match(/<h1[\s>]/gu)).toHaveLength(1);
			// The Content Security Policy refuses inline styles.
			expect(markup).not.toMatch(/\sstyle=/u);
			expect(markup).not.toContain('<iframe');
			expect(markup).not.toContain('<form');
			for (const address of loadedAddresses(markup)) expect(address).toMatch(/^\/(?!\/)/u);
		}
	});

	it.each(CASES)(
		'offers the repository and a source build before a release (%s %s)',
		(locale, page) => {
			const { markup } = renderPage(locale, page, UNPUBLISHED);
			expect(markup).toContain('href="https://github.com/i-need-token/orchbee"');
			expect(markup).not.toContain('npm install -g orchbee@next');
			expect(markup).not.toContain('/releases/download/');
		},
	);

	it.each(LOCALES)('offers the installer and the npm package after a release (%s)', (locale) => {
		const { markup } = renderPage(locale, 'home', PUBLISHED);
		expect(markup).toContain(`href="${PUBLISHED.installerUrl}"`);
		expect(markup).toContain('npm install -g orchbee@next');
		expect(markup).toContain(PUBLISHED.version);
		expect(markup).not.toContain('pnpm desktop');
	});

	it.each(WEBSITE_PAGES)('gives the %s page a canonical address in each language', (page) => {
		const english = renderPage('en-US', page);
		const chinese = renderPage('zh-CN', page);
		expect(english.canonical).toMatch(/^https:\/\/orchbee\.com\//u);
		expect(chinese.canonical).toBe(english.canonical?.replace('.com/', '.com/zh-CN/'));
		for (const metadata of [english, chinese])
			expect(metadata.alternates).toEqual([
				{ language: 'en-US', href: english.canonical },
				{ language: 'zh-CN', href: chinese.canonical },
			]);
		expect(english.title).not.toEqual(chinese.title);
		expect(english.description).not.toEqual(chinese.description);
	});

	it.each(LOCALES)('renders a %s not-found page that leads home and is not indexed', (locale) => {
		const page = renderPage(locale, 'not-found');
		expect(page.markup).toContain(`href="${locale === 'zh-CN' ? '/zh-CN/' : '/'}"`);
		expect(page.canonical).toBeUndefined();
		expect(page.title).not.toEqual(renderPage(locale, 'home').title);
	});
});
