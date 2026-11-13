import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
	mkdtempSync,
	mkdirSync,
	readFileSync,
	realpathSync,
	rmSync,
	statSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
	compareVersions,
	parsePointer,
	storeRoot,
} from '../apps/orchbee/npm/orchbee/lib/launcher.mjs';
import { packageNpm } from './package-npm.mjs';

const stem = 'ob-test-0123456789abcdef';

function scratch(t) {
	const parent = realpathSync(tmpdir());
	const root = mkdtempSync(path.join(parent, 'orchbee-npm-test-'));
	t.after(() => {
		const actual = realpathSync(root);
		assert.equal(path.dirname(actual), parent);
		assert.ok(path.basename(actual).startsWith('orchbee-npm-test-'));
		rmSync(actual, { recursive: true });
	});
	return root;
}

/** A release directory whose manifest lists `files`; the signature is not checked here. */
function release(root, name, platform, version, files) {
	const directory = path.join(root, name);
	const entries = Object.entries(files).map(([file, content]) => {
		const target = path.join(directory, ...file.split('/'));
		mkdirSync(path.dirname(target), { recursive: true });
		writeFileSync(target, content);
		const sha256 = createHash('sha256').update(content).digest('hex');
		return { path: file, url: file, sha256, size: Buffer.byteLength(content) };
	});
	const manifest = { schemaVersion: 2, product: 'orchbee', platform, version, files: entries };
	writeFileSync(path.join(directory, 'release.json'), JSON.stringify(manifest) + '\n');
	writeFileSync(path.join(directory, 'release.json.sig'), '{}\n');
	return directory;
}

function windowsFiles(programStem = stem) {
	return Object.fromEntries(
		['', '-server', '-worker', '-terminal-host']
			.map((suffix) => [`${programStem}${suffix}.exe`, `program${suffix}`])
			.concat([
				['conpty/conpty.dll', 'conpty'],
				['cohort.json', '{}'],
			]),
	);
}

const readJson = (file) => JSON.parse(readFileSync(file, 'utf8'));

test('the main package depends on exactly the sub-packages of its run, at its own version', (t) => {
	const root = scratch(t);
	const directory = release(root, 'win', 'windows-x86_64', '0.1.0-beta.1', windowsFiles());
	const output = path.join(root, 'out');
	const [main, sub] = packageNpm({ version: '0.1.0-beta.1', output, releases: [directory] });

	const carrier = readJson(path.join(main, 'package.json'));
	assert.equal(carrier.name, stem);
	assert.equal(carrier.version, '0.1.0-beta.1');
	assert.deepEqual(carrier.optionalDependencies, { [`@${stem}/cli-win32-x64`]: '0.1.0-beta.1' });
	assert.deepEqual(Object.keys(carrier.bin), [stem]);
	assert.equal(carrier.scripts, undefined);
	assert.equal(carrier.publishConfig.tag, 'next');

	const platform = readJson(path.join(sub, 'package.json'));
	assert.equal(platform.name, `@${stem}/cli-win32-x64`);
	assert.equal(platform.version, '0.1.0-beta.1');
	assert.deepEqual([platform.os, platform.cpu], [['win32'], ['x64']]);
	assert.equal(platform.bin, undefined);
	assert.equal(platform.scripts, undefined);
	// The release directory travels unchanged, both documents included.
	for (const file of ['release.json', 'release.json.sig', `${stem}.exe`, 'conpty/conpty.dll'])
		assert.deepEqual(
			readFileSync(path.join(sub, 'release', ...file.split('/'))),
			readFileSync(path.join(directory, ...file.split('/'))),
			file,
		);
});

test('a release version is tagged latest and the release stem names the release packages', (t) => {
	const root = scratch(t);
	const directory = release(root, 'win', 'windows-x86_64', '0.1.0', windowsFiles('orchbee'));
	const [main] = packageNpm({
		version: '0.1.0',
		output: path.join(root, 'out'),
		releases: [directory],
	});
	const carrier = readJson(path.join(main, 'package.json'));
	assert.equal(carrier.name, 'orchbee');
	assert.deepEqual(carrier.optionalDependencies, { '@orchbee/cli-win32-x64': '0.1.0' });
	assert.equal(carrier.publishConfig.tag, 'latest');
});

test('every package names the building repository, which npm provenance requires', (t) => {
	const root = scratch(t);
	const directory = release(root, 'win', 'windows-x86_64', '0.1.0', windowsFiles('orchbee'));
	const url = 'https://github.com/owner/orchbee';
	const folders = packageNpm({
		version: '0.1.0',
		output: path.join(root, 'out'),
		releases: [directory],
		repositoryUrl: url,
	});
	for (const folder of folders)
		assert.deepEqual(readJson(path.join(folder, 'package.json')).repository, {
			type: 'git',
			url: `git+${url}.git`,
		});
	assert.throws(
		() =>
			packageNpm({
				version: '0.1.0',
				output: path.join(root, 'other'),
				releases: [directory],
				repositoryUrl: 'github.com/owner/orchbee',
			}),
		/repository URL/u,
	);
});

test('packaging refuses releases it cannot carry whole and unchanged', (t) => {
	const root = scratch(t);
	const good = release(root, 'win', 'windows-x86_64', '0.1.0', windowsFiles());
	const output = (name) => path.join(root, name);
	assert.throws(
		() => packageNpm({ version: '0.1.1', output: output('a'), releases: [good] }),
		/not 0\.1\.1/u,
	);
	assert.throws(
		() => packageNpm({ version: '0.1.0', output: output('b'), releases: [good, good] }),
		/Two release directories/u,
	);
	const tampered = release(root, 'tampered', 'windows-x86_64', '0.1.0', windowsFiles());
	writeFileSync(path.join(tampered, `${stem}-worker.exe`), 'changed');
	assert.throws(
		() => packageNpm({ version: '0.1.0', output: output('c'), releases: [tampered] }),
		/does not match/u,
	);
	const unsigned = release(root, 'unsigned', 'windows-x86_64', '0.1.0', windowsFiles());
	rmSync(path.join(unsigned, 'release.json.sig'));
	assert.throws(
		() => packageNpm({ version: '0.1.0', output: output('d'), releases: [unsigned] }),
		/release\.json\.sig is missing/u,
	);
	const dropped = release(root, 'dropped', 'windows-x86_64', '0.1.0', {
		...windowsFiles(),
		'agent-observation-adapters/node_modules/x.js': 'x',
	});
	assert.throws(
		() => packageNpm({ version: '0.1.0', output: output('e'), releases: [dropped] }),
		/npm would leave out/u,
	);
	mkdirSync(output('existing'));
	assert.throws(
		() => packageNpm({ version: '0.1.0', output: output('existing'), releases: [good] }),
		/existing/u,
	);
});

test('macOS and Linux sub-packages keep their programs executable, and only off Windows', (t) => {
	const root = scratch(t);
	const files = Object.fromEntries(
		['', '-server', '-worker', '-terminal-host'].map((suffix) => [`${stem}${suffix}`, 'program']),
	);
	const directory = release(root, 'linux', 'linux-x86_64', '0.1.0', files);
	if (process.platform === 'win32') {
		assert.throws(
			() => packageNpm({ version: '0.1.0', output: path.join(root, 'out'), releases: [directory] }),
			/executable bit/u,
		);
		return;
	}
	const [, sub] = packageNpm({
		version: '0.1.0',
		output: path.join(root, 'out'),
		releases: [directory],
	});
	assert.equal(readJson(path.join(sub, 'package.json')).name, `@${stem}/cli-linux-x64`);
	for (const suffix of ['', '-server', '-worker', '-terminal-host'])
		assert.equal(statSync(path.join(sub, 'release', stem + suffix)).mode & 0o111, 0o111);
});

test('the launcher orders versions by SemVer precedence', () => {
	const ascending = [
		'0.1.0-alpha',
		'0.1.0-alpha.1',
		'0.1.0-alpha.beta',
		'0.1.0-beta',
		'0.1.0-beta.2',
		'0.1.0-beta.11',
		'0.1.0-rc.1',
		'0.1.0',
		'0.1.1',
		'0.2.0',
		'0.10.0',
		'1.0.0',
	];
	for (let index = 1; index < ascending.length; index++) {
		assert.ok(compareVersions(ascending[index - 1], ascending[index]) < 0, ascending[index]);
		assert.ok(compareVersions(ascending[index], ascending[index - 1]) > 0, ascending[index]);
	}
	assert.equal(compareVersions('0.1.0-beta.1', '0.1.0-beta.1'), 0);
});

test('the launcher reads only a format-1 pointer as an active version', () => {
	const digest = 'a'.repeat(64);
	const entry = (product) => ({ product, manifest: digest });
	assert.equal(parsePointer(JSON.stringify({ version: 1, active: entry('0.1.0') })), '0.1.0');
	assert.equal(
		parsePointer(
			JSON.stringify({
				version: 1,
				active: entry('0.1.0'),
				previous: entry('0.1.0-beta.3'),
				extensions: {},
			}),
		),
		'0.1.0',
	);
	for (const other of [
		'',
		'[]',
		JSON.stringify({ version: 2, active: entry('0.1.0') }),
		JSON.stringify({ version: 1, active: entry('../0.1.0') }),
		JSON.stringify({ version: 1, active: { product: '0.1.0', manifest: 'A'.repeat(64) } }),
		JSON.stringify({ version: 1, active: entry('0.1.0'), other: true }),
		JSON.stringify({ version: 1, active: entry('0.1.0'), extensions: [] }),
	])
		assert.equal(parsePointer(other), null, other);
});

test('the launcher reads the store root the install command writes', () => {
	const windows = { LOCALAPPDATA: 'C:\\Users\\a\\AppData\\Local' };
	assert.equal(
		storeRoot('orchbee', 'win32', windows),
		'C:\\Users\\a\\AppData\\Local\\Programs\\Orchbee',
	);
	assert.equal(
		storeRoot(stem, 'win32', windows),
		`C:\\Users\\a\\AppData\\Local\\Programs\\${stem}`,
	);
	assert.equal(
		storeRoot(stem, 'win32', { ...windows, ORCHBEE_INSTALL_DIR: 'D:\\store' }),
		'D:\\store',
	);
	assert.equal(storeRoot(stem, 'win32', { ...windows, ORCHBEE_INSTALL_DIR: 'relative' }), null);
	assert.equal(storeRoot('orchbee', 'linux', { HOME: '/home/a' }), '/home/a/.local/share/orchbee');
});
