import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

import { assertPinnedGhosttyCheckout } from './ghostty-source.mjs';

const REPOSITORY_ROOT = process.cwd();
const GHOSTTY_ROOT = path.resolve(process.env.GHOSTTY_SOURCE ?? '../ghostty');
assertPinnedGhosttyCheckout({ ghosttyRoot: GHOSTTY_ROOT, repositoryRoot: REPOSITORY_ROOT });
const MANIFEST_PATH = path.join(
	REPOSITORY_ROOT,
	'packages/terminal/docs/ghostty-surface-test-provenance.md',
);
const ZIG_TEST = /^\s*test(?:\s+(?:"([^"]*)"|([A-Za-z_][A-Za-z0-9_]*)))?\s*\{/u;
const MANIFEST_ROW =
	/^- `([^`]+)` — (Covered|Gap|Browser product|Upstream internal|Application layer)(?: \([^)]*\))?: (.*)$/u;
const STATUS_NAMES = [
	'Covered',
	'Gap',
	'Browser product',
	'Upstream internal',
	'Application layer',
];
const PROVISIONED_FONT_ASSETS = [
	{
		path: 'apps/web/src/workbench-client/projected-terminal-renderer/fonts/JetBrainsMono-Variable.ttf',
		sha256: '662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016',
	},
	{
		path: 'apps/web/src/workbench-client/projected-terminal-renderer/fonts/JetBrainsMono-VariableItalic.ttf',
		sha256: 'f115aaa12113718c02ce72864fe6823b87241bc23d3e44cf1220155f861063f2',
	},
	{
		path: 'apps/web/src/workbench-client/projected-terminal-renderer/fonts/SymbolsNerdFont-Regular.ttf',
		sha256: '71db104aa66567d0efe0b98758f9dfc1895573a453fe85fb53d1c38544a55106',
	},
	{
		path: 'apps/web/src/workbench-client/projected-terminal-renderer/fonts/JetBrainsMono-OFL.txt',
		sha256: '60d55f23c6ce05a81099a762cb67ca2c9b6ea251c7912720998b4c89ebfd4faa',
	},
	{
		path: 'apps/web/src/workbench-client/projected-terminal-renderer/fonts/SymbolsNerdFont-LICENSE.txt',
		sha256: '84a7a98c82140fb12c37fe42b93805baa16024cb3e5acc599b7ffe612c55d847',
	},
];
const GHOSTTY_FONT_PINS = ['JetBrainsMono-2.304.tar.gz', 'NerdFontsSymbolsOnly-3.4.0.tar.gz'];

const zigRoots = ['src/renderer', 'src/font'];
const zigFiles = zigRoots.flatMap((root) => listFiles(path.join(GHOSTTY_ROOT, root), '.zig'));

const declarations = zigFiles.flatMap(extractZigDeclarations);
const upstreamCounts = {
	zig: listFiles(path.join(GHOSTTY_ROOT, 'src'), '.zig').flatMap(extractZigDeclarations).length,
	swift: ['macos/Tests', 'macos/GhosttyUITests']
		.flatMap((root) => listFiles(path.join(GHOSTTY_ROOT, root), '.swift'))
		.flatMap(extractSwiftDeclarations).length,
};
const manifestRows = parseManifest();
const failures = [];
for (const asset of PROVISIONED_FONT_ASSETS) {
	const assetPath = path.join(REPOSITORY_ROOT, asset.path);
	if (!fs.existsSync(assetPath)) {
		failures.push(`${asset.path} is missing`);
		continue;
	}
	const actual = createHash('sha256').update(fs.readFileSync(assetPath)).digest('hex');
	if (actual !== asset.sha256) failures.push(`${asset.path} has unexpected SHA-256 ${actual}`);
}
const ghosttyBuildManifest = fs.readFileSync(path.join(GHOSTTY_ROOT, 'build.zig.zon'), 'utf8');
for (const pin of GHOSTTY_FONT_PINS) {
	if (!ghosttyBuildManifest.includes(pin)) failures.push(`Ghostty no longer pins ${pin}`);
}
const counts = Object.fromEntries(STATUS_NAMES.map((status) => [status, 0]));
const expectedByFile = new Map();
for (const declaration of declarations) {
	const expected = expectedByFile.get(declaration.file) ?? new Set();
	expected.add(declaration.name);
	expectedByFile.set(declaration.file, expected);
	const matches = manifestRows.filter(
		(row) => row.file === declaration.file && row.tests.includes(declaration.name),
	);
	if (matches.length !== 1) {
		failures.push(
			`${declaration.file}::${declaration.name} has ${matches.length} manifest mappings`,
		);
		continue;
	}
	const status = matches[0]?.status;
	if (status) counts[status] = (counts[status] ?? 0) + 1;
}

for (const row of manifestRows) {
	const expected = expectedByFile.get(row.file);
	if (!expected) {
		failures.push(`${row.file} is outside the audited source set`);
		continue;
	}
	for (const test of row.tests) {
		if (!expected.has(test)) failures.push(`${row.file}::${test} is a stale manifest mapping`);
	}
}

if (failures.length > 0) {
	process.stderr.write(`${failures.join('\n')}\n`);
	process.exitCode = 1;
} else {
	process.stdout.write(
		`${JSON.stringify({ upstream: upstreamCounts, declarations: declarations.length, counts }, null, 2)}\n`,
	);
}

function listFiles(directory, extension) {
	return fs
		.readdirSync(directory, { withFileTypes: true })
		.flatMap((entry) => {
			const entryPath = path.join(directory, entry.name);
			if (entry.isDirectory()) return listFiles(entryPath, extension);
			return entry.name.endsWith(extension) ? [entryPath] : [];
		})
		.toSorted();
}

function extractZigDeclarations(file) {
	const relative = path.relative(GHOSTTY_ROOT, file).split(path.sep).join('/');
	const fileDeclarations = [];
	for (const [index, line] of readLines(file).entries()) {
		const match = line.match(ZIG_TEST);
		if (!match) continue;
		fileDeclarations.push({
			file: relative,
			name: match[1] ?? match[2] ?? `<unnamed@${index + 1}>`,
		});
	}
	return fileDeclarations;
}

function extractSwiftDeclarations(file) {
	const relative = path.relative(GHOSTTY_ROOT, file).split(path.sep).join('/');
	const lines = readLines(file);
	const fileDeclarations = [];
	const capturedLines = new Set();
	for (let index = 0; index < lines.length; index++) {
		if (!lines[index]?.includes('@Test')) continue;
		for (let functionIndex = index; functionIndex < lines.length; functionIndex++) {
			const functionName = lines[functionIndex]?.match(/\bfunc\s+([A-Za-z_][A-Za-z0-9_]*)/u)?.[1];
			if (!functionName) continue;
			fileDeclarations.push({ file: relative, name: functionName });
			capturedLines.add(functionIndex);
			index = functionIndex;
			break;
		}
	}
	for (const [index, line] of lines.entries()) {
		if (capturedLines.has(index)) continue;
		const functionName = line.match(/\bfunc\s+(test[A-Z_][A-Za-z0-9_]*)/u)?.[1];
		if (functionName) fileDeclarations.push({ file: relative, name: functionName });
	}
	return fileDeclarations;
}

function parseManifest() {
	const rows = [];
	for (const line of readLines(MANIFEST_PATH)) {
		const match = line.match(MANIFEST_ROW);
		if (!match) continue;
		rows.push({
			file: match[1],
			status: match[2],
			tests: [...match[3].matchAll(/`([^`]+)`/gu)].map((testMatch) => testMatch[1]),
		});
	}
	return rows;
}

function readLines(file) {
	return fs.readFileSync(file, 'utf8').split(/\r?\n/u);
}
