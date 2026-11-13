// Derives the website's product pictures from the README captures in resources/readme/.
// `pnpm --filter @orchbee/website pictures` crops, scales and re-encodes them with the installed
// Google Chrome through playwright-core; `--check` verifies the tracked output without a browser and
// runs before every build, so a retaken capture cannot ship with stale website copies.
// The outputs only crop, scale and re-encode the captures: nothing is drawn, retouched or combined.
// Stills become WebP; recordings become muted VP9 WebM videos with a WebP poster of their last frame.
import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const sourceRoot = path.join(repositoryRoot, 'resources', 'readme');
// The website serves the pictures as they are from public/pictures/; the digests of the sources
// they were made from sit outside public/ so they are not published.
const websiteRoot = path.join(repositoryRoot, 'apps', 'website');
const outputRoot = path.join(websiteRoot, 'public', 'pictures');
const manifestPath = path.join(websiteRoot, 'picture-sources.json');
const STILL_QUALITY = 0.9;
// VP9 quantizer (0 best, 63 worst): low enough that terminal text stays sharp.
const VIDEO_QUANTIZER = 28;
// Key frames are costly on a busy terminal screen; the short loops need only their first one.
const KEY_FRAME_INTERVAL_MS = 60000;

// Each output names its README source, the source rectangle it keeps (the whole capture when
// absent), its output size, and for a recording whether it keeps every frame or only the last.
const PLAN = new Map([
	['hero-1600.webp', { source: 'hero.png', width: 1600, height: 882 }],
	['hero-800.webp', { source: 'hero.png', width: 800, height: 441 }],
	// The two Agent Panels, for narrow screens where the whole window is unreadable.
	[
		'hero-agents-784.webp',
		{ source: 'hero.png', crop: [784, 28, 784, 830], width: 784, height: 830 },
	],
	// The sidebar alone: its Agent rows and their state glyphs.
	['hero-sidebar.webp', { source: 'hero.png', crop: [0, 0, 256, 264], width: 256, height: 264 }],
	['desktop-1600.webp', { source: 'desktop.png', width: 1600, height: 888 }],
	['desktop-800.webp', { source: 'desktop.png', width: 800, height: 444 }],
	[
		'desktop-left-808.webp',
		{ source: 'desktop.png', crop: [0, 0, 808, 444], width: 808, height: 444 },
	],
	['multiplex.webm', { source: 'multiplex.gif', width: 1100, height: 324, frames: 'all' }],
	['multiplex-still.webp', { source: 'multiplex.gif', width: 1100, height: 324, frames: 'last' }],
	// Each half of the multiplex recording on its own, stacked on narrow screens.
	[
		'multiplex-tui.webm',
		{ source: 'multiplex.gif', crop: [0, 0, 546, 324], width: 546, height: 324, frames: 'all' },
	],
	[
		'multiplex-tui-still.webp',
		{ source: 'multiplex.gif', crop: [0, 0, 546, 324], width: 546, height: 324, frames: 'last' },
	],
	[
		'multiplex-desktop.webm',
		{ source: 'multiplex.gif', crop: [550, 0, 550, 324], width: 550, height: 324, frames: 'all' },
	],
	[
		'multiplex-desktop-still.webp',
		{ source: 'multiplex.gif', crop: [550, 0, 550, 324], width: 550, height: 324, frames: 'last' },
	],
	['agents-cli.webm', { source: 'agents-cli.gif', width: 1100, height: 605, frames: 'all' }],
	['agents-cli-still.webp', { source: 'agents-cli.gif', width: 1100, height: 605, frames: 'last' }],
	// The Command Prompt Panel that runs the commands, for narrow screens.
	[
		'agents-cli-shell.webm',
		{ source: 'agents-cli.gif', crop: [164, 0, 472, 605], width: 472, height: 605, frames: 'all' },
	],
	[
		'agents-cli-shell-still.webp',
		{ source: 'agents-cli.gif', crop: [164, 0, 472, 605], width: 472, height: 605, frames: 'last' },
	],
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

// EBML, the container syntax of WebM: element IDs carry their own length marker; sizes are
// variable-length integers.
const EBML = {
	header: [0x1a, 0x45, 0xdf, 0xa3],
	version: [0x42, 0x86],
	readVersion: [0x42, 0xf7],
	maxIdLength: [0x42, 0xf2],
	maxSizeLength: [0x42, 0xf3],
	docType: [0x42, 0x82],
	docTypeVersion: [0x42, 0x87],
	docTypeReadVersion: [0x42, 0x85],
	segment: [0x18, 0x53, 0x80, 0x67],
	info: [0x15, 0x49, 0xa9, 0x66],
	timecodeScale: [0x2a, 0xd7, 0xb1],
	duration: [0x44, 0x89],
	muxingApp: [0x4d, 0x80],
	writingApp: [0x57, 0x41],
	tracks: [0x16, 0x54, 0xae, 0x6b],
	trackEntry: [0xae],
	trackNumber: [0xd7],
	trackUid: [0x73, 0xc5],
	trackType: [0x83],
	flagLacing: [0x9c],
	codecId: [0x86],
	video: [0xe0],
	pixelWidth: [0xb0],
	pixelHeight: [0xba],
	cues: [0x1c, 0x53, 0xbb, 0x6b],
	cuePoint: [0xbb],
	cueTime: [0xb3],
	cueTrackPositions: [0xb7],
	cueTrack: [0xf7],
	cueClusterPosition: [0xf1],
	cluster: [0x1f, 0x43, 0xb6, 0x75],
	timecode: [0xe7],
	simpleBlock: [0xa3],
};
const MASTER_IDS = new Set(
	[EBML.segment, EBML.tracks, EBML.trackEntry, EBML.video].map((id) =>
		Buffer.from(id).toString('hex'),
	),
);

function vintSize(size) {
	for (let length = 1; length <= 8; length += 1) {
		if (size < 2 ** (7 * length) - 1) {
			const bytes = Buffer.alloc(length);
			let rest = size;
			for (let index = length - 1; index >= 0; index -= 1) {
				bytes[index] = rest % 256;
				rest = Math.floor(rest / 256);
			}
			bytes[0] |= 0x80 >> (length - 1);
			return bytes;
		}
	}
	throw new Error(`EBML element too large: ${size}`);
}

function element(id, payload) {
	return Buffer.concat([Buffer.from(id), vintSize(payload.length), payload]);
}

function unsignedElement(id, value) {
	const bytes = [];
	let rest = value;
	do {
		bytes.unshift(rest % 256);
		rest = Math.floor(rest / 256);
	} while (rest > 0);
	return element(id, Buffer.from(bytes));
}

function floatElement(id, value) {
	const payload = Buffer.alloc(8);
	payload.writeDoubleBE(value);
	return element(id, payload);
}

function stringElement(id, value) {
	return element(id, Buffer.from(value, 'ascii'));
}

// Writes a WebM file of one VP9 track. Each cluster starts at a key frame, so block timecodes stay
// within the 16-bit offset from their cluster.
function webm(width, height, durationMs, chunks) {
	const header = element(
		EBML.header,
		Buffer.concat([
			unsignedElement(EBML.version, 1),
			unsignedElement(EBML.readVersion, 1),
			unsignedElement(EBML.maxIdLength, 4),
			unsignedElement(EBML.maxSizeLength, 8),
			stringElement(EBML.docType, 'webm'),
			unsignedElement(EBML.docTypeVersion, 4),
			unsignedElement(EBML.docTypeReadVersion, 2),
		]),
	);
	const info = element(
		EBML.info,
		Buffer.concat([
			unsignedElement(EBML.timecodeScale, 1_000_000),
			floatElement(EBML.duration, durationMs),
			stringElement(EBML.muxingApp, 'generate-website-pictures'),
			stringElement(EBML.writingApp, 'generate-website-pictures'),
		]),
	);
	const tracks = element(
		EBML.tracks,
		element(
			EBML.trackEntry,
			Buffer.concat([
				unsignedElement(EBML.trackNumber, 1),
				unsignedElement(EBML.trackUid, 1),
				unsignedElement(EBML.trackType, 1),
				unsignedElement(EBML.flagLacing, 0),
				stringElement(EBML.codecId, 'V_VP9'),
				element(
					EBML.video,
					Buffer.concat([
						unsignedElement(EBML.pixelWidth, width),
						unsignedElement(EBML.pixelHeight, height),
					]),
				),
			]),
		),
	);
	const clusters = [];
	let current;
	for (const chunk of chunks) {
		if (chunk.key || current === undefined) {
			current = { start: chunk.timestampMs, blocks: [] };
			clusters.push(current);
		}
		const block = Buffer.alloc(4);
		block[0] = 0x81;
		block.writeInt16BE(chunk.timestampMs - current.start, 1);
		block[3] = chunk.key ? 0x80 : 0;
		current.blocks.push(element(EBML.simpleBlock, Buffer.concat([block, chunk.data])));
	}
	const body = clusters.map((cluster) =>
		element(
			EBML.cluster,
			Buffer.concat([unsignedElement(EBML.timecode, cluster.start), ...cluster.blocks]),
		),
	);
	// Cues let a player seek, and loop, without reading the whole file. They precede the clusters,
	// so each cluster position is written at a fixed width before the cues' own size is known.
	const cues = (positions) =>
		element(
			EBML.cues,
			Buffer.concat(
				clusters.map((cluster, index) => {
					const position = Buffer.alloc(8);
					position.writeBigUInt64BE(BigInt(positions[index]));
					return element(
						EBML.cuePoint,
						Buffer.concat([
							unsignedElement(EBML.cueTime, cluster.start),
							element(
								EBML.cueTrackPositions,
								Buffer.concat([
									unsignedElement(EBML.cueTrack, 1),
									element(EBML.cueClusterPosition, position),
								]),
							),
						]),
					);
				}),
			),
		);
	const leading = info.length + tracks.length + cues(clusters.map(() => 0)).length;
	const positions = [];
	let offset = leading;
	for (const cluster of body) {
		positions.push(offset);
		offset += cluster.length;
	}
	return Buffer.concat([
		header,
		element(EBML.segment, Buffer.concat([info, tracks, cues(positions), ...body])),
	]);
}

function readVint(buffer, offset, keepMarker) {
	const first = buffer[offset];
	let length = 1;
	while (length <= 8 && (first & (0x80 >> (length - 1))) === 0) length += 1;
	let value = keepMarker ? first : first & (0xff >> length);
	for (let index = 1; index < length; index += 1) value = value * 256 + buffer[offset + index];
	return { value, length };
}

// Reads the pixel size of the first video track of a WebM file.
function webmSize(buffer) {
	const size = {};
	const walk = (start, end) => {
		for (let offset = start; offset < end;) {
			const id = readVint(buffer, offset, true);
			const idHex = buffer.subarray(offset, offset + id.length).toString('hex');
			const length = readVint(buffer, offset + id.length, false);
			const data = offset + id.length + length.length;
			if (MASTER_IDS.has(idHex)) walk(data, data + length.value);
			if (idHex === 'b0' || idHex === 'ba') {
				size[idHex] = buffer.readUIntBE(data, length.value);
			}
			offset = data + length.value;
		}
	};
	if (buffer.subarray(0, 4).toString('hex') !== '1a45dfa3') return null;
	walk(0, buffer.length);
	return size['b0'] === undefined ? null : [size['b0'], size['ba']];
}

// Runs in the page: decodes the source, crops and scales it, and returns a WebP still, or for a
// recording that keeps every frame, its VP9 chunks with their timestamps.
async function encodeInPage({
	base64,
	type,
	crop,
	width,
	height,
	frames,
	quality,
	quantizer,
	keyEvery,
}) {
	const data = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
	const toBase64 = (bytes) => {
		let binary = '';
		for (const byte of bytes) binary += String.fromCharCode(byte);
		return btoa(binary);
	};
	const canvas = new OffscreenCanvas(width, height);
	const context = canvas.getContext('2d', { alpha: false, willReadFrequently: true });
	context.imageSmoothingQuality = 'high';
	const draw = (image) => {
		const [sx, sy, sw, sh] = crop ?? [
			0,
			0,
			image.displayWidth ?? image.width,
			image.displayHeight ?? image.height,
		];
		context.drawImage(image, sx, sy, sw, sh, 0, 0, width, height);
	};
	const still = async () =>
		toBase64(
			new Uint8Array(
				await (await canvas.convertToBlob({ type: 'image/webp', quality })).arrayBuffer(),
			),
		);
	if (frames === undefined) {
		draw(await createImageBitmap(new Blob([data], { type })));
		return { still: await still() };
	}
	const decoder = new ImageDecoder({ data, type });
	await decoder.tracks.ready;
	await decoder.completed;
	const frameCount = decoder.tracks.selectedTrack.frameCount;
	if (frames === 'last') {
		draw((await decoder.decode({ frameIndex: frameCount - 1 })).image);
		return { still: await still() };
	}
	const chunks = [];
	let failure;
	const encoder = new VideoEncoder({
		output: (chunk) => {
			const bytes = new Uint8Array(chunk.byteLength);
			chunk.copyTo(bytes);
			chunks.push({
				timestampMs: Math.round(chunk.timestamp / 1000),
				key: chunk.type === 'key',
				data: toBase64(bytes),
			});
		},
		error: (error) => {
			failure = String(error);
		},
	});
	encoder.configure({
		codec: 'vp09.00.10.08',
		width,
		height,
		bitrateMode: 'quantizer',
		latencyMode: 'quality',
	});
	let previous;
	let timestamp = 0;
	let lastKey = -Infinity;
	for (let index = 0; index < frameCount; index += 1) {
		const { image } = await decoder.decode({ frameIndex: index });
		const duration = image.duration / 1000;
		draw(image);
		image.close();
		const pixels = new Uint32Array(context.getImageData(0, 0, width, height).data.buffer);
		const same =
			previous !== undefined && pixels.every((value, offset) => value === previous[offset]);
		if (!same) {
			const keyFrame = timestamp - lastKey >= keyEvery;
			if (keyFrame) lastKey = timestamp;
			const frame = new VideoFrame(canvas, { timestamp: timestamp * 1000 });
			encoder.encode(frame, { keyFrame, vp9: { quantizer } });
			frame.close();
			previous = pixels;
		}
		timestamp += duration;
	}
	await encoder.flush();
	encoder.close();
	if (failure !== undefined) throw new Error(failure);
	return { chunks, durationMs: Math.round(timestamp) };
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
		const bytes = await readFile(path.join(outputRoot, file));
		const size = file.endsWith('.webm') ? webmSize(bytes) : webpSize(bytes);
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
		// ImageDecoder and VideoEncoder exist only in a secure context, so the page is served from a
		// fulfilled HTTPS route that never reaches the network.
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
			const encoded = await page.evaluate(encodeInPage, {
				base64: bytes.toString('base64'),
				type: entry.source.endsWith('.gif') ? 'image/gif' : 'image/png',
				crop: entry.crop,
				width: entry.width,
				height: entry.height,
				frames: entry.frames,
				quality: STILL_QUALITY,
				quantizer: VIDEO_QUANTIZER,
				keyEvery: KEY_FRAME_INTERVAL_MS,
			});
			const output =
				encoded.still === undefined
					? webm(
							entry.width,
							entry.height,
							encoded.durationMs,
							encoded.chunks.map((chunk) => ({
								...chunk,
								data: Buffer.from(chunk.data, 'base64'),
							})),
						)
					: Buffer.from(encoded.still, 'base64');
			await writeFile(path.join(outputRoot, file), output);
		}
		await writeFile(manifestPath, `${JSON.stringify(digests, null, '\t')}\n`);
	} finally {
		await browser.close();
	}
}
