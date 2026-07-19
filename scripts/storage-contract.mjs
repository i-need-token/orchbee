import { readFile, readdir } from 'node:fs/promises';
import { basename, join, relative, resolve } from 'node:path';

const REPOSITORY_ROOT = resolve(import.meta.dirname, '..');
const STORAGE_KEY_PATTERN =
	/^orchbee:[a-z][a-z0-9]*(?:-[a-z0-9]+)*:[a-z][a-z0-9]*(?:-[a-z0-9]+)*:v[1-9][0-9]*$/u;
const STORAGE_KEY_LITERAL_PATTERN = /(['"`])(orchbee:[^'"`\s]+)\1/gu;
const BROWSER_STORAGE_ACCESS_PATTERN =
	/\b(?:(?:window|globalThis)\.)?(?:localStorage|sessionStorage)\s*\./gu;

function normalize(path) {
	return path.split('\\').join('/');
}

function sourceLine(source, offset) {
	return source.slice(0, offset).split('\n').length;
}

function isApplicationEntry(path) {
	return /^apps\/[^/]+\/src\/composition\/entry\.tsx?$/u.test(normalize(path));
}

function isStorageOwnerModule(path) {
	const normalized = normalize(path);
	return normalized.includes('/src/') && basename(normalized).endsWith('-storage.ts');
}

export function validateStorageSource({ path, source }) {
	const diagnostics = [];

	for (const match of source.matchAll(BROWSER_STORAGE_ACCESS_PATTERN)) {
		if (isApplicationEntry(path)) continue;
		diagnostics.push({
			line: sourceLine(source, match.index),
			message:
				'Access browser storage only at an application composition entry and inject the capability into its owner.',
			path,
		});
	}

	for (const match of source.matchAll(STORAGE_KEY_LITERAL_PATTERN)) {
		const key = match[2];
		if (key === undefined) continue;
		const line = sourceLine(source, match.index);
		if (!isStorageOwnerModule(path)) {
			diagnostics.push({
				line,
				message: `Move storage key "${key}" into its owning *-storage.ts module.`,
				path,
			});
		}
		if (!STORAGE_KEY_PATTERN.test(key)) {
			diagnostics.push({
				line,
				message: `Storage key "${key}" must match orchbee:<owner>:<record>:v<schema>.`,
				path,
			});
		}
	}

	return diagnostics;
}

function isProductionTypeScript(path) {
	const normalized = normalize(path);
	return (
		/\.tsx?$/u.test(normalized) &&
		!normalized.includes('/__tests__/') &&
		!/(?:\.test|\.spec|\.stories)\.tsx?$/u.test(normalized)
	);
}

async function collectSourceFiles(directory) {
	const entries = await readdir(directory, { withFileTypes: true });
	const files = await Promise.all(
		entries.map(async (entry) => {
			const path = join(directory, entry.name);
			if (entry.isDirectory()) return collectSourceFiles(path);
			return isProductionTypeScript(path) ? [path] : [];
		}),
	);
	return files.flat();
}

async function main() {
	const files = (
		await Promise.all(
			['apps', 'packages'].map((directory) => collectSourceFiles(join(REPOSITORY_ROOT, directory))),
		)
	).flat();
	const diagnostics = (
		await Promise.all(
			files.map(async (path) =>
				validateStorageSource({
					path: normalize(relative(REPOSITORY_ROOT, path)),
					source: await readFile(path, 'utf8'),
				}),
			),
		)
	).flat();

	if (diagnostics.length > 0) {
		for (const diagnostic of diagnostics) {
			process.stderr.write(
				`${diagnostic.path}:${diagnostic.line}: ${diagnostic.message} See docs/frontend/browser-storage.md.\n`,
			);
		}
		process.exitCode = 1;
		return;
	}

	process.stdout.write(`Verified browser storage contract across ${files.length} source file(s)\n`);
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === import.meta.filename) {
	await main();
}
