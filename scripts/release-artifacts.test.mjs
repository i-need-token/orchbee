import assert from 'node:assert/strict';
import { generateKeyPairSync } from 'node:crypto';
import {
	mkdirSync,
	mkdtempSync,
	readFileSync,
	readdirSync,
	realpathSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { crc32 } from 'node:zlib';

import { desktopFilesDocument } from './desktop-files.mjs';
import { checkArtifacts } from './release-artifacts.mjs';
import { feedDocument, gateFeed, sha256 } from './release-feed.mjs';
import { FEED_DOMAIN, loadKey, signatureDocument } from './sign-release-manifest.mjs';

// The repository's public test key, which TEST_KEYS trusts; nothing here is a release key.
const testKey = loadKey(
	readFileSync(
		new URL(
			'../apps/orchbee/src/cli/update/tests/fixtures/test-only-signing-key.pem',
			import.meta.url,
		),
	),
);
const stem = 'ob-test-0123456789abcdef';
const version = '0.1.0-beta.2';
const platform = 'windows-x86_64';
const base = `${stem}-${version}-${platform}`;
const feedFile = `${base}.feed.json`;

function scratch(t) {
	const parent = realpathSync(tmpdir());
	const root = mkdtempSync(path.join(parent, 'orchbee-release-artifacts-test-'));
	t.after(() => {
		const actual = realpathSync(root);
		assert.equal(path.dirname(actual), parent);
		assert.ok(path.basename(actual).startsWith('orchbee-release-artifacts-test-'));
		rmSync(actual, { recursive: true });
	});
	return root;
}

function writeSums(directory) {
	const lines = readdirSync(directory, { withFileTypes: true })
		.filter(
			(entry) => entry.isFile() && entry.name !== 'SHA256SUMS' && !entry.name.endsWith('.sig'),
		)
		.map((entry) => entry.name)
		.toSorted()
		.map((name) => `${sha256(readFileSync(path.join(directory, name)))}  ${name}\n`);
	writeFileSync(path.join(directory, 'SHA256SUMS'), lines.join(''));
}

/** A zip of stored entries, `[name, bytes]`, in the order given. */
function zip(entries) {
	const locals = [];
	const centrals = [];
	let offset = 0;
	for (const [name, data] of entries) {
		const nameBytes = Buffer.from(name, 'utf8');
		const local = Buffer.alloc(30);
		local.writeUInt32LE(0x04034b50, 0);
		local.writeUInt16LE(20, 4);
		local.writeUInt32LE(crc32(data), 14);
		local.writeUInt32LE(data.length, 18);
		local.writeUInt32LE(data.length, 22);
		local.writeUInt16LE(nameBytes.length, 26);
		const central = Buffer.alloc(46);
		central.writeUInt32LE(0x02014b50, 0);
		central.writeUInt16LE(20, 4);
		central.writeUInt16LE(20, 6);
		central.writeUInt32LE(crc32(data), 16);
		central.writeUInt32LE(data.length, 20);
		central.writeUInt32LE(data.length, 24);
		central.writeUInt16LE(nameBytes.length, 28);
		central.writeUInt32LE(offset, 42);
		locals.push(local, nameBytes, data);
		centrals.push(central, nameBytes);
		offset += local.length + nameBytes.length + data.length;
	}
	const directory = Buffer.concat(centrals);
	const end = Buffer.alloc(22);
	end.writeUInt32LE(0x06054b50, 0);
	end.writeUInt16LE(entries.length, 8);
	end.writeUInt16LE(entries.length, 10);
	end.writeUInt32LE(directory.length, 12);
	end.writeUInt32LE(offset, 16);
	return Buffer.concat([...locals, directory, end]);
}

/**
 * A packaged window directory under `root` holding `release.json` as its bundled runtime manifest,
 * with its list, and the window archive of it as `[name, bytes]` entries.
 */
function windowEntries(root, manifest) {
	const window = path.join(root, 'packaged-window');
	mkdirSync(path.join(window, 'resources', 'cohort'), { recursive: true });
	const files = new Map([
		[`${stem}-desktop.exe`, Buffer.from('window')],
		['resources/app.asar', Buffer.from('application')],
		['resources/cohort/release.json', manifest],
	]);
	for (const [name, bytes] of files) writeFileSync(path.join(window, ...name.split('/')), bytes);
	files.set('desktop-files.json', desktopFilesDocument(window, version));
	const folder = `${stem}-desktop-${version}-${platform}`;
	rmSync(window, { recursive: true });
	return [
		[`${folder}/`, Buffer.alloc(0)],
		...[...files].map(([name, bytes]) => [`${folder}/${name}`, bytes]),
	];
}

/** A small test-stem artifacts directory as `collect --profile test` leaves it. */
function artifacts(
	t,
	{
		archive = { size: undefined, sha256: undefined },
		key = testKey,
		window = undefined,
		windowList = undefined,
	} = {},
) {
	const root = scratch(t);
	const release = path.join(root, base);
	mkdirSync(release);
	const program = Buffer.from('program');
	writeFileSync(path.join(release, `${stem}.exe`), program);
	const manifest = Buffer.from(
		JSON.stringify({
			schemaVersion: 2,
			product: 'orchbee',
			channel: 'stable',
			platform,
			version,
			buildId: 'test.1',
			files: [
				{ path: `${stem}.exe`, url: `${stem}.exe`, sha256: sha256(program), size: program.length },
			],
			components: [],
		}),
	);
	writeFileSync(path.join(release, 'release.json'), manifest);
	writeFileSync(path.join(release, 'release.json.sig'), 'not checked here');
	const runtime = Buffer.from('zip bytes');
	writeFileSync(path.join(root, `${base}.zip`), runtime);
	let desktop;
	if (window !== undefined) {
		const entries = windowEntries(root, manifest);
		const list = entries.find(([name]) => name.endsWith('/desktop-files.json'))[1];
		const bytes = zip(window(entries));
		const name = `${stem}-desktop-${version}-${platform}.zip`;
		writeFileSync(path.join(root, name), bytes);
		desktop = {
			version,
			manifestSha256: windowList ?? sha256(list),
			archive: { url: name, size: bytes.length, sha256: sha256(bytes) },
		};
	}
	const document = feedDocument({
		channel: 'stable',
		platform,
		version,
		buildId: 'test.1',
		manifestSha256: sha256(manifest),
		archive: {
			url: `${base}.zip`,
			size: archive.size ?? runtime.length,
			sha256: archive.sha256 ?? sha256(runtime),
		},
		desktop,
	});
	writeFileSync(path.join(root, feedFile), document);
	writeFileSync(
		path.join(root, `${feedFile}.sig`),
		signatureDocument(document, [key], FEED_DOMAIN),
	);
	writeSums(root);
	return root;
}

test('check admits a feed that matches its archive and carries a trusted signature', (t) => {
	assert.deepEqual(checkArtifacts(artifacts(t)), []);
});

test('check refuses a feed whose archive size or digest differs from the zip', (t) => {
	const size = checkArtifacts(artifacts(t, { archive: { size: 4 } }));
	assert.deepEqual(size, [`${feedFile}: archive.size is 4, but ${base}.zip has 9 bytes`]);
	const digest = checkArtifacts(artifacts(t, { archive: { sha256: '0'.repeat(64) } }));
	assert.deepEqual(digest, [`${feedFile}: archive.sha256 does not match ${base}.zip`]);
});

test('check admits a window archive whose list names exactly its files, and its gate copy', (t) => {
	const root = artifacts(t, { window: (entries) => entries });
	assert.deepEqual(checkArtifacts(root), []);
	// The gate copy names both archives by their bare names; the feed job signs it.
	const gate = path.join(root, `${base}.gate.feed.json`);
	const copy = gateFeed(readFileSync(path.join(root, feedFile)));
	writeFileSync(gate, copy);
	writeFileSync(`${gate}.sig`, signatureDocument(copy, [testKey], FEED_DOMAIN));
	assert.deepEqual(checkArtifacts(root), []);
});

test('check refuses a window archive that its list or the feed does not describe', (t) => {
	const folder = `${stem}-desktop-${version}-${platform}`;
	const changed = checkArtifacts(
		artifacts(t, {
			window: (entries) =>
				entries.map(([name, bytes]) =>
					name.endsWith('/resources/app.asar') ? [name, Buffer.from('other')] : [name, bytes],
				),
		}),
	);
	assert.deepEqual(changed, [
		`${feedFile}: ${folder}.zip holds resources/app.asar with other bytes than desktop-files.json lists`,
	]);
	const extra = checkArtifacts(
		artifacts(t, {
			window: (entries) => [...entries, [`${folder}/unlisted.dll`, Buffer.from('dll')]],
		}),
	);
	assert.deepEqual(extra, [
		`${feedFile}: ${folder}.zip holds unlisted.dll, which desktop-files.json omits`,
	]);
	const outside = checkArtifacts(
		artifacts(t, { window: (entries) => [...entries, ['elsewhere.txt', Buffer.from('x')]] }),
	);
	assert.deepEqual(outside, [
		`${feedFile}: ${folder}.zip holds elsewhere.txt outside the one top-level folder ${folder}`,
	]);
	const unbound = checkArtifacts(
		artifacts(t, { window: (entries) => entries, windowList: '0'.repeat(64) }),
	);
	assert.deepEqual(unbound, [
		`${feedFile}: extensions.desktop.manifest.sha256 does not match ${folder}.zip's list`,
	]);
});

test('check refuses a feed signature that does not verify with the build table', (t) => {
	const untrusted = checkArtifacts(
		artifacts(t, { key: generateKeyPairSync('ed25519').privateKey }),
	);
	assert.deepEqual(untrusted, [
		`${feedFile}: .sig does not verify with a key of the build's table (untrusted)`,
	]);
	// A feed changed after signing: the signature names the trusted key but no longer verifies.
	const changed = artifacts(t);
	const feed = path.join(changed, feedFile);
	const signature = readFileSync(`${feed}.sig`);
	const other = readFileSync(feed, 'utf8').replace('"test.1"', '"test.2"');
	writeFileSync(feed, other);
	writeFileSync(`${feed}.sig`, signature);
	writeSums(changed);
	const problems = checkArtifacts(changed);
	assert.ok(
		problems.some((problem) => /\(invalid\)/u.test(problem)),
		problems.join('\n'),
	);
	// A signature beside no feed is refused too.
	const orphan = artifacts(t);
	rmSync(path.join(orphan, feedFile));
	writeSums(orphan);
	assert.ok(checkArtifacts(orphan).includes(`${feedFile}.sig signs no feed document`));
});
