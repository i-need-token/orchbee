import assert from 'node:assert/strict';
import { createPublicKey } from 'node:crypto';
import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	checkPreconditions,
	distTagFor,
	npmDecision,
	npmPublicationOrder,
	planGithubRelease,
	planRelease,
	registryIntegrity,
	releaseAssets,
	releaseTable,
	selectGatePaths,
	selectMacosGatePaths,
	reuseDecision,
	tagNotes,
	verifyState,
} from './release-publish.mjs';
import {
	parseGateArguments,
	probeFacts,
	seedWindowStore,
	StepFailure,
	UsageError,
} from './release-update-check.mjs';
import {
	FEED_DOMAIN,
	loadKey,
	MANIFEST_DOMAIN,
	signatureDocument,
} from './sign-release-manifest.mjs';

const rows = [
	{ platform: 'windows-x86_64', runner: 'windows-latest', build: true, publish: true },
	{ platform: 'windows-aarch64', runner: 'windows-11-arm', build: false, publish: false },
	{ platform: 'macos-aarch64', runner: 'macos-15', build: true, publish: false },
];

test('a pushed tag publishes its rows and a manual run rehearses every row that builds', () => {
	const tag = planRelease({ event: 'push', ref: 'refs/tags/v0.1.0-beta.1', platforms: rows });
	assert.equal(tag.version, '0.1.0-beta.1');
	assert.equal(tag.tag, 'v0.1.0-beta.1');
	assert.equal(tag.publish, true);
	assert.deepEqual(tag.matrix.include, [rows[0]]);
	assert.deepEqual(tag.macosMatrix.include, []);
	assert.equal(tag.macos, false);
	const rehearsal = planRelease({
		event: 'workflow_dispatch',
		ref: 'refs/heads/main',
		version: '0.1.0-beta.1',
		platforms: rows,
	});
	assert.equal(rehearsal.publish, false);
	assert.equal(rehearsal.version, '0.1.0-beta.1');
	assert.deepEqual(rehearsal.matrix.include, [rows[0], rows[2]]);
	assert.deepEqual(rehearsal.windowsMatrix.include, [rows[0]]);
	assert.deepEqual(rehearsal.macosMatrix.include, [rows[2]]);
	assert.equal(rehearsal.macos, true);
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
	assert.throws(plan([{ ...rows[2], publish: true }]), /needs a Windows row/u);
	const freebsd = {
		platform: 'freebsd-x86_64',
		runner: 'ubuntu-24.04',
		build: true,
		publish: true,
	};
	assert.throws(plan([rows[0], freebsd]), /no build jobs for it/u);
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

// The release gate picks among releases v0.2.0-beta.<n>, the first of which updates itself.
const gateNow = new Date('2026-12-31T12:00:00Z');
const day = 24 * 60 * 60 * 1000;
const published = (n, daysAgo, extra = {}) => ({
	tag: `v0.2.0-beta.${n}`,
	draft: false,
	publishedAt: new Date(gateNow.getTime() - daysAgo * day).toISOString(),
	...extra,
});
const gatePaths = (releases, version = '0.2.0-beta.99', firstVersion = '0.2.0-beta.1') =>
	selectGatePaths({ releases, version, now: gateNow, firstVersion });

test('the gate updates from the previous, the next older, the middle and the oldest release', () => {
	assert.deepEqual(gatePaths([]), []);
	assert.deepEqual(gatePaths([published(1, 3)]), ['v0.2.0-beta.1']);
	const four = [published(3, 2), published(1, 30), published(4, 1), published(2, 10)];
	assert.deepEqual(gatePaths(four), [
		'v0.2.0-beta.4',
		'v0.2.0-beta.3',
		'v0.2.0-beta.2',
		'v0.2.0-beta.1',
	]);
	const nine = Array.from({ length: 9 }, (_, index) => published(index + 1, 80 - index * 9));
	assert.deepEqual(gatePaths(nine), [
		'v0.2.0-beta.9',
		'v0.2.0-beta.8',
		'v0.2.0-beta.5',
		'v0.2.0-beta.1',
	]);
});

test('gate ties: equal times order by version and the older of two middle releases counts', () => {
	const six = [4, 1, 6, 3, 5, 2].map((n) => published(n, 5));
	assert.deepEqual(gatePaths(six), [
		'v0.2.0-beta.6',
		'v0.2.0-beta.5',
		'v0.2.0-beta.3',
		'v0.2.0-beta.1',
	]);
	assert.deepEqual(gatePaths([published(2, 5), published(1, 5), published(2, 5)]), [
		'v0.2.0-beta.2',
		'v0.2.0-beta.1',
	]);
});

test('the gate window holds only published releases from the first one on, within 90 days', () => {
	const releases = [
		published(2, 90),
		published(1, 91),
		{ tag: 'v0.1.0-beta.13', draft: false, publishedAt: published(0, 1).publishedAt },
		published(5, 1, { draft: true }),
		published(4, 2, { tag: 'nightly' }),
		published(3, 3, { publishedAt: null }),
		published(99, 1),
		published(100, 1),
	];
	assert.deepEqual(gatePaths(releases, '0.2.0-beta.99', '0.2.0-beta.2'), ['v0.2.0-beta.2']);
	assert.deepEqual(gatePaths([published(1, 1)], '0.2.0-beta.1'), []);
});

test('macOS gate paths pick the newest published release carrying macos-aarch64 feed assets', () => {
	const winAssets = (ver) => [
		`orchbee-${ver}-windows-x86_64.feed.json`,
		`orchbee-${ver}-windows-x86_64.feed.json.sig`,
	];
	const macAssets = (ver) => [
		`orchbee-${ver}-macos-aarch64.feed.json`,
		`orchbee-${ver}-macos-aarch64.feed.json.sig`,
	];
	const releases = [
		{
			tag: 'v0.1.0-beta.12',
			draft: false,
			publishedAt: new Date(gateNow.getTime() - 20 * day).toISOString(),
			assets: winAssets('0.1.0-beta.12'),
		},
		{
			tag: 'v0.1.0-beta.17',
			draft: false,
			publishedAt: new Date(gateNow.getTime() - 5 * day).toISOString(),
			assets: [...winAssets('0.1.0-beta.17'), ...macAssets('0.1.0-beta.17')],
		},
		{
			tag: 'v0.1.0-beta.18',
			draft: false,
			publishedAt: new Date(gateNow.getTime() - 2 * day).toISOString(),
			assets: winAssets('0.1.0-beta.18'),
		},
		{
			tag: 'v0.1.0-beta.19',
			draft: false,
			publishedAt: new Date(gateNow.getTime() - 1 * day).toISOString(),
			assets: [`orchbee-0.1.0-beta.19-macos-aarch64.feed.json`],
		},
	];
	assert.deepEqual(selectMacosGatePaths({ releases, version: '0.1.0-beta.20', now: gateNow }), [
		'v0.1.0-beta.17',
	]);
	assert.deepEqual(
		selectMacosGatePaths({
			releases,
			version: '0.1.0-beta.20',
			now: gateNow,
			hasMacosRow: false,
		}),
		[],
	);
	assert.deepEqual(selectGatePaths({ releases, version: '0.1.0-beta.20', now: gateNow }), [
		'v0.1.0-beta.18',
		'v0.1.0-beta.17',
	]);
});

// Answers the two API reads of tagNotes the way `gh api --jq` prints them.
const github =
	(object, message = '') =>
	([, route]) => ({
		stdout: route.includes('/git/ref/tags/') ? JSON.stringify(object) : `${message}\n`,
	});

test('the notes are the tag message without its signature, and a tag without notes stops', () => {
	const annotated = (message) => github({ type: 'tag', sha: 'a'.repeat(40) }, message);
	const notes = '## Fixed\n\n- # in a list item stays\n-----BEGIN quoted text-----';
	const signed = `\n${notes}\n\n-----BEGIN PGP SIGNATURE-----\n\nwsBc\n-----END PGP SIGNATURE-----`;
	assert.equal(tagNotes('v0.1.0', 'owner/orchbee', annotated(signed)), notes);
	const ssh = `${notes}\n-----BEGIN SSH SIGNATURE-----\nU1NI\n-----END SSH SIGNATURE-----`;
	assert.equal(tagNotes('v0.1.0', 'owner/orchbee', annotated(ssh)), notes);
	const refusals = [
		github({ type: 'commit', sha: 'b'.repeat(40) }),
		annotated(' \n\n'),
		annotated('-----BEGIN PGP SIGNATURE-----\nwsBc\n-----END PGP SIGNATURE-----'),
	];
	for (const gh of refusals)
		assert.throws(
			() => tagNotes('v0.1.0', 'owner/orchbee', gh),
			/git tag -a v0\.1\.0 --cleanup=verbatim -F <notes file>/u,
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

test('a draft only receives a feed document with a signature from a release key', (t) => {
	const root = mkdtempSync(path.join(realpathSync(tmpdir()), 'orchbee-release-assets-test-'));
	t.after(() => rmSync(root, { recursive: true }));
	const feed = Buffer.from('{"schemaVersion":1}\n');
	writeFileSync(path.join(root, 'SHA256SUMS'), '');
	writeFileSync(path.join(root, 'orchbee-0.1.0-windows-x86_64.feed.json'), feed);
	assert.throws(() => releaseAssets(root, [testPublic]), /has no .*\.feed\.json\.sig/u);
	const signature = path.join(root, 'orchbee-0.1.0-windows-x86_64.feed.json.sig');
	writeFileSync(signature, signatureDocument(feed, [loadKey(testKey)], FEED_DOMAIN));
	assert.deepEqual(
		releaseAssets(root, [testPublic]).map((asset) => asset.name),
		[
			'orchbee-0.1.0-windows-x86_64.feed.json',
			'orchbee-0.1.0-windows-x86_64.feed.json.sig',
			'SHA256SUMS',
		].toSorted(),
	);
	assert.throws(() => releaseAssets(root, ['c'.repeat(64)]), /\(untrusted\)/u);
	writeFileSync(signature, signatureDocument(feed, [loadKey(testKey)], MANIFEST_DOMAIN));
	assert.throws(() => releaseAssets(root, [testPublic]), /\(invalid\)/u);
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

test('seedWindowStore pre-seeds desktop/ when supported and fails closed on regressions', (t) => {
	const root = mkdtempSync(path.join(realpathSync(tmpdir()), 'orchbee-window-seed-test-'));
	t.after(() => rmSync(root, { recursive: true, force: true }));
	const clientPath = path.join(root, 'bundled-orchbee');
	writeFileSync(clientPath, '');

	const desktopMember = (ver) => ({
		version: ver,
		archive: {
			format: 'zip',
			url: `https://github.com/owner/orchbee/releases/download/v${ver}/orchbee-desktop-${ver}-windows-x86_64.zip`,
			size: 128,
			sha256: 'a'.repeat(64),
		},
		manifest: { sha256: 'b'.repeat(64) },
	});
	const activeChecked = [];
	const port = {
		platform: 'windows-x86_64',
		bundledWindowClient: () => clientPath,
		expectWindowActive: (_storeRoot, _stem, ver) => activeChecked.push(ver),
	};
	const baseArgs = {
		port,
		source: { kind: 'https', base: 'https://github.com/owner/orchbee/releases/download/v0.1.0/' },
		stem: 'orchbee',
		work: root,
		storeRoot: path.join(root, 'store'),
		env: {},
		fetchWindow: () => path.join(root, 'window.zip'),
		unpackWindow: () => root,
	};

	// Missing extensions.desktop on either side skips without fetching or running the helper.
	assert.deepEqual(
		seedWindowStore({
			...baseArgs,
			olderFeed: { version: '0.1.0-beta.4' },
			offeredFeed: {
				version: '0.1.0-beta.18',
				extensions: { desktop: desktopMember('0.1.0-beta.18') },
			},
			fetchWindow: () => assert.fail('should not fetch window archive'),
		}),
		{ seeded: false },
	);

	// Pre-window release (before 0.1.0-beta.14) exiting 2 without orchbee.install skips gracefully.
	assert.deepEqual(
		seedWindowStore({
			...baseArgs,
			olderFeed: {
				version: '0.1.0-beta.12',
				extensions: { desktop: desktopMember('0.1.0-beta.12') },
			},
			offeredFeed: {
				version: '0.1.0-beta.18',
				extensions: { desktop: desktopMember('0.1.0-beta.18') },
			},
			runInstaller: () => ({ status: 2, stdout: '', stderr: 'unknown option', document: null }),
		}),
		{ seeded: false },
	);

	// Release >= 0.1.0-beta.14 MUST support --installer-window; exit 2 fails closed.
	assert.throws(
		() =>
			seedWindowStore({
				...baseArgs,
				olderFeed: {
					version: '0.1.0-beta.17',
					extensions: { desktop: desktopMember('0.1.0-beta.17') },
				},
				offeredFeed: {
					version: '0.1.0-beta.18',
					extensions: { desktop: desktopMember('0.1.0-beta.18') },
				},
				runInstaller: () => ({ status: 2, stdout: '', stderr: 'unknown option', document: null }),
			}),
		StepFailure,
	);

	// Supported release seeds desktop/ with exact argv and checks active.json; a window up to
	// 0.1.0-beta.24 knows the helper only as --installer-window-v1.
	let seenArgv;
	const seeded = seedWindowStore({
		...baseArgs,
		olderFeed: {
			version: '0.1.0-beta.17',
			extensions: { desktop: desktopMember('0.1.0-beta.17') },
		},
		offeredFeed: {
			version: '0.1.0-beta.18',
			extensions: { desktop: desktopMember('0.1.0-beta.18') },
		},
		runInstaller: (_program, argv) => {
			seenArgv = argv;
			return {
				status: 0,
				stdout: '',
				stderr: '',
				document: { schema: 'orchbee.install', result: { activated: true } },
			};
		},
	});
	assert.deepEqual(seenArgv, ['--installer-window-v1']);
	assert.deepEqual(seeded, {
		seeded: true,
		olderVersion: '0.1.0-beta.17',
		expectedVersion: '0.1.0-beta.18',
	});
	assert.deepEqual(activeChecked, ['0.1.0-beta.17']);
	// A later window reads the current spelling.
	seedWindowStore({
		...baseArgs,
		olderFeed: {
			version: '0.1.0-beta.25',
			extensions: { desktop: desktopMember('0.1.0-beta.25') },
		},
		offeredFeed: {
			version: '0.1.0-beta.26',
			extensions: { desktop: desktopMember('0.1.0-beta.26') },
		},
		runInstaller: (_program, argv) => {
			seenArgv = argv;
			return {
				status: 0,
				stdout: '',
				stderr: '',
				document: { schema: 'orchbee.install', result: { activated: true } },
			};
		},
	});
	assert.deepEqual(seenArgv, ['--installer-window']);
});

test('parseGateArguments and probeFacts validate gate inputs and gapless probe output', () => {
	const workDir = path.resolve(tmpdir(), 'gate-work');
	const feedFile = path.resolve(tmpdir(), 'orchbee-0.1.0-macos-aarch64.gate.feed.json');
	const parsed = parseGateArguments(
		[
			'v0.1.0-beta.18',
			'--release',
			'https://github.com/owner/orchbee/releases/download/v0.1.0-beta.18/',
			'--feed',
			feedFile,
			'--work',
			workDir,
		],
		'usage',
	);
	assert.equal(parsed.older, '0.1.0-beta.18');
	assert.equal(parsed.stem, 'orchbee');
	assert.throws(
		() =>
			parseGateArguments(
				[
					'v0.1.0-beta.18',
					'--release',
					'http://127.0.0.1:8080/',
					'--feed',
					feedFile,
					'--work',
					workDir,
				],
				'usage',
			),
		UsageError,
	);

	assert.deepEqual(
		probeFacts([
			{ pid: 4242, number: 1 },
			{ pid: 4242, number: 2 },
			{ pid: 4242, number: 3 },
		]),
		{ pid: 4242, last: 3 },
	);
	assert.throws(
		() =>
			probeFacts([
				{ pid: 4242, number: 1 },
				{ pid: 4242, number: 3 },
			]),
		/jump from 1 to 3/u,
	);
	assert.throws(
		() =>
			probeFacts([
				{ pid: 4242, number: 1 },
				{ pid: 9999, number: 2 },
			]),
		/processes 4242, 9999/u,
	);
});

test('a run reuses a passed run of its own commit, or waits only for an earlier one building it', () => {
	const commit = 'a'.repeat(40);
	const passed = {
		id: 7,
		title: 'Release 0.1.0-beta.25 rehearsal',
		headSha: commit,
		status: 'completed',
		conclusion: 'success',
		joined: true,
	};
	const decide = (runs) => reuseDecision({ runs, version: '0.1.0-beta.25', commit, current: 9 });
	const running = { status: 'in_progress', conclusion: null, joined: false };
	assert.deepEqual(
		decide([
			passed,
			{ ...passed, id: 8, title: 'Release candidate' },
			{ ...passed, id: 6, ...running },
		]),
		{ reuse: 8 },
	);
	assert.deepEqual(decide([{ ...passed, title: 'Release v0.1.0-beta.25' }]), { reuse: 7 });
	for (const other of [
		{ headSha: 'b'.repeat(40) },
		{ title: 'Release 0.1.0-beta.24 rehearsal' },
		{ title: 'Release dry run' },
		{ conclusion: 'failure' },
		{ joined: false },
		{ id: 9 },
	])
		assert.deepEqual(decide([{ ...passed, ...other }]), {}, JSON.stringify(other));
	assert.deepEqual(decide([{ ...passed, ...running }]), { wait: 7 });
	assert.deepEqual(decide([{ ...passed, ...running, id: 10 }]), {});
});

test('Verify counts as passed once any run of the commit passed', () => {
	const done = (conclusion) => ({ status: 'completed', conclusion });
	assert.equal(verifyState([done('failure'), done('success')]), 'passed');
	assert.equal(
		verifyState([done('failure'), { status: 'in_progress', conclusion: null }]),
		'running',
	);
	assert.equal(verifyState([done('failure'), done('cancelled')]), 'failed');
	assert.equal(verifyState([]), 'missing');
});
