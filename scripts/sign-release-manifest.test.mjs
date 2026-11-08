import assert from 'node:assert/strict';
import { createHash, createPublicKey, generateKeyPairSync, verify } from 'node:crypto';
import {
	copyFileSync,
	existsSync,
	mkdtempSync,
	readFileSync,
	realpathSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
	loadKey,
	parseArguments,
	signManifest,
	signatureDocument,
} from './sign-release-manifest.mjs';

// The updater's own test reads these fixtures, so a signature produced here verifies there.
const fixtures = new URL('../apps/orchbee/src/cli/update/tests/fixtures/', import.meta.url);
const testKeyPem = readFileSync(new URL('test-only-signing-key.pem', fixtures), 'utf8');
const domain = Buffer.from('orchbee-release-manifest-v2\n', 'utf8');

function scratch(t) {
	const parent = realpathSync(tmpdir());
	const root = mkdtempSync(path.join(parent, 'orchbee-release-signing-test-'));
	t.after(() => {
		const actual = realpathSync(root);
		assert.equal(path.dirname(actual), parent);
		assert.ok(path.basename(actual).startsWith('orchbee-release-signing-test-'));
		rmSync(actual, { recursive: true });
	});
	return root;
}

function freshKey() {
	return generateKeyPairSync('ed25519').privateKey;
}

test('signing the fixture with the test key reproduces the document the updater verifies', (t) => {
	const root = scratch(t);
	const manifest = path.join(root, 'release.json');
	copyFileSync(new URL('release.json', fixtures), manifest);
	const destination = signManifest(manifest, [loadKey(testKeyPem)]);
	assert.equal(destination, manifest + '.sig');
	assert.deepEqual(readFileSync(destination), readFileSync(new URL('release.json.sig', fixtures)));
});

test('each signature covers the domain prefix and names its key', () => {
	const manifest = Buffer.from('{"schemaVersion":2}');
	const keys = [loadKey(testKeyPem), freshKey(), freshKey(), freshKey()];
	const document = JSON.parse(signatureDocument(manifest, keys));
	assert.deepEqual(Object.keys(document), ['signatures']);
	assert.equal(document.signatures.length, 4);
	assert.equal(document.signatures[0].keyId, 'b41b7cd3822bd0c2');
	for (const [index, entry] of document.signatures.entries()) {
		assert.deepEqual(Object.keys(entry), ['keyId', 'algorithm', 'signature']);
		assert.equal(entry.algorithm, 'ed25519');
		const publicKey = createPublicKey(keys[index]);
		const raw = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32);
		assert.equal(entry.keyId, createHash('sha256').update(raw).digest('hex').slice(0, 16));
		const signature = Buffer.from(entry.signature, 'base64');
		assert.equal(signature.length, 64);
		assert.ok(verify(null, Buffer.concat([domain, manifest]), publicKey, signature));
		assert.ok(!verify(null, manifest, publicKey, signature));
	}
});

test('signing refuses unusable keys, oversized inputs and existing signatures', (t) => {
	const root = scratch(t);
	const key = loadKey(testKeyPem);
	assert.throws(() => signatureDocument(Buffer.alloc(0), []), /1 to 4 keys/u);
	assert.throws(
		() => signatureDocument(Buffer.alloc(0), [key, freshKey(), freshKey(), freshKey(), freshKey()]),
		/1 to 4 keys/u,
	);
	assert.throws(() => signatureDocument(Buffer.alloc(0), [key, key]), /distinct/u);
	assert.throws(() => signatureDocument(Buffer.alloc(1024 * 1024 + 1), [key]), /1 MiB/u);
	const rsa = generateKeyPairSync('rsa', { modulusLength: 2048 }).privateKey;
	assert.throws(() => loadKey(rsa.export({ format: 'pem', type: 'pkcs8' })), /must be Ed25519/u);

	const manifest = path.join(root, 'release.json');
	assert.throws(() => signManifest('release.json', [key]), /absolute path/u);
	assert.throws(() => signManifest(path.join(root, 'manifest.json'), [key]), /absolute path/u);
	for (const text of ['{"schemaVersion":1,"product":"orchbee"}', '{"schemaVersion":2}', '[]']) {
		writeFileSync(manifest, text);
		assert.throws(() => signManifest(manifest, [key]), /schemaVersion 2/u, text);
	}
	writeFileSync(
		manifest,
		Buffer.concat([
			Buffer.from('{"schemaVersion":2,"product":"'),
			Buffer.from([0xff]),
			Buffer.from('"}'),
		]),
	);
	assert.throws(() => signManifest(manifest, [key]));
	assert.equal(existsSync(manifest + '.sig'), false);
	writeFileSync(manifest, '{"schemaVersion":2,"product":"orchbee"}');
	writeFileSync(manifest + '.sig', 'existing signature');
	assert.throws(() => signManifest(manifest, [key]), /EEXIST/u);
	assert.equal(readFileSync(manifest + '.sig', 'utf8'), 'existing signature');
});

test('keys come only from absolute files or named environment variables', (t) => {
	const root = scratch(t);
	const keyFile = path.join(root, 'key.pem');
	writeFileSync(keyFile, testKeyPem);
	const manifest = path.join(root, 'release.json');
	const parsed = parseArguments(
		[manifest, '--key-file', keyFile, '--key-env', 'ORCHBEE_RELEASE_SIGNING_KEY'],
		{ ORCHBEE_RELEASE_SIGNING_KEY: testKeyPem },
	);
	assert.equal(parsed.manifest, manifest);
	assert.equal(parsed.keys.length, 2);
	for (const argv of [
		[manifest],
		[manifest, '--key-file'],
		[manifest, '--key-file', 'key.pem'],
		[manifest, '--key-env', 'lowercase'],
		[manifest, '--key', keyFile],
	])
		assert.throws(() => parseArguments(argv, {}), /Usage/u, argv.join(' '));
	assert.throws(
		() => parseArguments([manifest, '--key-env', 'ORCHBEE_RELEASE_SIGNING_KEY'], {}),
		/holds no key/u,
	);
});
