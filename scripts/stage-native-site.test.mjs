import assert from 'node:assert/strict';
import {
	cpSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	realpathSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

import { options, verifiedCapsuleDigest } from './stage-native-site.mjs';
import { assertWindowsFileSymlinkSupport } from './stage-native-windows.mjs';
import { verifyConptyBundle } from '../apps/terminal-host/scripts/windows-conpty.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const canonical = path.join(root, 'apps', 'desktop', 'resources', 'agent-observation-adapters');
const CAPSULE_STAGING_PREFIX = 'orchbee-capsule-staging-';

function removeOwnedTemporaryDirectory(directory, parent, prefix) {
	const normalizedParent = realpathSync(parent);
	const actual = realpathSync(directory);
	if (path.dirname(actual) !== normalizedParent || !path.basename(actual).startsWith(prefix))
		throw new Error('Refusing cleanup outside the owned staging directory');
	rmSync(actual, { recursive: true });
}

function ownedTemporaryDirectory(t, prefix) {
	const parent = realpathSync(tmpdir());
	const directory = mkdtempSync(path.join(parent, prefix));
	t.after(() => removeOwnedTemporaryDirectory(directory, parent, prefix));
	return directory;
}

test('Windows file-symlink preflight accepts a supported symlink operation and removes its probe', (t) => {
	const directory = ownedTemporaryDirectory(t, 'orchbee-symlink-supported-');
	assertWindowsFileSymlinkSupport(directory, (target, link, type) => {
		assert.equal(target, 'file-symlink-target');
		assert.equal(path.dirname(link), directory);
		assert.equal(type, 'file');
		if (process.platform !== 'win32') symlinkSync(target, link, type);
	});
	assert.deepEqual(readdirSync(directory), []);
});

test('Windows file-symlink preflight follows host permissions and removes its probe', (t) => {
	const directory = ownedTemporaryDirectory(t, 'orchbee-symlink-host-');
	try {
		assertWindowsFileSymlinkSupport(directory);
	} catch (error) {
		if (process.platform !== 'win32') throw error;
		assert.match(error.message, /Windows Developer Mode/u);
	}
	assert.deepEqual(readdirSync(directory), []);
});

test('Windows file-symlink preflight gives actionable guidance for permission failures and removes its probe', (t) => {
	const directory = ownedTemporaryDirectory(t, 'orchbee-symlink-denied-');
	for (const code of ['EPERM', 'EACCES']) {
		const cause = Object.assign(new Error('Access denied'), { code });
		assert.throws(
			() =>
				assertWindowsFileSymlinkSupport(directory, () => {
					throw cause;
				}),
			(error) => {
				assert.match(error.message, /Windows Developer Mode/u);
				assert.match(error.message, /elevated terminal/u);
				assert.match(error.message, /apps\/terminal-host\/README\.md#development/u);
				assert.equal(error.cause, cause);
				return true;
			},
		);
		assert.deepEqual(readdirSync(directory), []);
	}
});

test('macOS staging accepts a normalized absolute output and rejects WSL routing', () => {
	assert.deepEqual(options(['--output', '/private/tmp/orchbee-native-dev'], 'darwin'), {
		'--output': '/private/tmp/orchbee-native-dev',
	});
	assert.throws(
		() => options(['--wsl', 'Ubuntu', '--output', '/tmp/orchbee-native-dev'], 'darwin'),
		/--wsl is available only from a Windows host/u,
	);
	assert.throws(() => options(['--output', 'orchbee-native-dev'], 'darwin'), /absolute output/u);
});

test('Windows ConPTY staging rejects missing, altered and linked bundles before publication', (t) => {
	const parent = realpathSync(tmpdir());
	const directory = mkdtempSync(path.join(parent, 'orchbee-conpty-verification-'));
	t.after(() => {
		const actual = realpathSync(directory);
		assert.equal(path.dirname(actual), parent);
		assert.ok(path.basename(actual).startsWith('orchbee-conpty-verification-'));
		rmSync(actual, { recursive: true });
	});
	const bundle = path.join(directory, 'bundle');
	assert.throws(() => verifyConptyBundle(bundle));
	mkdirSync(bundle);
	assert.throws(() => verifyConptyBundle(bundle));
	writeFileSync(path.join(bundle, 'conpty.dll'), 'altered');
	assert.throws(() => verifyConptyBundle(bundle), /integrity mismatch/u);
	const linked = path.join(directory, 'linked');
	symlinkSync(bundle, linked, process.platform === 'win32' ? 'junction' : 'dir');
	assert.throws(() => verifyConptyBundle(linked), /Unsafe ConPTY bundle directory/u);
});

function stagedCopy(mutate) {
	const parent = realpathSync(tmpdir());
	const directory = mkdtempSync(path.join(parent, CAPSULE_STAGING_PREFIX));
	const copy = path.join(directory, 'agent-observation-adapters');
	try {
		cpSync(canonical, copy, { recursive: true });
		mutate(copy);
		return { directory, copy };
	} catch (error) {
		try {
			removeOwnedTemporaryDirectory(directory, parent, CAPSULE_STAGING_PREFIX);
		} catch (cleanupError) {
			throw new AggregateError(
				[error, cleanupError],
				`Staged capsule preparation failed and its temporary directory could not be removed: ${directory}`,
				{ cause: error },
			);
		}
		throw error;
	}
}

test('staged capsule copy removes its temporary directory when the mutator throws', () => {
	let directory;
	const cause = new Error('The capsule mutator failed');
	assert.throws(
		() =>
			stagedCopy((copy) => {
				directory = path.dirname(copy);
				throw cause;
			}),
		(error) => error === cause,
	);
	assert.equal(existsSync(directory), false);
});

test('canonical resource capsule verifies and reports its declared digest', () => {
	const declared = JSON.parse(
		readFileSync(path.join(canonical, 'capsule.json'), 'utf8'),
	).payloadSha256;
	assert.match(declared, /^[0-9a-f]{64}$/u);
	assert.equal(verifiedCapsuleDigest(canonical), declared);
});

test('a tampered payload byte is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		const file = path.join(target, 'bundle', 'package.json');
		writeFileSync(file, `${readFileSync(file, 'utf8')}\n`);
	});
	t.after(() =>
		removeOwnedTemporaryDirectory(directory, realpathSync(tmpdir()), CAPSULE_STAGING_PREFIX),
	);
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a capsule manifest drift is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		const manifest = JSON.parse(readFileSync(path.join(target, 'capsule.json'), 'utf8'));
		manifest.contractVersion = 4;
		writeFileSync(path.join(target, 'capsule.json'), JSON.stringify(manifest));
	});
	t.after(() =>
		removeOwnedTemporaryDirectory(directory, realpathSync(tmpdir()), CAPSULE_STAGING_PREFIX),
	);
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a capsule manifest with an unknown member is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		const manifest = JSON.parse(readFileSync(path.join(target, 'capsule.json'), 'utf8'));
		manifest.extra = true;
		writeFileSync(path.join(target, 'capsule.json'), JSON.stringify(manifest));
	});
	t.after(() =>
		removeOwnedTemporaryDirectory(directory, realpathSync(tmpdir()), CAPSULE_STAGING_PREFIX),
	);
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a missing required payload file is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		rmSync(path.join(target, 'bundle', 'package.json'));
	});
	t.after(() =>
		removeOwnedTemporaryDirectory(directory, realpathSync(tmpdir()), CAPSULE_STAGING_PREFIX),
	);
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a symbolic link inside the capsule is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		symlinkSync(
			path.join(target, 'bundle'),
			path.join(target, 'link'),
			process.platform === 'win32' ? 'junction' : 'dir',
		);
	});
	t.after(() =>
		removeOwnedTemporaryDirectory(directory, realpathSync(tmpdir()), CAPSULE_STAGING_PREFIX),
	);
	assert.equal(verifiedCapsuleDigest(copy), null);
});
