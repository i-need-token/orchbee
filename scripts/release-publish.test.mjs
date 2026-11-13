import assert from 'node:assert/strict';
import { createPublicKey } from 'node:crypto';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
	checkPreconditions,
	distTagFor,
	npmDecision,
	npmPublicationOrder,
	planGithubRelease,
	planRelease,
	registryIntegrity,
	releaseTable,
} from './release-publish.mjs';

const rows = [
	{ platform: 'windows-x86_64', runner: 'windows-latest', build: true, publish: true },
	{ platform: 'windows-aarch64', runner: 'windows-11-arm', build: false, publish: false },
	{ platform: 'linux-x86_64', runner: 'ubuntu-24.04', build: true, publish: false },
];

test('a pushed tag publishes its version and a manual run only rehearses the one it names', () => {
	const tag = planRelease({ event: 'push', ref: 'refs/tags/v0.1.0-beta.1', platforms: rows });
	assert.equal(tag.version, '0.1.0-beta.1');
	assert.equal(tag.tag, 'v0.1.0-beta.1');
	assert.equal(tag.publish, true);
	assert.deepEqual(tag.matrix.include, [rows[0]]);
	const rehearsal = planRelease({
		event: 'workflow_dispatch',
		ref: 'refs/heads/main',
		version: '0.1.0-beta.1',
		platforms: rows,
	});
	assert.equal(rehearsal.publish, false);
	assert.equal(rehearsal.version, '0.1.0-beta.1');
});

test('the release path refuses what it cannot release with a sentence naming it', () => {
	const refusals = [
		[{ event: 'push', ref: 'refs/heads/main' }, /pushed tag/u],
		[{ event: 'push', ref: 'refs/tags/v0.1' }, /pushed tag/u],
		[{ event: 'push', ref: 'refs/tags/v0.1.0+build.1' }, /pushed tag/u],
		[{ event: 'push', ref: 'refs/tags/v0.1.0', version: '0.1.0' }, /give no version/u],
		[{ event: 'workflow_dispatch', ref: 'refs/heads/main', version: 'v0.1.0' }, /without the v/u],
		[{ event: 'workflow_dispatch', ref: 'refs/heads/main' }, /rehearsal names a version/u],
		[{ event: 'schedule', ref: 'refs/heads/main' }, /not from schedule/u],
	];
	for (const [input, expected] of refusals)
		assert.throws(() => planRelease({ ...input, platforms: rows }), expected, input.ref);
	const plan = (platforms) => () =>
		planRelease({ event: 'push', ref: 'refs/tags/v0.1.0', platforms });
	assert.throws(plan([rows[1], rows[2]]), /No row/u);
	assert.throws(plan([rows[0], { ...rows[2], publish: true }]), /one platform row/u);
});

test('a pre-release goes to the dist-tag next and only a plain version to latest', () => {
	assert.equal(distTagFor('0.1.0-beta.1'), 'next');
	assert.equal(distTagFor('1.0.0-rc.2'), 'next');
	assert.equal(distTagFor('0.1.0'), 'latest');
	assert.equal(
		planRelease({ event: 'push', ref: 'refs/tags/v0.1.0', platforms: rows }).distTag,
		'latest',
	);
});

// The repository's public test key stands in for a release key; nothing here is a release key.
const testKey = readFileSync(
	new URL(
		'../apps/orchbee/src/cli/update/tests/fixtures/test-only-signing-key.pem',
		import.meta.url,
	),
	'utf8',
);
const testPublic = createPublicKey(testKey)
	.export({ format: 'der', type: 'spki' })
	.subarray(-32)
	.toString('hex');
const source = (keys) =>
	`/// Keys.\nconst RELEASE_KEYS: &[TrustedKey] = &[${keys
		.map((key) => `\n    TrustedKey::from_hex("${key}"),`)
		.join(
			'',
		)}\n];\n\nconst TEST_KEYS: &[TrustedKey] = &[TrustedKey::from_hex(\n    "${'1'.repeat(64)}",\n)];\n`;

test('the release table is read from the Rust source, the shipped one included', () => {
	assert.deepEqual(releaseTable(source([])), []);
	assert.deepEqual(releaseTable(source(['a'.repeat(64), 'b'.repeat(64)])), [
		'a'.repeat(64),
		'b'.repeat(64),
	]);
	assert.equal(releaseTable('const OTHER: &[u8] = &[];'), undefined);
	const shipped = readFileSync(
		new URL('../apps/orchbee/src/cli/update/signature.rs', import.meta.url),
		'utf8',
	);
	assert.ok(Array.isArray(releaseTable(shipped)));
});

test('each missing precondition is named, and no message carries a secret value', () => {
	const run = (environment, keys, npm = 'NPM_TOKEN') =>
		checkPreconditions({
			environment,
			signingKeyVariable: 'ORCHBEE_RELEASE_SIGNING_KEY',
			npmTokenVariable: npm ?? undefined,
			source: source(keys),
		});
	const secretLike = 'npm_secretvalue1234567890';
	const nothing = run({}, []);
	assert.equal(nothing.problems.length, 3);
	assert.match(nothing.problems[0], /ORCHBEE_RELEASE_SIGNING_KEY is empty/u);
	assert.match(nothing.problems[1], /RELEASE_KEYS .* is empty/u);
	assert.match(nothing.problems[2], /NPM_TOKEN is empty/u);

	const garbage = run({ ORCHBEE_RELEASE_SIGNING_KEY: 'not a key', NPM_TOKEN: secretLike }, [
		testPublic,
	]);
	assert.deepEqual(garbage.problems, [
		'ORCHBEE_RELEASE_SIGNING_KEY does not hold an Ed25519 private key in PKCS #8 PEM form.',
	]);

	const untrusted = run({ ORCHBEE_RELEASE_SIGNING_KEY: testKey, NPM_TOKEN: secretLike }, [
		'c'.repeat(64),
	]);
	assert.equal(untrusted.problems.length, 1);
	assert.match(untrusted.problems[0], /key id [0-9a-f]{16}\) is not in RELEASE_KEYS/u);

	const ready = run({ ORCHBEE_RELEASE_SIGNING_KEY: testKey, NPM_TOKEN: secretLike }, [testPublic]);
	assert.deepEqual(ready.problems, []);
	// A rehearsal names no npm credential and needs none.
	assert.deepEqual(run({ ORCHBEE_RELEASE_SIGNING_KEY: testKey }, [testPublic], null).problems, []);

	for (const result of [nothing, garbage, untrusted, ready]) {
		const text = [...result.problems, ...result.notes].join('\n');
		assert.ok(!text.includes(secretLike));
		assert.ok(!text.includes('PRIVATE KEY'));
		assert.ok(!text.includes(testPublic));
	}
});

const assets = [
	{ name: 'Orchbee-0.1.0-beta.1-windows-x86_64-setup.exe', sha256: '1'.repeat(64) },
	{ name: 'SHA256SUMS', sha256: '2'.repeat(64) },
];
const remote = (asset, digest = `sha256:${asset.sha256}`) => ({ name: asset.name, digest });

test('a stopped publication completes its draft and never changes a published release', () => {
	assert.deepEqual(planGithubRelease({ existing: [], assets }), {
		action: 'create',
		upload: assets.map((asset) => asset.name),
		problems: [],
	});
	const partial = planGithubRelease({
		existing: [{ draft: true, assets: [remote(assets[0])] }],
		assets,
	});
	assert.deepEqual(partial, { action: 'upload', upload: ['SHA256SUMS'], problems: [] });
	for (const draft of [true, false]) {
		const complete = planGithubRelease({
			existing: [{ draft, assets: assets.map((a) => remote(a)) }],
			assets,
		});
		assert.deepEqual(complete, { action: 'none', upload: [], problems: [] });
	}
	const refused = [
		{ draft: false, assets: [remote(assets[0])] },
		{ draft: true, assets: [remote(assets[0], `sha256:${'9'.repeat(64)}`)] },
		{ draft: true, assets: [remote(assets[0], null)] },
		{ draft: true, assets: [{ name: 'other.zip', digest: `sha256:${'3'.repeat(64)}` }] },
	];
	for (const release of refused) {
		const plan = planGithubRelease({ existing: [release], assets });
		assert.equal(plan.action, 'none');
		assert.deepEqual(plan.upload, []);
		assert.equal(plan.problems.length, 1, JSON.stringify(release));
	}
	const twice = planGithubRelease({
		existing: [
			{ draft: true, assets: [] },
			{ draft: true, assets: [] },
		],
		assets,
	});
	assert.match(twice.problems[0], /2 releases name the tag/u);
});

const repositoryUrl = 'https://github.com/owner/orchbee';
const manifest = (name, extra = {}) => ({
	name,
	version: '0.1.0-beta.1',
	repository: { type: 'git', url: `git+${repositoryUrl}.git` },
	publishConfig: { access: 'public', tag: 'next' },
	...extra,
});
const main = manifest('orchbee', {
	bin: { orchbee: 'bin/orchbee.mjs' },
	optionalDependencies: { '@orchbee/cli-win32-x64': '0.1.0-beta.1' },
});
const sub = manifest('@orchbee/cli-win32-x64');

test('platform packages publish before the main package, which must list exactly them', () => {
	const order = npmPublicationOrder(
		[
			{ file: 'orchbee.tgz', manifest: main },
			{ file: 'cli.tgz', manifest: sub },
		],
		{ version: '0.1.0-beta.1', repositoryUrl },
	);
	assert.deepEqual(
		order.map((entry) => entry.manifest.name),
		['@orchbee/cli-win32-x64', 'orchbee'],
	);
	const refusals = [
		[[{ file: 'cli.tgz', manifest: sub }], /exactly one main package/u],
		[[{ file: 'orchbee.tgz', manifest: main }], /no platform package/u],
		[
			[
				{ file: 'orchbee.tgz', manifest: { ...main, optionalDependencies: {} } },
				{ file: 'cli.tgz', manifest: sub },
			],
			/not exactly the platform packages/u,
		],
		[
			[
				{ file: 'orchbee.tgz', manifest: main },
				{ file: 'cli.tgz', manifest: { ...sub, version: '0.1.0-beta.2' } },
			],
			/not 0\.1\.0-beta\.1/u,
		],
		[
			[
				{ file: 'orchbee.tgz', manifest: { ...main, name: 'ob-test-0123456789abcdef' } },
				{ file: 'cli.tgz', manifest: { ...sub, name: '@ob-test-0123456789abcdef/cli-win32-x64' } },
			],
			/main package named orchbee/u,
		],
		[
			[
				{ file: 'orchbee.tgz', manifest: main },
				{ file: 'cli.tgz', manifest: { ...sub, publishConfig: { tag: 'latest' } } },
			],
			/not next/u,
		],
		[
			[
				{ file: 'orchbee.tgz', manifest: { ...main, repository: undefined } },
				{ file: 'cli.tgz', manifest: sub },
			],
			/provenance will name/u,
		],
	];
	for (const [packages, expected] of refusals)
		assert.throws(
			() => npmPublicationOrder(packages, { version: '0.1.0-beta.1', repositoryUrl }),
			expected,
		);
});

test('a version npm already holds is skipped with the same tarball and fails with another', () => {
	const base = { name: 'orchbee', version: '0.1.0-beta.1', local: 'sha512-AAAA' };
	assert.deepEqual(npmDecision({ ...base, published: undefined }), { action: 'publish' });
	assert.deepEqual(npmDecision({ ...base, published: 'sha512-AAAA' }), { action: 'skip' });
	const other = npmDecision({ ...base, published: 'sha512-BBBB' });
	assert.equal(other.action, 'fail');
	assert.match(other.problem, /never replaced/u);
});

test('the registry answer is read without the network', async () => {
	const requested = [];
	const answer = (status, body) => async (url) => {
		requested.push(url);
		return { status, ok: status >= 200 && status < 300, json: async () => body };
	};
	assert.equal(await registryIntegrity('orchbee', '0.1.0', answer(404)), undefined);
	const document = { versions: { '0.1.0': { dist: { integrity: 'sha512-AAAA' } } } };
	assert.equal(
		await registryIntegrity('@orchbee/cli-win32-x64', '0.1.0', answer(200, document)),
		'sha512-AAAA',
	);
	assert.equal(requested.at(-1), 'https://registry.npmjs.org/@orchbee%2fcli-win32-x64');
	assert.equal(await registryIntegrity('orchbee', '0.1.1', answer(200, document)), undefined);
	await assert.rejects(registryIntegrity('orchbee', '0.1.0', answer(503)), /answered 503/u);
});
