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

import { checkArtifacts } from './release-artifacts.mjs';
import { feedDocument, sha256 } from './release-feed.mjs';
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

/** A small test-stem artifacts directory as `collect --profile test` leaves it. */
function artifacts(t, { archive = { size: undefined, sha256: undefined }, key = testKey } = {}) {
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
	const zip = Buffer.from('zip bytes');
	writeFileSync(path.join(root, `${base}.zip`), zip);
	const document = feedDocument({
		channel: 'stable',
		platform,
		version,
		buildId: 'test.1',
		manifestSha256: sha256(manifest),
		archive: {
			url: `${base}.zip`,
			size: archive.size ?? zip.length,
			sha256: archive.sha256 ?? sha256(zip),
		},
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
