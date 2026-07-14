import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const REPOSITORY_ROOT = process.cwd();
const GHOSTTY_ROOT = path.resolve(process.env.GHOSTTY_SOURCE ?? '../ghostty');
const MANIFEST_PATH = path.join(
	REPOSITORY_ROOT,
	'docs/terminal/ghostty-surface-test-provenance.md',
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

const zigRoots = ['src/renderer', 'src/font', 'src/terminal/search'];
const zigFiles = zigRoots.flatMap((root) => listFiles(path.join(GHOSTTY_ROOT, root), '.zig'));
zigFiles.push(
	path.join(GHOSTTY_ROOT, 'src/surface_mouse.zig'),
	path.join(GHOSTTY_ROOT, 'src/apprt/gtk/class/surface.zig'),
);

const declarations = zigFiles.flatMap(extractZigDeclarations);
const upstreamCounts = {
	zig: listFiles(path.join(GHOSTTY_ROOT, 'src'), '.zig').flatMap(extractZigDeclarations).length,
	swift: ['macos/Tests', 'macos/GhosttyUITests']
		.flatMap((root) => listFiles(path.join(GHOSTTY_ROOT, root), '.swift'))
		.flatMap(extractSwiftDeclarations).length,
};
for (const swiftFile of [
	'macos/Tests/Ghostty/Surface View/SurfaceView+SearchStateTests.swift',
	'macos/Tests/Ghostty/SurfaceViewAppKitTests.swift',
	'macos/GhosttyUITests/GhosttyMouseStateTests.swift',
]) {
	declarations.push(...extractSwiftDeclarations(path.join(GHOSTTY_ROOT, swiftFile)));
}

const manifestRows = parseManifest();
const failures = [];
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
	const relative = path.relative(GHOSTTY_ROOT, file);
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
	const relative = path.relative(GHOSTTY_ROOT, file);
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
