// Derives the website's product pictures from the README captures in resources/readme/.
// `pnpm --filter @orchbee/website pictures` re-encodes them with the installed Google Chrome through
// playwright-core; `--check` verifies the tracked output without a browser and runs before every
// build, so a retaken capture cannot ship with stale website copies.
import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const sourceRoot = path.join(repositoryRoot, 'resources', 'readme');
// The website serves the stills as they are from public/pictures/; the digests of the sources they
// were made from sit outside public/ so they are not published.
const websiteRoot = path.join(repositoryRoot, 'apps', 'website');
const outputRoot = path.join(websiteRoot, 'public', 'pictures');
const manifestPath = path.join(websiteRoot, 'picture-sources.json');
const QUALITY = 0.9;

// Each output names its README source, its size, and for an animation the frame it keeps.
const PLAN = new Map([
	['hero-1600.webp', { source: 'hero.png', width: 1600, height: 882 }],
	['hero-800.webp', { source: 'hero.png', width: 800, height: 441 }],
	['desktop-1600.webp', { source: 'desktop.png', width: 1600, height: 888 }],
	['desktop-800.webp', { source: 'desktop.png', width: 800, height: 444 }],
	['multiplex-still.webp', { source: 'multiplex.gif', width: 1100, height: 324, frame: 'last' }],
	['agents-cli-still.webp', { source: 'agents-cli.gif', width: 1100, height: 605, frame: 'last' }],
]);

const unexpectedArguments = process.argv.slice(2).filter((argument) => argument !== '--check');
if (unexpectedArguments.length > 0) {
	throw new Error(`Unsupported arguments: ${unexpectedArguments.join(', ')}`);
}

async function sourceDigests() {
	const sources = [...new Set([...PLAN.values()].map((entry) => entry.source))].toSorted();
	const digests = {};
	for (const source of sources) {
		const bytes = await readFile(path.join(sourceRoot, source));
		digests[source] = createHash('sha256').update(bytes).digest('hex');
	}
	return digests;
}

// Reads the canvas size from a RIFF WebP header (lossy VP8, lossless VP8L, or extended VP8X).
function webpSize(buffer) {
	if (buffer.toString('ascii', 0, 4) !== 'RIFF' || buffer.toString('ascii', 8, 12) !== 'WEBP') {
		return null;
	}
	const chunk = buffer.toString('ascii', 12, 16);
	if (chunk === 'VP8 ') {
		return [buffer.readUInt16LE(26) & 0x3fff, buffer.readUInt16LE(28) & 0x3fff];
	}
	if (chunk === 'VP8L') {
		const bits = buffer.readUInt32LE(21);
		return [(bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1];
	}
	if (chunk === 'VP8X') {
		return [buffer.readUIntLE(24, 3) + 1, buffer.readUIntLE(27, 3) + 1];
	}
	return null;
}

const digests = await sourceDigests();

if (process.argv.includes('--check')) {
	const problems = [];
	const tracked = new Set(await readdir(outputRoot).catch(() => []));
	for (const file of tracked) {
		if (!PLAN.has(file)) problems.push(`${file}: unexpected`);
	}
	const manifest = await readFile(manifestPath, 'utf8')
		.then((text) => JSON.parse(text))
		.catch(() => null);
	if (JSON.stringify(manifest) !== JSON.stringify(digests)) {
		problems.push(
			`${path.basename(manifestPath)}: the README captures changed since the pictures were generated`,
		);
	}
	for (const [file, entry] of PLAN) {
		if (!tracked.has(file)) {
			problems.push(`${file}: missing`);
			continue;
		}
		const size = webpSize(await readFile(path.join(outputRoot, file)));
		if (JSON.stringify(size) !== JSON.stringify([entry.width, entry.height])) {
			problems.push(`${file}: expected ${entry.width}x${entry.height}, found ${String(size)}`);
		}
	}
	if (problems.length > 0) {
		throw new Error(
			`The website pictures are stale. Run pnpm --filter @orchbee/website pictures.\n${problems.join('\n')}`,
		);
	}
} else {
	const { chromium } = await import('playwright-core');
	const browser = await chromium.launch({ channel: 'chrome' });
	try {
		const page = await browser.newPage();
		// ImageDecoder exists only in a secure context, so the page is served from a fulfilled
		// HTTPS route that never reaches the network.
		const origin = 'https://pictures.invalid/';
		await page.route('**/*', (route) =>
			route.request().url() === origin
				? route.fulfill({
						contentType: 'text/html',
						body: '<!doctype html><title>pictures</title>',
					})
				: route.abort(),
		);
		await page.goto(origin);
		await rm(outputRoot, { force: true, recursive: true });
		await mkdir(outputRoot, { recursive: true });
		for (const [file, entry] of PLAN) {
			const bytes = await readFile(path.join(sourceRoot, entry.source));
			const encoded = await page.evaluate(
				async ({ base64, type, width, height, frame, quality }) => {
					const data = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
					let image;
					if (frame === undefined) {
						image = await createImageBitmap(new Blob([data], { type }));
					} else {
						const decoder = new ImageDecoder({ data, type });
						await decoder.tracks.ready;
						const frameCount = decoder.tracks.selectedTrack.frameCount;
						image = (await decoder.decode({ frameIndex: frameCount - 1 })).image;
					}
					const canvas = new OffscreenCanvas(width, height);
					const context = canvas.getContext('2d');
					context.imageSmoothingQuality = 'high';
					context.drawImage(image, 0, 0, width, height);
					const blob = await canvas.convertToBlob({ type: 'image/webp', quality });
					const output = new Uint8Array(await blob.arrayBuffer());
					let binary = '';
					for (const byte of output) binary += String.fromCharCode(byte);
					return btoa(binary);
				},
				{
					base64: bytes.toString('base64'),
					type: entry.source.endsWith('.gif') ? 'image/gif' : 'image/png',
					width: entry.width,
					height: entry.height,
					frame: entry.frame,
					quality: QUALITY,
				},
			);
			await writeFile(path.join(outputRoot, file), Buffer.from(encoded, 'base64'));
		}
		await writeFile(manifestPath, `${JSON.stringify(digests, null, '\t')}\n`);
	} finally {
		await browser.close();
	}
}
