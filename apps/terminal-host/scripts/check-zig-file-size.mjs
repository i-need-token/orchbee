import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const maximumProductionLines = 500;
const sourceRoot = fileURLToPath(new URL('../src/', import.meta.url));

// A generated production source may be listed only when its checked-in
// generator is the reviewed authority for the emitted file.
const generatedSourceAllowlist = new Set();

async function collectZigFiles(directory) {
	const entries = await readdir(directory, { withFileTypes: true });
	const nestedFiles = await Promise.all(
		entries.map(async (entry) => {
			const entryPath = path.join(directory, entry.name);
			if (entry.isDirectory()) return collectZigFiles(entryPath);
			return entry.isFile() && entry.name.endsWith('.zig') ? [entryPath] : [];
		}),
	);
	return nestedFiles.flat();
}

function isProductionSource(relativePath) {
	const segments = relativePath.split(path.sep);
	return (
		!segments.includes('__tests__') &&
		!relativePath.endsWith('_test.zig') &&
		!generatedSourceAllowlist.has(relativePath)
	);
}

function physicalLineCount(source) {
	if (source.length === 0) return 0;
	const newlineCount = source.match(/\n/g)?.length ?? 0;
	return newlineCount + (source.endsWith('\n') ? 0 : 1);
}

const zigFiles = await collectZigFiles(sourceRoot);
const productionFiles = zigFiles
	.map((filePath) => ({
		filePath,
		relativePath: path.relative(sourceRoot, filePath),
	}))
	.filter(({ relativePath }) => isProductionSource(relativePath));

const results = await Promise.all(
	productionFiles.map(async ({ filePath, relativePath }) => ({
		lineCount: physicalLineCount(await readFile(filePath, 'utf8')),
		relativePath,
	})),
);
const oversized = results
	.filter(({ lineCount }) => lineCount > maximumProductionLines)
	.sort((left, right) => right.lineCount - left.lineCount);

if (oversized.length > 0) {
	console.error(`Production Zig files must not exceed ${maximumProductionLines} physical lines:`);
	for (const { lineCount, relativePath } of oversized) {
		console.error(`  ${lineCount} src/${relativePath}`);
	}
	process.exitCode = 1;
} else {
	console.log(
		`Checked ${productionFiles.length} production Zig files; none exceed ${maximumProductionLines} lines.`,
	);
}
