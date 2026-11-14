import assert from 'node:assert/strict';
import {
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
	FEED_DOMAIN,
	keyTable,
	loadKey,
	signatureDocument,
	signFeed,
} from './sign-release-manifest.mjs';
import { planWebsiteRelease } from './website-release.mjs';

// The shared feed fixture, signed with the public test key; TEST_KEYS stands in for RELEASE_KEYS.
const fixtures = new URL('../apps/orchbee/src/cli/update/tests/fixtures/', import.meta.url);
const feed = readFileSync(new URL('feed.json.signed', fixtures));
const signature = readFileSync(new URL('feed.json.sig', fixtures));
const table = keyTable(
	readFileSync(new URL('../apps/orchbee/src/cli/update/signature.rs', import.meta.url), 'utf8'),
	'TEST_KEYS',
);
const release = readFileSync(
	new URL('../apps/website/src/website/release.ts', import.meta.url),
	'utf8',
);
const good = {
	version: '0.1.0',
	feed,
	signature,
	table,
	archive: { size: 27000000, sha256: '9'.repeat(64) },
	installerPublic: true,
	release,
};

test('a verified feed and a matching archive move the channel and the download button together', () => {
	const plan = planWebsiteRelease(good);
	assert.deepEqual(plan.problems, []);
	assert.deepEqual(
		plan.writes.map((write) => write.path),
		[
			'apps/website/public/update/stable/windows-x86_64/feed.json.signed',
			'apps/website/public/update/stable/windows-x86_64/feed.json.sig',
			'apps/website/src/website/release.ts',
		],
	);
	assert.deepEqual(plan.writes[0].bytes, feed);
	assert.deepEqual(plan.writes[1].bytes, signature);
	const source = plan.writes[2].bytes.toString('utf8');
	assert.match(source, /version: '0\.1\.0'/u);
	assert.match(source, /releases\/download\/v0\.1\.0\/Orchbee-0\.1\.0-windows-x86_64-setup\.exe/u);
});

test('nothing is written when the signature or the archive check fails', () => {
	const refusals = [
		[{ signature: Buffer.from('{"signatures":[]}\n') }, /malformed/u],
		[{ table: ['c'.repeat(64)] }, /untrusted/u],
		[{ feed: Buffer.from(feed.toString('utf8').replace('fixture.1', 'fixture.2')) }, /invalid/u],
		[{ version: '0.1.1' }, /names version 0\.1\.0, not 0\.1\.1/u],
		[{ archive: { size: 26999999, sha256: '9'.repeat(64) } }, /26999999 bytes/u],
		[{ archive: { size: 27000000, sha256: '8'.repeat(64) } }, /SHA-256/u],
		[{ archive: undefined }, /not fetched/u],
		[{ installerPublic: false }, /setup\.exe is not public/u],
		[{ release: 'export const release = {};\n' }, /block to set/u],
		// A stand-in source never reaches the network, so it refuses an absolute archive address.
		[{ standIn: true }, /relative path/u],
	];
	for (const [change, expected] of refusals) {
		const plan = planWebsiteRelease({ ...good, ...change });
		assert.deepEqual(plan.writes, [], JSON.stringify(Object.keys(change)));
		assert.match(plan.problems.join('\n'), expected);
	}
});

// JSON.parse admits these spellings and the updater's reader refuses them, so neither the signing
// tool nor the website command may let one through.
test('a feed not in its written form is neither signed nor moves the channel', (t) => {
	const parent = realpathSync(tmpdir());
	const root = mkdtempSync(path.join(parent, 'orchbee-website-release-test-'));
	t.after(() => rmSync(root, { recursive: true }));
	const key = loadKey(readFileSync(new URL('test-only-signing-key.pem', fixtures), 'utf8'));
	const text = feed.toString('utf8');
	const spellings = {
		'byte-order mark': Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), feed]),
		'repeated member': Buffer.from(
			text.replace('"product":"orchbee",', '"product":"orchbee","product":"orchbee",'),
		),
		'1.0 for a number': Buffer.from(text.replace('"schemaVersion":1,', '"schemaVersion":1.0,')),
	};
	for (const [name, bytes] of Object.entries(spellings)) {
		assert.notDeepEqual(bytes, feed, name);
		const document = path.join(root, `${name.replaceAll(/\W/gu, '-')}.feed.json`);
		writeFileSync(document, bytes);
		assert.throws(() => signFeed(document, [key]), /written form/u, name);
		assert.equal(existsSync(document + '.sig'), false, name);
		const plan = planWebsiteRelease({
			...good,
			feed: bytes,
			signature: Buffer.from(signatureDocument(bytes, [key], FEED_DOMAIN)),
		});
		assert.deepEqual(plan.writes, [], name);
		assert.match(plan.problems.join('\n'), /written form/u, name);
	}
});
