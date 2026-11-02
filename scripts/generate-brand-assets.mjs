// Regenerates resources/brand/ from resources/icon.svg; `--check` verifies the tracked output.
// Generation renders with the installed Google Chrome through playwright-core.
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

import {
	brandAssetPlan,
	encodeIcns,
	encodeIco,
	inspectBrandAsset,
	renderBrandSvgs,
} from './brand-assets.mjs';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const outputRoot = path.join(repositoryRoot, 'resources', 'brand');
const supportedArguments = new Set(['--check']);
const unexpectedArguments = process.argv
	.slice(2)
	.filter((argument) => !supportedArguments.has(argument));
if (unexpectedArguments.length > 0) {
	throw new Error(`Unsupported arguments: ${unexpectedArguments.join(', ')}`);
}

const svgs = renderBrandSvgs(
	await readFile(path.join(repositoryRoot, 'resources', 'icon.svg'), 'utf8'),
);
const plan = brandAssetPlan();

async function listFiles(directory, prefix = '') {
	const entries = await readdir(directory, { withFileTypes: true }).catch(() => []);
	const nested = await Promise.all(
		entries.map((entry) =>
			entry.isDirectory()
				? listFiles(path.join(directory, entry.name), `${prefix}${entry.name}/`)
				: [`${prefix}${entry.name}`],
		),
	);
	return nested.flat();
}

if (process.argv.includes('--check')) {
	const tracked = new Set(await listFiles(outputRoot));
	const problems = [...tracked]
		.filter((file) => !plan.has(file))
		.map((file) => `${file}: unexpected`);
	for (const [file, entry] of plan) {
		if (!tracked.has(file)) {
			problems.push(`${file}: missing`);
			continue;
		}
		const problem = inspectBrandAsset(entry, await readFile(path.join(outputRoot, file)), svgs);
		if (problem !== null) problems.push(`${file}: ${problem}`);
	}
	if (problems.length > 0) {
		throw new Error(
			`resources/brand/ is stale. Run pnpm generate:brand-assets.\n${problems.join('\n')}`,
		);
	}
} else {
	const { chromium } = await import('playwright-core');
	const browser = await chromium.launch({ channel: 'chrome' });
	try {
		const page = await browser.newPage({ deviceScaleFactor: 1 });
		const renderings = new Map();
		const render = async (svg, width, height, options = {}) => {
			const key = JSON.stringify([svg, width, height, options]);
			if (renderings.has(key)) return renderings.get(key);
			const { background = null, kind = 'png', markSize = width } = options;
			await page.setViewportSize({ width, height });
			const source = `data:image/svg+xml;base64,${Buffer.from(svgs.get(svg)).toString('base64')}`;
			await page.setContent(
				`<!doctype html><body style="margin:0;width:${width}px;height:${height}px;display:grid;place-items:center;background:${background ?? 'transparent'}"><img src="${source}" width="${markSize}" height="${markSize}" style="display:block"></body>`,
			);
			await page.locator('img').evaluate((image) => image.decode());
			const image = await page.screenshot({
				omitBackground: background === null,
				type: kind,
				...(kind === 'jpeg' ? { quality: 95 } : {}),
			});
			renderings.set(key, image);
			return image;
		};
		await rm(outputRoot, { force: true, recursive: true });
		for (const [file, entry] of plan) {
			let content;
			if (entry.kind === 'svg') {
				content = `${svgs.get(entry.svg)}\n`;
			} else if (entry.kind === 'ico' || entry.kind === 'icns') {
				// One page renders every image, so renders stay sequential.
				const images = [];
				for (const item of entry.kind === 'ico' ? entry.sizes : entry.entries) {
					const [label, size] = entry.kind === 'ico' ? [item, item] : item;
					images.push([label, await render(entry.svg, size, size)]);
				}
				content = entry.kind === 'ico' ? encodeIco(images) : encodeIcns(images);
			} else {
				content = await render(entry.svg, entry.width, entry.height, entry);
			}
			await mkdir(path.dirname(path.join(outputRoot, file)), { recursive: true });
			await writeFile(path.join(outputRoot, file), content);
		}
	} finally {
		await browser.close();
	}
}
