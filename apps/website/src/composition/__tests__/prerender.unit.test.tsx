import { describe, expect, it } from 'vitest';
import { renderNotFound, renderPage } from '../prerender';

const LOCALES = ['en-US', 'zh-CN'] as const;
const SECTIONS = ['anywhere', 'agents', 'orchestrate', 'private', 'terminal', 'install'] as const;
const PUBLISHED = {
	published: true,
	version: '9.9.9-test.1',
	installerUrl: 'https://github.com/i-need-token/orchbee/releases/download/v9.9.9-test.1/test.exe',
} as const;

// Every resource the page loads must come from the site itself; icons come from the page.
function loadedAddresses(markup: string): string[] {
	return [...markup.matchAll(/\s(?:src|srcset)="([^"]+)"/gu)].flatMap((match) =>
		(match[1] ?? '').split(',').map((candidate) => candidate.trim().split(/\s+/u)[0] ?? ''),
	);
}

describe('localized static website', () => {
	it.each(LOCALES)('renders every section of the %s page without a browser runtime', (locale) => {
		const { markup } = renderPage(locale);
		expect(markup.match(/<h1[\s>]/gu)).toHaveLength(1);
		for (const section of SECTIONS) expect(markup).toContain(`id="${section}"`);
		expect(markup).not.toContain('<iframe');
		expect(markup).not.toContain('<form');
		for (const address of loadedAddresses(markup)) expect(address).toMatch(/^\/(?!\/)/u);
		for (const [, icon] of markup.matchAll(/<use\s[^>]*href="([^"]*)"/gu))
			expect(icon).toMatch(/^#/u);
	});

	it.each(LOCALES)('offers the repository and a source build before a release (%s)', (locale) => {
		const { markup } = renderPage(locale, { published: false });
		expect(markup).toContain('href="https://github.com/i-need-token/orchbee"');
		expect(markup).toContain('pnpm desktop');
		expect(markup).not.toContain('npm install -g orchbee@next');
		expect(markup).not.toContain('/releases/download/');
	});

	it.each(LOCALES)('offers the installer and the npm package after a release (%s)', (locale) => {
		const { markup } = renderPage(locale, PUBLISHED);
		expect(markup).toContain(`href="${PUBLISHED.installerUrl}"`);
		expect(markup).toContain('npm install -g orchbee@next');
		expect(markup).toContain(PUBLISHED.version);
		expect(markup).not.toContain('pnpm desktop');
	});

	it('gives both languages stable canonical URLs and localized search metadata', () => {
		const english = renderPage('en-US');
		const chinese = renderPage('zh-CN');
		expect(english.canonical).toBe('https://orchbee.com/');
		expect(chinese.canonical).toBe('https://orchbee.com/zh-CN/');
		expect(english.alternate).toBe('/zh-CN/');
		expect(chinese.alternate).toBe('/');
		expect(english.title).not.toEqual(chinese.title);
		expect(english.description).not.toEqual(chinese.description);
	});

	it.each(LOCALES)('renders a %s not-found page that leads home', (locale) => {
		const page = renderNotFound(locale);
		expect(page.markup.match(/<h1[\s>]/gu)).toHaveLength(1);
		expect(page.markup).toContain(`href="${locale === 'zh-CN' ? '/zh-CN/' : '/'}"`);
		expect(page.title).not.toEqual(renderPage(locale).title);
	});
});
