import assert from 'node:assert/strict';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { after, describe, it } from 'node:test';
import { runReleaseVersion } from './release-version.mjs';

const REPOSITORY_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const NEXT = '0.1.0-beta.1';

// The repository files that the script reads, and the four of them that `set` writes.
const SOURCE_FILES = [
	'Cargo.toml',
	'Cargo.lock',
	'apps/orchbee/Cargo.toml',
	'apps/orchbee-launcher/Cargo.toml',
	'apps/orchbee-server/Cargo.toml',
	'apps/orchbee-worker/Cargo.toml',
	'crates/local-runtime/Cargo.toml',
	'apps/orchbee/crates/workbench-client/Cargo.toml',
	'apps/desktop/package.json',
	'apps/terminal-host/build.zig',
];
const WRITTEN_FILES = [
	'Cargo.toml',
	'Cargo.lock',
	'apps/desktop/package.json',
	'apps/terminal-host/build.zig',
];

const temporaryRoots = [];

after(() => {
	for (const root of temporaryRoots) rmSync(root, { recursive: true, force: true });
});

// A temporary directory holding copies of the source files as the repository has them now.
function copyTree() {
	const root = mkdtempSync(join(tmpdir(), 'release-version-'));
	temporaryRoots.push(root);
	for (const file of SOURCE_FILES) {
		mkdirSync(dirname(join(root, file)), { recursive: true });
		copyFileSync(join(REPOSITORY_ROOT, file), join(root, file));
	}
	return root;
}

// A copy in which `set` has written NEXT, so that every recorded place agrees.
function setTree() {
	const root = copyTree();
	assert.equal(runReleaseVersion(['set', NEXT], root).status, 0);
	return root;
}

function readFile(root, file) {
	return readFileSync(join(root, file), 'utf8');
}

// Replaces the one occurrence of `from` in a file with `to`.
function replaceOnce(root, file, from, to) {
	const text = readFile(root, file);
	assert.equal(text.split(from).length - 1, 1, `${file} holds the text exactly once`);
	writeFileSync(join(root, file), text.replace(from, to));
}

// Checks against NEXT and asserts that exactly one difference is reported, on the given path.
function assertOneDifference(root, path) {
	const result = runReleaseVersion(['check', `v${NEXT}`], root);
	assert.equal(result.status, 1);
	assert.equal(result.lines.length, 1, result.lines.join('\n'));
	assert.ok(result.lines[0].startsWith(`${path}:`), result.lines[0]);
}

// `set` changes one line in each recorded place: five in Cargo.lock, one in each other file.
function expectedChanges(file) {
	if (file === 'Cargo.lock') return 6;
	return WRITTEN_FILES.includes(file) ? 1 : 0;
}

describe('release-version', () => {
	it('reports agreement after set writes a version', () => {
		const root = setTree();
		const result = runReleaseVersion(['check', `v${NEXT}`], root);
		assert.equal(result.status, 0);
		assert.deepEqual(result.lines, []);
	});

	it('names the root workspace version when only it differs', () => {
		const root = setTree();
		replaceOnce(root, 'Cargo.toml', `version = "${NEXT}"`, 'version = "0.9.0"');
		assertOneDifference(root, 'Cargo.toml');
	});

	it('names a Rust program that carries its own version instead of inheriting it', () => {
		const root = setTree();
		replaceOnce(
			root,
			'apps/orchbee/Cargo.toml',
			'\nversion.workspace = true',
			`\nversion = "${NEXT}"`,
		);
		assertOneDifference(root, 'apps/orchbee/Cargo.toml');
	});

	it('names the Desktop package version when only it differs', () => {
		const root = setTree();
		replaceOnce(root, 'apps/desktop/package.json', `"version": "${NEXT}"`, '"version": "0.9.0"');
		assertOneDifference(root, 'apps/desktop/package.json');
	});

	it('names the Terminal Host default version when only it differs', () => {
		const root = setTree();
		replaceOnce(root, 'apps/terminal-host/build.zig', `"${NEXT}"`, '"0.9.0"');
		assertOneDifference(root, 'apps/terminal-host/build.zig');
	});

	it('names the Cargo.lock entry that differs when only that entry differs', () => {
		const root = setTree();
		const lock = readFile(root, 'Cargo.lock');
		const old = `version = "${NEXT}"`;
		const at = lock.indexOf(old, lock.indexOf('name = "orchbee-server"'));
		writeFileSync(
			join(root, 'Cargo.lock'),
			`${lock.slice(0, at)}version = "0.9.0"${lock.slice(at + old.length)}`,
		);
		const result = runReleaseVersion(['check', `v${NEXT}`], root);
		assert.equal(result.status, 1);
		assert.equal(result.lines.length, 1, result.lines.join('\n'));
		assert.ok(result.lines[0].startsWith('Cargo.lock:'), result.lines[0]);
		assert.ok(result.lines[0].includes('orchbee-server'), result.lines[0]);
	});

	it('exits 2 for malformed input and writes nothing', () => {
		const root = copyTree();
		const before = readFile(root, 'Cargo.toml');
		const malformed = [
			[],
			['set'],
			['set', '1.2'],
			['set', '1.2.3+build.1'],
			['set', `v${NEXT}`],
			['set', NEXT, 'extra'],
			['check'],
			['check', NEXT],
			['check', 'v1.2'],
			['publish', NEXT],
		];
		for (const argv of malformed) {
			const result = runReleaseVersion(argv, root);
			assert.equal(result.status, 2, JSON.stringify(argv));
			assert.equal(result.lines.length, 1, JSON.stringify(argv));
		}
		assert.equal(readFile(root, 'Cargo.toml'), before);
	});

	it('changes only the recorded version values when it sets a version', () => {
		const root = copyTree();
		const before = new Map(SOURCE_FILES.map((file) => [file, readFile(root, file)]));
		const previous = JSON.parse(before.get('apps/desktop/package.json')).version;
		assert.equal(runReleaseVersion(['set', NEXT], root).status, 0);
		for (const file of SOURCE_FILES) {
			const original = before.get(file).split('\n');
			const updated = readFile(root, file).split('\n');
			assert.equal(updated.length, original.length, `${file} keeps its lines`);
			let changed = 0;
			for (const [index, line] of original.entries()) {
				if (updated[index] === line) continue;
				changed += 1;
				assert.equal(line.replace(previous, NEXT), updated[index], `${file} line ${index + 1}`);
			}
			assert.equal(changed, expectedChanges(file), `${file} changes its recorded lines only`);
		}
	});
});
