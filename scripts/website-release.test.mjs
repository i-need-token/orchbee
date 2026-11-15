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
import { feedDocument } from './release-feed.mjs';
import { planWebsiteRelease, publishedPlatforms } from './website-release.mjs';

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
const testKey = loadKey(readFileSync(new URL('test-only-signing-key.pem', fixtures), 'utf8'));
const windows = {
	platform: 'windows-x86_64',
	feed,
	signature,
	archive: { size: 27000000, sha256: '9'.repeat(64) },
	installerPublic: true,
};
const good = { version: '0.1.0', rows: [windows], table, release };

/** A signed macOS row of version 0.1.0. */
function macosRow(cpu) {
	const platform = `macos-${cpu}`;
	const bytes = feedDocument({
		channel: 'stable',
		platform,
		version: '0.1.0',
		buildId: 'fixture.1',
		manifestSha256: '7'.repeat(64),
		archive: {
			url: `https://releases.example.test/orchbee/v0.1.0/orchbee-0.1.0-${platform}.tar.gz`,
			size: 21000000,
			sha256: '6'.repeat(64),
		},
	});
	return {
		platform,
		feed: bytes,
		signature: Buffer.from(signatureDocument(bytes, [testKey], FEED_DOMAIN)),
		archive: { size: 21000000, sha256: '6'.repeat(64) },
		installerPublic: true,
	};
}

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
	assert.doesNotMatch(source, /'macos-\w+': `/u);
});

test('every published platform row moves its own feed and offers its own download', () => {
	const rows = [windows, macosRow('aarch64'), macosRow('x86_64')];
	const plan = planWebsiteRelease({ ...good, rows });
	assert.deepEqual(plan.problems, []);
	for (const row of rows) {
		const folder = `apps/website/public/update/stable/${row.platform}`;
		const written = plan.writes.filter((write) => write.path.startsWith(`${folder}/`));
		assert.deepEqual(
			written.map((write) => [write.path, write.bytes]),
			[
				[`${folder}/feed.json.signed`, row.feed],
				[`${folder}/feed.json.sig`, row.signature],
			],
		);
	}
	const source = plan.writes.at(-1).bytes.toString('utf8');
	assert.match(source, /'macos-aarch64': `[^`]*\/v0\.1\.0\/Orchbee-0\.1\.0-macos-aarch64\.dmg`/u);
	assert.match(source, /'macos-x86_64': `[^`]*\/v0\.1\.0\/Orchbee-0\.1\.0-macos-x86_64\.dmg`/u);
});

test('a tracked feed of a platform the release did not publish is removed', () => {
	const plan = planWebsiteRelease({
		...good,
		tracked: ['stable/windows-x86_64', 'stable/macos-aarch64', 'preview/macos-aarch64'],
	});
	assert.deepEqual(plan.removals, [
		'apps/website/public/update/stable/macos-aarch64/feed.json.signed',
		'apps/website/public/update/stable/macos-aarch64/feed.json.sig',
	]);
});

test('SHA256SUMS names the published rows, which must include Windows and be offered', () => {
	const sums = (...platforms) =>
		Buffer.from(
			platforms
				.map((platform) => `${'1'.repeat(64)}  orchbee-0.1.0-${platform}.feed.json\n`)
				.join('') + `${'2'.repeat(64)}  Orchbee-0.1.0-macos-aarch64.dmg\n`,
		);
	assert.deepEqual(publishedPlatforms(sums('macos-x86_64', 'windows-x86_64'), '0.1.0'), {
		problems: [],
		platforms: ['windows-x86_64', 'macos-x86_64'],
	});
	assert.match(
		publishedPlatforms(sums('macos-aarch64'), '0.1.0').problems.join('\n'),
		/lists no orchbee-0\.1\.0-windows-x86_64\.feed\.json/u,
	);
	assert.match(
		publishedPlatforms(sums('windows-x86_64', 'linux-x86_64'), '0.1.0').problems.join('\n'),
		/linux-x86_64, which the website offers no download for/u,
	);
});

test('nothing is written when the signature or the archive check fails', () => {
	const row = (change) => ({ rows: [{ ...windows, ...change }] });
	const refusals = [
		[row({ signature: Buffer.from('{"signatures":[]}\n') }), /malformed/u],
		[{ table: ['c'.repeat(64)] }, /untrusted/u],
		[
			row({ feed: Buffer.from(feed.toString('utf8').replace('fixture.1', 'fixture.2')) }),
			/invalid/u,
		],
		[{ version: '0.1.1' }, /names version 0\.1\.0, not 0\.1\.1/u],
		[row({ archive: { size: 26999999, sha256: '9'.repeat(64) } }), /26999999 bytes/u],
		[row({ archive: { size: 27000000, sha256: '8'.repeat(64) } }), /SHA-256/u],
		[row({ archive: undefined }), /not fetched/u],
		[row({ installerPublic: false }), /setup\.exe is not public/u],
		[
			{ rows: [windows, { ...macosRow('aarch64'), installerPublic: false }] },
			/macos-aarch64\.dmg is not public/u,
		],
		[{ release: 'export const release = {};\n' }, /block to set/u],
		// A stand-in source never reaches the network, so it refuses an absolute archive address.
		[{ standIn: true }, /relative path/u],
	];
	for (const [change, expected] of refusals) {
		const plan = planWebsiteRelease({ ...good, tracked: ['stable/macos-x86_64'], ...change });
		assert.deepEqual(plan.writes, [], JSON.stringify(Object.keys(change)));
		assert.deepEqual(plan.removals, [], JSON.stringify(Object.keys(change)));
		assert.match(plan.problems.join('\n'), expected);
	}
});

// JSON.parse admits these spellings and the updater's reader refuses them, so neither the signing
// tool nor the website command may let one through.
test('a feed not in its written form is neither signed nor moves the channel', (t) => {
	const parent = realpathSync(tmpdir());
	const root = mkdtempSync(path.join(parent, 'orchbee-website-release-test-'));
	t.after(() => rmSync(root, { recursive: true }));
	const key = testKey;
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
			rows: [
				{
					...windows,
					feed: bytes,
					signature: Buffer.from(signatureDocument(bytes, [key], FEED_DOMAIN)),
				},
			],
		});
		assert.deepEqual(plan.writes, [], name);
		assert.match(plan.problems.join('\n'), /written form/u, name);
	}
});
