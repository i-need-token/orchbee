import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readdir, readFile, rename, rm, writeFile } from 'node:fs/promises';

import { fileURLToPath } from 'node:url';

const output = new URL('../dist/', import.meta.url);

const { renderPage, WEBSITE_PAGES } = await import(
	new URL('../dist/.prerender/render.mjs', import.meta.url).href
);
const LOCALES = ['en-US', 'zh-CN'];
const shell = await readFile(new URL('index.html', output), 'utf8');
const { origin } = renderPage('en-US', 'home');
function escape(value) {
	return value
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;');
}

// The alternate links of one page: every language, and English for any other.
function alternateLinks(page, tag) {
	const english = page.alternates.find(({ language }) => language === 'en-US');
	return [...page.alternates, { language: 'x-default', href: english.href }]
		.map(({ language, href }) => tag(language, href))
		.join('');
}

function documentFor(locale, page) {
	const indexable = page.canonical !== undefined;
	const home = indexable && /^\/(zh-CN\/)?$/u.test(new URL(page.canonical).pathname);
	const structuredData = JSON.stringify({
		'@context': 'https://schema.org',
		'@type': home ? 'WebSite' : 'WebPage',
		name: home ? 'Orchbee' : page.title,
		url: page.canonical,
		inLanguage: locale,
		description: page.description,
	}).replaceAll('<', '\\u003c');
	const scriptHash = createHash('sha256').update(structuredData).digest('base64');
	const policy = `default-src 'self'; script-src 'self' 'sha256-${scriptHash}'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'`;
	const discovery = indexable
		? `<link rel="canonical" href="${page.canonical}">
${alternateLinks(page, (language, href) => `<link rel="alternate" hreflang="${language}" href="${href}">\n`)}<meta property="og:type" content="website">
<meta property="og:site_name" content="Orchbee">
<meta property="og:title" content="${escape(page.title)}">
<meta property="og:description" content="${escape(page.description)}">
<meta property="og:url" content="${page.canonical}">
<meta property="og:image" content="${origin}/og.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="Orchbee">
<meta property="og:locale" content="${locale.replace('-', '_')}">
<meta property="og:locale:alternate" content="${locale === 'zh-CN' ? 'en_US' : 'zh_CN'}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${escape(page.title)}">
<meta name="twitter:description" content="${escape(page.description)}">
<meta name="twitter:image" content="${origin}/og.png">
<script type="application/ld+json">${structuredData}</script>`
		: '<meta name="robots" content="noindex">';
	const metadata = `<meta http-equiv="Content-Security-Policy" content="${escape(policy)}">
<meta name="description" content="${escape(page.description)}">
<meta name="referrer" content="strict-origin-when-cross-origin">
<link rel="icon" href="/favicon.ico" sizes="32x32">
<link rel="icon" type="image/svg+xml" href="/icon.svg">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
${discovery}`;
	const html = shell
		.replace(/<html[^>]*>/u, `<html lang="${locale}" dir="ltr" data-theme="dark">`)
		.replace(/<meta charset="[^"]+"\s*\/?>/u, '')
		.replace('<head>', `<head><meta charset="utf-8">${metadata}`)
		.replace(/<title>.*?<\/title>/u, `<title>${escape(page.title)}</title>`)
		.replace('<div id="root"></div>', `<div id="root">${page.markup}</div>`);
	if (!html.includes(page.markup) || html.includes('<div id="root"></div>'))
		throw new Error('Static website rendering did not fill the document.');
	return html;
}

const indexed = [];
for (const locale of LOCALES) {
	for (const name of WEBSITE_PAGES) {
		const page = renderPage(locale, name);
		const directory = new URL(`.${new URL(page.canonical).pathname}`, output);
		await mkdir(directory, { recursive: true });
		await writeFile(new URL('index.html', directory), documentFor(locale, page));
		indexed.push(page);
	}
	// Cloudflare serves the nearest 404.html above a missing path, so /zh-CN/… gets Chinese.
	const directory = locale === 'zh-CN' ? new URL('zh-CN/', output) : output;
	await writeFile(
		new URL('404.html', directory),
		documentFor(locale, renderPage(locale, 'not-found')),
	);
}
await writeFile(
	new URL('robots.txt', output),
	`User-agent: *\nAllow: /\nSitemap: ${origin}/sitemap.xml\n`,
);
await writeFile(
	new URL('sitemap.xml', output),
	`<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">${indexed.map((page) => `<url><loc>${page.canonical}</loc>${alternateLinks(page, (language, href) => `<xhtml:link rel="alternate" hreflang="${language}" href="${href}"/>`)}</url>`).join('')}</urlset>\n`,
);
// The update feed is tracked as feed.json.signed, a name the formatter leaves alone, because its
// signature covers the exact bytes; it is served as feed.json. A rename keeps every byte.
const update = new URL('update/', output);
if (existsSync(update)) {
	for (const channel of await readdir(update)) {
		for (const platform of await readdir(new URL(`${channel}/`, update))) {
			const folder = new URL(`${channel}/${platform}/`, update);
			const names = (await readdir(folder)).toSorted().join(',');
			if (names !== 'feed.json.sig,feed.json.signed')
				throw new Error(
					`public/update/${channel}/${platform}/ must hold exactly feed.json.signed and feed.json.sig, not ${names}`,
				);
			await rename(new URL('feed.json.signed', folder), new URL('feed.json', folder));
		}
	}
}
// Build-time React code is not part of the published static artifact.
await rm(new URL('.prerender/', output), { recursive: true });
console.log(
	`Generated ${indexed.length} static pages and ${LOCALES.length} 404 pages in ${fileURLToPath(output)}`,
);
