import { createHash } from 'node:crypto';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';

import { fileURLToPath } from 'node:url';

const output = new URL('../dist/', import.meta.url);

const { renderPage } = await import(new URL('../dist/.prerender/render.mjs', import.meta.url).href);
const shell = await readFile(new URL('index.html', output), 'utf8');
const origin = new URL(renderPage('en-US').canonical).origin;
function escape(value) {
	return value
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;');
}

for (const locale of ['en-US', 'zh-CN']) {
	const page = renderPage(locale);
	const structuredData = JSON.stringify({
		'@context': 'https://schema.org',
		'@type': 'WebSite',
		name: 'Orchbee',
		url: page.canonical,
		inLanguage: locale,
		description: page.description,
	}).replaceAll('<', '\\u003c');
	const scriptHash = createHash('sha256').update(structuredData).digest('base64');
	const policy = `default-src 'self'; script-src 'self' 'sha256-${scriptHash}'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'`;
	const metadata = `<meta http-equiv="Content-Security-Policy" content="${escape(policy)}">
<meta name="description" content="${escape(page.description)}">
<meta name="referrer" content="strict-origin-when-cross-origin">
<link rel="icon" type="image/svg+xml" href="/icon.svg">
<link rel="canonical" href="${page.canonical}">
<link rel="alternate" hreflang="en-US" href="${origin}/">
<link rel="alternate" hreflang="zh-CN" href="${origin}/zh-CN/">
<link rel="alternate" hreflang="x-default" href="${origin}/">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Orchbee">
<meta property="og:title" content="${escape(page.title)}">
<meta property="og:description" content="${escape(page.description)}">
<meta property="og:url" content="${page.canonical}">
<meta property="og:locale" content="${locale.replace('-', '_')}">
<meta property="og:locale:alternate" content="${locale === 'zh-CN' ? 'en_US' : 'zh_CN'}">
<meta name="twitter:card" content="summary">
<meta name="twitter:title" content="${escape(page.title)}">
<meta name="twitter:description" content="${escape(page.description)}">
<script type="application/ld+json">${structuredData}</script>`;
	const html = shell
		.replace(/<html[^>]*>/u, `<html lang="${locale}" dir="ltr" data-theme="dark">`)
		.replace(/<meta charset="[^"]+"\s*\/?>/u, '')
		.replace('<head>', `<head><meta charset="utf-8">${metadata}`)
		.replace(/<title>.*?<\/title>/u, `<title>${escape(page.title)}</title>`)
		.replace('<div id="root"></div>', `<div id="root">${page.markup}</div>`);
	if (!html.includes(page.markup) || html.includes('<div id="root"></div>'))
		throw new Error('Static website rendering did not fill the document.');
	const directory = locale === 'zh-CN' ? new URL('zh-CN/', output) : output;
	await mkdir(directory, { recursive: true });
	await writeFile(new URL('index.html', directory), html);
}
await writeFile(
	new URL('robots.txt', output),
	`User-agent: *\nAllow: /\nSitemap: ${origin}/sitemap.xml\n`,
);
await writeFile(
	new URL('sitemap.xml', output),
	`<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">${['/', '/zh-CN/'].map((path) => `<url><loc>${origin}${path}</loc><xhtml:link rel="alternate" hreflang="en-US" href="${origin}/"/><xhtml:link rel="alternate" hreflang="zh-CN" href="${origin}/zh-CN/"/><xhtml:link rel="alternate" hreflang="x-default" href="${origin}/"/></url>`).join('')}</urlset>\n`,
);
// Build-time React code is not part of the published static artifact.
await rm(new URL('.prerender/', output), { recursive: true });
console.log(`Generated English and Simplified Chinese static pages in ${fileURLToPath(output)}`);
