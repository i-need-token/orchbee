// Gathers one platform's release artifacts from a finished installer build into one directory, and
// checks such a directory after it has been moved, as the release workflow does between runners.
//
//   node scripts/release-artifacts.mjs collect --profile <test|release> --output <absolute new directory>
//     [--repository-url https://<host>/<owner>/<name>]
//   node scripts/release-artifacts.mjs check <directory>
//
// `collect` reads what `pnpm desktop:installer:<profile>` left under apps/desktop/out/<profile>/: the
// packaged application's bundled runtime, which that build made a signed release directory, and the
// setup file. It writes, without signing the release directory again:
//
//   <stem>-<version>-<platform>/   the signed release directory, every file checked against release.json
//   <stem>-<version>-<platform>.zip  that directory as one top-level folder, for the command line alone
//   <stem>-<version>-<platform>.feed.json  the update feed document naming that archive
//                                  (scripts/release-feed.mjs)
//   <stem>-<version>-<platform>.feed.json.sig  test profile only: its signature with the test key;
//                                  the release path signs with the release key when it publishes
//   <setup name>.exe               the installer
//   <npm tarballs>                 the folders of scripts/package-npm.mjs, packed by `npm pack`
//   SHA256SUMS                     `<64 hex>  <name>` and LF for every other top-level file except the
//                                  feed signature, in name order
//
// The release profile's feed names its archive by the GitHub release asset address of the run's
// repository (GITHUB_SERVER_URL and GITHUB_REPOSITORY) or of `--repository-url`; the test profile's
// by the bare archive name, resolved beside the feed. In a GitHub Actions run the npm packages name
// the run's repository, which npm provenance requires.
//
// `check` confirms SHA256SUMS names exactly the other top-level files and that each matches, that
// every release directory still matches its manifest, and that each feed document follows the
// reader's rules and matches its archive and release directory. A feed signature, the one file
// SHA256SUMS does not list, must verify with a key of the build's table: RELEASE_KEYS for the
// release stem, TEST_KEYS otherwise. Nothing is published or sent anywhere.
//
// Exit codes: 0 written or intact; 1 a missing, changed or unexpected file; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	copyFileSync,
	existsSync,
	lstatSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	readdirSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { copyRelease, packageNpm, readRelease } from './package-npm.mjs';
import { feedDocument, feedName, readFeed, sha256, writtenForm } from './release-feed.mjs';
import {
	FEED_DOMAIN,
	keyTable,
	loadKey,
	signFeed,
	verifySignature,
} from './sign-release-manifest.mjs';

const repository = fileURLToPath(new URL('..', import.meta.url));
const signatureSource = 'apps/orchbee/src/cli/update/signature.rs';
// The repository's public test key; RELEASE_KEYS never trusts it.
const testSigningKey = path.join(
	repository,
	...'apps/orchbee/src/cli/update/tests/fixtures/test-only-signing-key.pem'.split('/'),
);
const releaseStem = 'orchbee';
const testStem = /^ob-test-[0-9a-f]{16}$/u;
const checksumsName = 'SHA256SUMS';
const feedSignatureSuffix = '.feed.json.sig';
const checksumLine = /^([0-9a-f]{64}) {2}([^/\\\r\n]+)$/u;
const usage =
	'Usage: node scripts/release-artifacts.mjs collect --profile <test|release> --output <absolute new directory> [--repository-url https://<host>/<owner>/<name>]\n' +
	'       node scripts/release-artifacts.mjs check <directory>';

class UsageError extends Error {}

function fileDigest(filename) {
	return createHash('sha256').update(readFileSync(filename)).digest('hex');
}

function byName(left, right) {
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

/** The one entry of `directory` that `accept` admits, or an error naming what was found. */
function single(directory, accept, description) {
	const found = readdirSync(directory, { withFileTypes: true }).filter(accept);
	if (found.length !== 1)
		throw new Error(`Expected one ${description} in ${directory}, found ${found.length}`);
	return path.join(directory, found[0].name);
}

/** The version a signed release directory names, before the full manifest check. */
function manifestVersion(directory) {
	const manifest = JSON.parse(readFileSync(path.join(directory, 'release.json'), 'utf8'));
	if (typeof manifest?.version !== 'string')
		throw new Error(`${directory} holds no release version`);
	return manifest.version;
}

/** Runs `npm pack` for one package folder into `destination`. */
function npmPack(folder, destination) {
	const windows = process.platform === 'win32';
	// npm is a command script on Windows, which only a shell starts; the paths are quoted for it.
	const quote = (value) => (windows ? `"${value}"` : value);
	const result = spawnSync(
		windows ? 'npm.cmd' : 'npm',
		['pack', quote(folder), '--pack-destination', quote(destination)],
		{ shell: windows, stdio: ['ignore', 'inherit', 'inherit'], windowsHide: true },
	);
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) throw new Error(`npm pack ${folder} exited ${result.status}`);
}

/**
 * Writes `<directory>.zip` beside a Windows release directory, holding it as its one top-level
 * folder, with the system's bsdtar.
 */
function archiveRelease(directory) {
	const archive = `${directory}.zip`;
	const tar = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');
	const result = spawnSync(
		tar,
		[
			'--format',
			'zip',
			'-c',
			'-f',
			archive,
			'-C',
			path.dirname(directory),
			path.basename(directory),
		],
		{ stdio: ['ignore', 'inherit', 'inherit'], windowsHide: true },
	);
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) throw new Error(`tar ${archive} exited ${result.status}`);
}

/** The repository of the GitHub Actions run, or undefined outside one. */
function runRepository() {
	const { GITHUB_SERVER_URL: server, GITHUB_REPOSITORY: name } = process.env;
	return server && name ? `${server}/${name}` : undefined;
}

/**
 * Writes SHA256SUMS over every other top-level regular file of `directory`, except a feed
 * signature: the release path signs the feed after SHA256SUMS exists, and a test build does the
 * same, so both list the same files.
 */
function writeChecksums(directory) {
	const names = readdirSync(directory, { withFileTypes: true })
		.filter(
			(entry) =>
				entry.isFile() && entry.name !== checksumsName && !entry.name.endsWith(feedSignatureSuffix),
		)
		.map((entry) => entry.name)
		.toSorted(byName);
	const lines = names.map((name) => `${fileDigest(path.join(directory, name))}  ${name}\n`);
	writeFileSync(path.join(directory, checksumsName), lines.join(''), { flag: 'wx' });
}

/** The key table of the build `stem` names, read from the Rust source the updater compiles. */
function buildTable(stem) {
	const table = keyTable(
		readFileSync(path.join(repository, ...signatureSource.split('/')), 'utf8'),
		stem === releaseStem ? 'RELEASE_KEYS' : 'TEST_KEYS',
	);
	return table ?? [];
}

/**
 * The problems of the feed document `name` in `directory`: the reader's rules and written form, and
 * its members against the archive and the signed release directory beside it. The release stem
 * names its archive by the GitHub release address, a test stem by the bare archive name.
 */
function checkFeed(directory, name) {
	const bytes = readFileSync(path.join(directory, name));
	const { feed, problems } = readFeed(bytes);
	if (problems.length > 0) return problems.map((problem) => `${name}: ${problem}`);
	const own = [];
	if (!writtenForm(bytes, feed)) own.push('is not in its written form');
	const base = name.slice(0, -'.feed.json'.length);
	const suffix = `-${feed.version}-${feed.platform}`;
	const stem = base.endsWith(suffix) ? base.slice(0, -suffix.length) : '';
	if (stem !== releaseStem && !testStem.test(stem))
		own.push(`does not name its stem, version ${feed.version} and platform ${feed.platform}`);
	const archiveName = `${base}.zip`;
	const archive = path.join(directory, archiveName);
	const archiveMetadata = lstatSync(archive, { throwIfNoEntry: false });
	if (archiveMetadata === undefined || !archiveMetadata.isFile()) {
		own.push(`names ${archiveName}, which is missing`);
	} else {
		if (archiveMetadata.size !== feed.archive.size)
			own.push(
				`archive.size is ${feed.archive.size}, but ${archiveName} has ${archiveMetadata.size} bytes`,
			);
		if (fileDigest(archive) !== feed.archive.sha256)
			own.push(`archive.sha256 does not match ${archiveName}`);
	}
	const expectedUrl =
		stem === releaseStem
			? new RegExp(
					`^https://[^/]+/[^/]+/[^/]+/releases/download/v${escapeRegExp(feed.version)}/${escapeRegExp(archiveName)}$`,
					'u',
				)
			: new RegExp(`^${escapeRegExp(archiveName)}$`, 'u');
	if (!expectedUrl.test(feed.archive.url))
		own.push(
			stem === releaseStem
				? `archive.url is not the GitHub release address of ${archiveName}`
				: `archive.url is not the bare name ${archiveName}`,
		);
	const manifestPath = path.join(directory, base, 'release.json');
	if (!existsSync(manifestPath)) {
		own.push(`has no release directory ${base}`);
	} else {
		const manifestBytes = readFileSync(manifestPath);
		if (sha256(manifestBytes) !== feed.manifest.sha256)
			own.push(`manifest.sha256 does not match ${base}/release.json`);
		let manifest;
		try {
			manifest = JSON.parse(manifestBytes.toString('utf8'));
		} catch {
			manifest = {};
		}
		for (const member of ['channel', 'platform', 'version', 'buildId'])
			if (manifest[member] !== feed[member])
				own.push(`${member} is ${feed[member]}, but ${base}/release.json says ${manifest[member]}`);
	}
	const signature = path.join(directory, `${name}.sig`);
	if (existsSync(signature)) {
		const { outcome } = verifySignature(
			bytes,
			readFileSync(signature),
			buildTable(stem),
			FEED_DOMAIN,
		);
		if (outcome !== 'trusted')
			own.push(`.sig does not verify with a key of the build's table (${outcome})`);
	}
	return own.map((problem) => `${name}: ${problem}`);
}

function escapeRegExp(text) {
	return text.replaceAll(/[.*+?^${}()|[\]\\]/gu, '\\$&');
}

/** Every problem `check` finds in `directory`; none means intact. */
export function checkArtifacts(directory) {
	const problems = [];
	const listed = new Map();
	const sums = path.join(directory, checksumsName);
	if (!existsSync(sums)) return [`${checksumsName} is missing`];
	const text = readFileSync(sums, 'utf8');
	if (text.length > 0 && !text.endsWith('\n')) problems.push(`${checksumsName} must end with LF`);
	for (const line of text.split('\n').slice(0, -1)) {
		const match = checksumLine.exec(line);
		if (match === null) problems.push(`${checksumsName} has a malformed line: ${line}`);
		else listed.set(match[2], match[1]);
	}
	const names = [...listed.keys()];
	if (names.join('\n') !== names.toSorted(byName).join('\n'))
		problems.push(`${checksumsName} is not in name order`);
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		if (entry.name === checksumsName) continue;
		const filename = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			if (existsSync(path.join(filename, 'release.json'))) {
				problems.push(...checkReleaseDirectory(filename));
			} else problems.push(`${entry.name} is an unexpected directory`);
		} else if (!entry.isFile()) problems.push(`${entry.name} is not a regular file`);
		else if (entry.name.endsWith(feedSignatureSuffix) && !listed.has(entry.name)) {
			// The one kind of file SHA256SUMS does not list: the publish step signs the feed after
			// SHA256SUMS exists. checkFeed verifies it.
			if (!existsSync(filename.slice(0, -'.sig'.length)))
				problems.push(`${entry.name} signs no feed document`);
		} else if (!listed.has(entry.name)) problems.push(`${entry.name} is not in ${checksumsName}`);
		else if (fileDigest(filename) !== listed.get(entry.name))
			problems.push(`${entry.name} does not match ${checksumsName}`);
		if (entry.isFile() && entry.name.endsWith('.feed.json'))
			problems.push(...checkFeed(directory, entry.name));
		listed.delete(entry.name);
	}
	for (const name of listed.keys()) problems.push(`${name} is in ${checksumsName} but missing`);
	return problems;
}

/** The problems of one signed release directory against its own manifest. */
function checkReleaseDirectory(directory) {
	try {
		const release = readRelease(directory, manifestVersion(directory));
		const problems = [];
		for (const file of release.manifest.files) {
			const filename = path.join(directory, ...file.path.split('/'));
			const metadata = lstatSync(filename, { throwIfNoEntry: false });
			if (metadata === undefined || !metadata.isFile()) problems.push(`${file.path} is missing`);
			else if (metadata.size !== file.size || fileDigest(filename) !== file.sha256)
				problems.push(`${file.path} does not match release.json`);
		}
		return problems.map((problem) => `${path.basename(directory)}: ${problem}`);
	} catch (error) {
		return [`${path.basename(directory)}: ${error.message}`];
	}
}

/**
 * The address a feed names its archive by: the GitHub release asset for the release profile, the
 * bare archive name for the test profile, so a test feed works from a folder.
 */
function archiveAddress({ profile, repositoryUrl, version, archiveName }) {
	if (profile === 'test') return archiveName;
	if (repositoryUrl === undefined || !/^https:\/\/[^/?#]+\/[^/?#]+\/[^/?#]+$/u.test(repositoryUrl))
		throw new Error(
			'The release profile names its archive by the GitHub release address: run it in GitHub Actions or pass --repository-url https://github.com/<owner>/<name>',
		);
	return `${repositoryUrl}/releases/download/v${version}/${archiveName}`;
}

/** Writes the feed document beside the archive of `releaseDirectory`, and signs it for a test build. */
function writeFeed({ profile, archiveUrl, release, releaseDirectory }) {
	const archive = `${releaseDirectory}.zip`;
	const archiveBytes = readFileSync(archive);
	const { manifest } = release;
	const document = feedDocument({
		channel: manifest.channel,
		platform: manifest.platform,
		version: manifest.version,
		buildId: manifest.buildId,
		manifestSha256: sha256(readFileSync(path.join(releaseDirectory, 'release.json'))),
		archive: {
			url: archiveUrl,
			size: archiveBytes.length,
			sha256: sha256(archiveBytes),
		},
	});
	const feed = path.join(
		path.dirname(releaseDirectory),
		feedName(release.stem, manifest.version, manifest.platform),
	);
	writeFileSync(feed, document, { flag: 'wx' });
	// The release path signs with the release key in its publish step, after this run.
	if (profile === 'test') signFeed(feed, [loadKey(readFileSync(testSigningKey))]);
}

/** Gathers the artifacts of the `profile` installer build into `output`. */
function collect({ profile, output, repositoryUrl = runRepository() }) {
	if (!path.isAbsolute(output)) throw new UsageError('The output directory must be absolute');
	if (existsSync(output)) throw new Error(`Refusing to write into an existing ${output}`);
	const built = path.join(repository, 'apps', 'desktop', 'out', profile);
	const application = single(
		built,
		(entry) => entry.isDirectory() && entry.name.endsWith(`-${process.platform}-${process.arch}`),
		'packaged application',
	);
	const bundled = path.join(application, 'resources', 'cohort');
	const version = manifestVersion(bundled);
	const release = readRelease(bundled, version);
	if ((profile === 'release') !== (release.stem === 'orchbee'))
		throw new Error(`The ${profile} profile cannot carry the ${release.stem} programs`);
	const setup = single(
		path.join(built, 'installer'),
		(entry) => entry.isFile() && entry.name.endsWith('-setup.exe'),
		'setup file',
	);
	const releaseName = `${release.stem}-${version}-${release.manifest.platform}`;
	// Settled before anything is written, so a missing repository leaves no partial output.
	const archiveUrl = archiveAddress({
		profile,
		repositoryUrl,
		version,
		archiveName: `${releaseName}.zip`,
	});
	mkdirSync(output, { recursive: true });
	const releaseDirectory = path.join(output, releaseName);
	copyRelease(bundled, release, releaseDirectory);
	if (release.platform.os === 'win32') {
		archiveRelease(releaseDirectory);
		writeFeed({ profile, archiveUrl, release, releaseDirectory });
	}
	copyFileSync(setup, path.join(output, path.basename(setup)));
	// package-npm writes into a new directory; its folders are packed and then dropped.
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-release-npm-'));
	try {
		const folders = packageNpm({
			version,
			output: path.join(scratch, 'packages'),
			releases: [releaseDirectory],
			repositoryUrl: runRepository(),
		});
		for (const folder of folders) npmPack(folder, output);
	} finally {
		rmSync(scratch, { force: true, recursive: true });
	}
	writeChecksums(output);
	const problems = checkArtifacts(output);
	if (problems.length > 0) throw new Error(problems.join('\n'));
	return readdirSync(output).toSorted(byName);
}

function parseArguments(argv) {
	const [command, ...rest] = argv;
	if (command === 'check' && rest.length === 1) return { command, directory: rest[0] };
	if (command === 'collect') {
		const options = {};
		for (let index = 0; index < rest.length; index += 2) {
			const [option, value] = [rest[index], rest[index + 1]];
			if (!['--profile', '--output', '--repository-url'].includes(option) || value === undefined)
				throw new UsageError(usage);
			const key = option.slice(2);
			if (options[key] !== undefined) throw new UsageError(usage);
			options[key] = value;
		}
		if (!['test', 'release'].includes(options.profile) || options.output === undefined)
			throw new UsageError(usage);
		const { profile, output, 'repository-url': repositoryUrl } = options;
		return { command, profile, output, ...(repositoryUrl === undefined ? {} : { repositoryUrl }) };
	}
	throw new UsageError(usage);
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	try {
		const options = parseArguments(process.argv.slice(2));
		if (options.command === 'collect') {
			for (const name of collect(options)) process.stdout.write(`${name}\n`);
		} else {
			const problems = checkArtifacts(path.resolve(options.directory));
			for (const problem of problems) process.stderr.write(`${problem}\n`);
			if (problems.length === 0) process.stdout.write(`${options.directory} is intact\n`);
			process.exitCode = problems.length === 0 ? 0 : 1;
		}
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	}
}
