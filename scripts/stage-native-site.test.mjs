import assert from 'node:assert/strict';
import {
	cpSync,
	mkdirSync,
	mkdtempSync,
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
import { verifyConptyBundle } from '../apps/terminal-host/scripts/windows-conpty.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const canonical = path.join(root, 'apps', 'desktop', 'resources', 'agent-observation-adapters');

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
	const directory = mkdtempSync(path.join(tmpdir(), 'orchbee-capsule-staging-'));
	const copy = path.join(directory, 'agent-observation-adapters');
	cpSync(canonical, copy, { recursive: true });
	mutate(copy);
	return { directory, copy };
}

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
	t.after(() => rmSync(directory, { recursive: true, force: true }));
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a capsule manifest drift is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		const manifest = JSON.parse(readFileSync(path.join(target, 'capsule.json'), 'utf8'));
		manifest.contractVersion = 4;
		writeFileSync(path.join(target, 'capsule.json'), JSON.stringify(manifest));
	});
	t.after(() => rmSync(directory, { recursive: true, force: true }));
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a capsule manifest with an unknown member is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		const manifest = JSON.parse(readFileSync(path.join(target, 'capsule.json'), 'utf8'));
		manifest.extra = true;
		writeFileSync(path.join(target, 'capsule.json'), JSON.stringify(manifest));
	});
	t.after(() => rmSync(directory, { recursive: true, force: true }));
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a missing required payload file is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		rmSync(path.join(target, 'bundle', 'package.json'));
	});
	t.after(() => rmSync(directory, { recursive: true, force: true }));
	assert.equal(verifiedCapsuleDigest(copy), null);
});

test('a symbolic link inside the capsule is rejected', (t) => {
	const { directory, copy } = stagedCopy((target) => {
		symlinkSync('bundle', path.join(target, 'link'));
	});
	t.after(() => rmSync(directory, { recursive: true, force: true }));
	assert.equal(verifiedCapsuleDigest(copy), null);
});
