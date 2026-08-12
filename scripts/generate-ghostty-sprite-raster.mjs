import crypto from 'node:crypto';
import childProcess from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import zlib from 'node:zlib';

const repositoryRoot = process.cwd();
const ghosttyRoot = path.resolve(process.env.GHOSTTY_SOURCE ?? '../ghostty');
const spriteRoot = path.join(ghosttyRoot, 'src/font/sprite');
const spriteFacePath = path.join(spriteRoot, 'Face.zig');
const outputPath = path.join(
	repositoryRoot,
	'packages/terminal/src/ghostty-sprite-raster.generated.ts',
);
const drawFiles = [
	...fs.readFileSync(spriteFacePath, 'utf8').matchAll(/@import\("draw\/([^"]+\.zig)"\)/gu),
]
	.map((match) => match[1])
	.filter((file) => file !== undefined && file !== 'special.zig')
	.map((file) => path.join(spriteRoot, 'draw', file));
if (drawFiles.length === 0) throw new Error('No Ghostty sprite draw files were found');
const rasterMetrics = [
	{ width: 9, height: 17, boxThickness: 1 },
	{ width: 11, height: 21, boxThickness: 2 },
	{ width: 12, height: 24, boxThickness: 3 },
	{ width: 18, height: 36, boxThickness: 4 },
];

const ranges = readRanges(drawFiles);
const sourceFiles = new Set([spriteFacePath, ...drawFiles]);
const rasterSources = rasterMetrics.map((metrics) => {
	const encoded = encodeRaster(metrics, ranges);
	for (const file of encoded.sourceFiles) sourceFiles.add(file);
	return encoded.raster;
});
const sourceHash = hashFiles([...sourceFiles].toSorted());
const generated = formatGenerated(generate(sourceHash, ranges, rasterSources));

if (process.argv.includes('--check')) {
	const current = fs.readFileSync(outputPath, 'utf8');
	if (current !== generated) {
		process.stderr.write(`${path.relative(repositoryRoot, outputPath)} is stale\n`);
		process.exitCode = 1;
	}
} else {
	fs.writeFileSync(outputPath, generated);
}

function readRanges(files) {
	const result = files.flatMap((file) => {
		const source = fs.readFileSync(file, 'utf8');
		return [...source.matchAll(/^pub fn draw([\da-fA-F]+)(?:_([\da-fA-F]+))?\(/gmu)].map(
			(match) => [Number.parseInt(match[1], 16), Number.parseInt(match[2] ?? match[1], 16)],
		);
	});
	result.sort((left, right) => left[0] - right[0]);
	for (let index = 1; index < result.length; index++) {
		const previous = result[index - 1];
		const current = result[index];
		if (previous === undefined || current === undefined || current[0] <= previous[1]) {
			throw new Error('Ghostty sprite ranges overlap');
		}
	}
	if (result.length === 0) throw new Error('No Ghostty sprite ranges were found');
	return result;
}

function encodeRaster(metrics, spriteRanges) {
	const offsets = [0];
	const runs = [];
	const decoded = new Map();
	const rasterSourceFiles = new Set();
	for (const [start, end] of spriteRanges) {
		for (let codepoint = start; codepoint <= end; codepoint++) {
			const page = codepoint & ~0xff;
			const source = path.join(
				spriteRoot,
				'testdata',
				`U+${page.toString(16).toUpperCase()}...U+${(page + 0xff).toString(16).toUpperCase()}-${metrics.width}x${metrics.height}+${metrics.boxThickness}.png`,
			);
			rasterSourceFiles.add(source);
			let image = decoded.get(source);
			if (image === undefined) {
				image = decodeGrayscalePng(source);
				decoded.set(source, image);
			}
			appendGlyphRuns(runs, image, codepoint - page, metrics);
			offsets.push(runs.length / 4);
		}
	}
	const offsetBytes = Buffer.alloc(offsets.length * 4);
	for (const [index, offset] of offsets.entries()) offsetBytes.writeUInt32LE(offset, index * 4);
	return {
		raster: {
			...metrics,
			paddingX: Math.floor(metrics.width / 4),
			paddingY: Math.floor(metrics.height / 4),
			offsets: offsetBytes.toString('base64'),
			runs: Buffer.from(runs).toString('base64'),
		},
		sourceFiles: rasterSourceFiles,
	};
}

function appendGlyphRuns(runs, image, pageIndex, metrics) {
	const paddingX = Math.floor(metrics.width / 4);
	const paddingY = Math.floor(metrics.height / 4);
	const strideX = metrics.width + 2 * paddingX;
	const strideY = metrics.height + 2 * paddingY;
	if (image.width !== strideX * 16 || image.height !== strideY * 16) {
		throw new Error('Unexpected Ghostty sprite atlas dimensions');
	}
	const originX = (pageIndex % 16) * strideX;
	const originY = Math.floor(pageIndex / 16) * strideY;
	for (let y = 0; y < strideY; y++) {
		const row = image.rows[originY + y];
		if (row === undefined) throw new Error('Missing Ghostty sprite raster row');
		let x = 0;
		while (x < strideX) {
			const alpha = row[originX + x] ?? 0;
			if (alpha === 0) {
				x++;
				continue;
			}
			let end = x + 1;
			while (end < strideX && row[originX + end] === alpha) end++;
			runs.push(x, y, end - x, alpha);
			x = end;
		}
	}
}

function decodeGrayscalePng(file) {
	const source = fs.readFileSync(file);
	if (!source.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
		throw new Error(`Invalid PNG signature: ${file}`);
	}
	let offset = 8;
	let width = 0;
	let height = 0;
	const data = [];
	while (offset < source.length) {
		const length = source.readUInt32BE(offset);
		const type = source.toString('ascii', offset + 4, offset + 8);
		const chunk = source.subarray(offset + 8, offset + 8 + length);
		offset += length + 12;
		if (type === 'IHDR') {
			width = chunk.readUInt32BE(0);
			height = chunk.readUInt32BE(4);
			if (
				chunk[8] !== 8 ||
				chunk[9] !== 0 ||
				chunk[10] !== 0 ||
				chunk[11] !== 0 ||
				chunk[12] !== 0
			) {
				throw new Error(`Unsupported Ghostty sprite PNG format: ${file}`);
			}
		} else if (type === 'IDAT') {
			data.push(chunk);
		}
	}
	if (width <= 0 || height <= 0) throw new Error(`Missing PNG dimensions: ${file}`);
	const inflated = zlib.inflateSync(Buffer.concat(data));
	const rows = [];
	let sourceOffset = 0;
	let previous = Buffer.alloc(width);
	for (let y = 0; y < height; y++) {
		const filter = inflated[sourceOffset++];
		const row = Buffer.from(inflated.subarray(sourceOffset, sourceOffset + width));
		sourceOffset += width;
		for (let x = 0; x < width; x++) {
			const left = x === 0 ? 0 : (row[x - 1] ?? 0);
			const above = previous[x] ?? 0;
			const upperLeft = x === 0 ? 0 : (previous[x - 1] ?? 0);
			const predictor = switchPngFilter(filter, left, above, upperLeft);
			row[x] = ((row[x] ?? 0) + predictor) & 0xff;
		}
		rows.push(row);
		previous = row;
	}
	return { width, height, rows };
}

function switchPngFilter(filter, left, above, upperLeft) {
	if (filter === 0) return 0;
	if (filter === 1) return left;
	if (filter === 2) return above;
	if (filter === 3) return Math.floor((left + above) / 2);
	if (filter === 4) return paeth(left, above, upperLeft);
	throw new Error(`Unsupported PNG filter: ${filter}`);
}

function paeth(left, above, upperLeft) {
	const estimate = left + above - upperLeft;
	const leftDistance = Math.abs(estimate - left);
	const aboveDistance = Math.abs(estimate - above);
	const upperLeftDistance = Math.abs(estimate - upperLeft);
	if (leftDistance <= aboveDistance && leftDistance <= upperLeftDistance) return left;
	return aboveDistance <= upperLeftDistance ? above : upperLeft;
}

function hashFiles(files) {
	const hash = crypto.createHash('sha256');
	for (const file of files) {
		hash.update(path.relative(ghosttyRoot, file));
		hash.update('\0');
		hash.update(fs.readFileSync(file));
		hash.update('\0');
	}
	return hash.digest('hex');
}

function generate(hash, spriteRanges, rasters) {
	const lines = [
		'/**',
		' * Generated from the pinned Ghostty sprite-face draw ranges and reference rasters.',
		' * Regenerate with `pnpm generate:ghostty-panel-data`; do not edit by hand.',
		` * Source SHA-256: ${hash}`,
		' */',
		'',
		'export interface GeneratedGhosttySpriteRaster {',
		'  readonly width: number;',
		'  readonly height: number;',
		'  readonly boxThickness: number;',
		'  readonly paddingX: number;',
		'  readonly paddingY: number;',
		'  readonly offsets: readonly string[];',
		'  readonly runs: readonly string[];',
		'}',
		'',
		'export const GHOSTTY_SPRITE_RANGES = [',
	];
	for (const [start, end] of spriteRanges) {
		lines.push(`  [${formatCodepoint(start)}, ${formatCodepoint(end)}],`);
	}
	lines.push('] as const;', '', 'export const GHOSTTY_SPRITE_RASTERS = [');
	for (const raster of rasters) {
		lines.push('  {');
		lines.push(`    width: ${raster.width},`);
		lines.push(`    height: ${raster.height},`);
		lines.push(`    boxThickness: ${raster.boxThickness},`);
		lines.push(`    paddingX: ${raster.paddingX},`);
		lines.push(`    paddingY: ${raster.paddingY},`);
		lines.push('    offsets: [');
		for (const chunk of chunks(raster.offsets)) lines.push(`      '${chunk}',`);
		lines.push('    ],');
		lines.push('    runs: [');
		for (const chunk of chunks(raster.runs)) lines.push(`      '${chunk}',`);
		lines.push('    ],');
		lines.push('  },');
	}
	lines.push('] as const satisfies readonly GeneratedGhosttySpriteRaster[];', '');
	return lines.join('\n');
}

function chunks(value) {
	return value.match(/.{1,100}/gu) ?? [];
}

function formatCodepoint(value) {
	return `0x${value.toString(16)}`;
}

function formatGenerated(input) {
	const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'orchbee-ghostty-sprite-'));
	const temporaryFile = path.join(temporaryDirectory, 'ghostty-sprite-raster.generated.ts');
	try {
		fs.writeFileSync(temporaryFile, input);
		const result = childProcess.spawnSync('pnpm', ['exec', 'oxfmt', temporaryFile], {
			cwd: repositoryRoot,
			encoding: 'utf8',
		});
		if (result.status !== 0) {
			throw new Error(result.stderr || result.stdout || 'oxfmt failed');
		}
		return fs.readFileSync(temporaryFile, 'utf8');
	} finally {
		fs.rmSync(temporaryDirectory, { recursive: true, force: true });
	}
}
