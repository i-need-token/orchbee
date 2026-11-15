// Gathers one platform's release artifacts from a finished installer build into one directory, and
// checks such a directory after it has been moved, as the release workflow does between runners.
//
//   node scripts/release-artifacts.mjs collect --profile <test|release> --output <absolute new directory>
//     [--repository-url https://<host>/<owner>/<name>]
//   node scripts/release-artifacts.mjs check <directory>
//
// `collect` reads what `pnpm desktop:installer:<profile>` left under apps/desktop/out/<profile>/: the
// packaged application with its window file list, its bundled runtime, which that build made a
// signed release directory, and the setup file. It writes, without signing the release directory
// again:
//
//   <stem>-<version>-<platform>/   the signed release directory, every file checked against release.json
//   <stem>-<version>-<platform>.zip  that directory as one top-level folder, for the command line alone
//   <stem>-desktop-<version>-<platform>.zip  the packaged application, `desktop-files.json` included,
//                                  as one top-level folder of that name: the window archive
//                                  (scripts/desktop-files.mjs)
//   <stem>-<version>-<platform>.feed.json  the update feed document naming the runtime archive and,
//                                  in `extensions.desktop`, the window archive (scripts/release-feed.mjs)
//   <stem>-<version>-<platform>.feed.json.sig  test profile only: its signature with the test key;
//                                  the release path's feed job signs with the release key
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
// reader's rules and matches its archive and release directory. A feed's `extensions.desktop`
// must name the window archive of the feed's version beside it, by the same kind of address as the
// runtime archive, and that archive must hold one top-level folder whose list matches the entry's
// digest and names exactly the folder's other files, the bundled `release.json` being the feed's
// own. A feed signature, which SHA256SUMS
// does not list, must verify with a key of the build's table: RELEASE_KEYS for the release stem,
// TEST_KEYS otherwise. The release gate's copy of the feed, `<...>.gate.feed.json` with its `.sig`,
// placed beside the archive after the feed job, is not listed either: it must carry its signature
// and name the archive by its bare name, and is checked like the feed otherwise. Nothing is
// published or sent anywhere.
//
// Exit codes: 0 written or intact; 1 a missing, changed or unexpected file; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	copyFileSync,
	cpSync,
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

import {
	DESKTOP_FILES,
	checkDesktopArchive,
	desktopReleaseName,
	readDesktopFiles,
} from './desktop-files.mjs';
import { copyRelease, packageNpm, readRelease } from './package-npm.mjs';
import {
	GATE_FEED_SUFFIX,
	desktopProblems,
	feedDocument,
	feedName,
	readFeed,
	sha256,
	writtenForm,
} from './release-feed.mjs';
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

/**
 * The bytes of the window list apps/desktop/scripts/package.mjs wrote into the packaged
 * application, after checking it names `version`; the archive check confirms it against the files.
 */
function readWindowList(application, version) {
	const filename = path.join(application, DESKTOP_FILES);
	if (!existsSync(filename))
		throw new Error(`${application} holds no ${DESKTOP_FILES}; package it again on this commit`);
	const bytes = readFileSync(filename);
	const { list, problems } = readDesktopFiles(bytes);
	if (problems.length > 0) throw new Error(problems.join('\n'));
	if (list.version !== version)
		throw new Error(`${DESKTOP_FILES} names version ${list.version}, not ${version}`);
	return bytes;
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
 * Writes the zip `archive`, by default `<directory>.zip` beside a Windows release directory,
 * holding the directory as its one top-level folder, with the system's bsdtar.
 */
function archiveRelease(directory, archive = `${directory}.zip`) {
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

/**
 * Writes the window archive `<output>/<name>.zip`: the packaged application as one top-level folder
 * `name`. Windows' bsdtar cannot rename stored paths, so a scratch copy carries the folder's name;
 * `check` then confirms every file against the list.
 */
function archiveWindow(application, output, name) {
	const archive = path.join(output, `${name}.zip`);
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-release-window-'));
	try {
		const folder = path.join(scratch, name);
		cpSync(application, folder, { recursive: true, errorOnExist: true, verbatimSymlinks: true });
		archiveRelease(folder, archive);
	} finally {
		rmSync(scratch, { force: true, recursive: true });
	}
	return archive;
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
 * its members against the archive, the window archive and the signed release directory beside it.
 * The release stem names its archives by the GitHub release address, a test stem and every gate
 * copy by the bare archive name; a gate copy must also carry its signature.
 */
function checkFeed(directory, name) {
	const bytes = readFileSync(path.join(directory, name));
	const { feed, problems } = readFeed(bytes);
	if (problems.length > 0) return problems.map((problem) => `${name}: ${problem}`);
	const own = [];
	if (!writtenForm(bytes, feed)) own.push('is not in its written form');
	const gate = name.endsWith(GATE_FEED_SUFFIX);
	const base = name.slice(0, -(gate ? GATE_FEED_SUFFIX : '.feed.json').length);
	const suffix = `-${feed.version}-${feed.platform}`;
	const stem = base.endsWith(suffix) ? base.slice(0, -suffix.length) : '';
	if (stem !== releaseStem && !testStem.test(stem))
		own.push(`does not name its stem, version ${feed.version} and platform ${feed.platform}`);
	// The release feed names its archives by GitHub release address, every other by bare name.
	const releaseTag = stem === releaseStem && !gate ? `v${feed.version}` : undefined;
	own.push(...checkArchive(directory, feed.archive, `${base}.zip`, 'archive', releaseTag).problems);
	if (feed.extensions?.desktop !== undefined)
		own.push(...checkWindow(directory, feed, stem, releaseTag));
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
	} else if (gate) own.push(`has no ${name}.sig: the feed job signs the gate copy with the feed`);
	return own.map((problem) => `${name}: ${problem}`);
}

function escapeRegExp(text) {
	return text.replaceAll(/[.*+?^${}()|[\]\\]/gu, '\\$&');
}

/**
 * The problems of the archive `archiveName` in `directory` against the feed's member `member`,
 * named `label`, and its bytes when present. With `releaseTag` the member names it by the GitHub
 * release address of that tag, otherwise by its bare name.
 */
function checkArchive(directory, member, archiveName, label, releaseTag) {
	const problems = [];
	const archive = path.join(directory, archiveName);
	const metadata = lstatSync(archive, { throwIfNoEntry: false });
	let bytes;
	if (metadata === undefined || !metadata.isFile()) {
		problems.push(`names ${archiveName}, which is missing`);
	} else {
		bytes = readFileSync(archive);
		if (bytes.length !== member.size)
			problems.push(
				`${label}.size is ${member.size}, but ${archiveName} has ${bytes.length} bytes`,
			);
		if (sha256(bytes) !== member.sha256)
			problems.push(`${label}.sha256 does not match ${archiveName}`);
	}
	const expectedUrl =
		releaseTag === undefined
			? new RegExp(`^${escapeRegExp(archiveName)}$`, 'u')
			: new RegExp(
					`^https://[^/]+/[^/]+/[^/]+/releases/download/${escapeRegExp(releaseTag)}/${escapeRegExp(archiveName)}$`,
					'u',
				);
	if (!expectedUrl.test(member.url))
		problems.push(
			releaseTag === undefined
				? `${label}.url is not the bare name ${archiveName}`
				: `${label}.url is not the GitHub release address of ${archiveName}`,
		);
	return { bytes, problems };
}

/**
 * The problems of the feed's `extensions.desktop` against the window archive beside it: the entry's
 * rules, the archive's address, size and digest, and its contents against the list it carries, whose
 * digest the entry binds and whose bundled `release.json` is the feed's own.
 */
function checkWindow(directory, feed, stem, releaseTag) {
	const { desktop } = feed.extensions;
	const entryProblems = desktopProblems(desktop);
	if (entryProblems.length > 0) return entryProblems;
	// The tooling writes the window of the feed's own version; an earlier one is not beside it.
	if (desktop.version !== feed.version)
		return [`extensions.desktop.version is ${desktop.version}, not ${feed.version}`];
	const folder = desktopReleaseName(stem, desktop.version, feed.platform);
	const archiveName = `${folder}.zip`;
	const { bytes, problems } = checkArchive(
		directory,
		desktop.archive,
		archiveName,
		'extensions.desktop.archive',
		releaseTag,
	);
	if (bytes === undefined) return problems;
	const window = checkDesktopArchive(bytes, folder);
	problems.push(...window.problems.map((problem) => `${archiveName} ${problem}`));
	if (window.problems.length > 0) return problems;
	if (sha256(window.listBytes) !== desktop.manifest.sha256)
		problems.push(`extensions.desktop.manifest.sha256 does not match ${archiveName}'s list`);
	if (window.list.version !== desktop.version)
		problems.push(`${archiveName}'s list names version ${window.list.version}`);
	const bundled = window.list.files.find((file) => file.path === 'resources/cohort/release.json');
	if (bundled?.sha256 !== feed.manifest.sha256)
		problems.push(`${archiveName} does not bundle the feed's release.json`);
	return problems;
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
			// SHA256SUMS does not list a feed signature: the feed job signs the feed after SHA256SUMS
			// exists. checkFeed verifies it.
			if (!existsSync(filename.slice(0, -'.sig'.length)))
				problems.push(`${entry.name} signs no feed document`);
		} else if (!listed.has(entry.name)) {
			// Nor the gate copy, which the feed job writes after it too; checkFeed checks it.
			if (!entry.name.endsWith(GATE_FEED_SUFFIX))
				problems.push(`${entry.name} is not in ${checksumsName}`);
		} else if (fileDigest(filename) !== listed.get(entry.name))
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

/**
 * Writes the feed document beside the archive of `releaseDirectory`, naming the window archive
 * `desktop.archive` with the window list `desktop.list`, and signs it for a test build.
 */
function writeFeed({ profile, archiveUrl, release, releaseDirectory, desktop }) {
	const archive = `${releaseDirectory}.zip`;
	const archiveBytes = readFileSync(archive);
	const desktopBytes = readFileSync(desktop.archive);
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
		desktop: {
			version: manifest.version,
			manifestSha256: sha256(desktop.list),
			archive: { url: desktop.url, size: desktopBytes.length, sha256: sha256(desktopBytes) },
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
	const desktopName = desktopReleaseName(release.stem, version, release.manifest.platform);
	// Settled before anything is written, so a missing repository or window list leaves no partial
	// output.
	const archiveUrl = archiveAddress({
		profile,
		repositoryUrl,
		version,
		archiveName: `${releaseName}.zip`,
	});
	const desktopUrl = archiveAddress({
		profile,
		repositoryUrl,
		version,
		archiveName: `${desktopName}.zip`,
	});
	const windows = release.platform.os === 'win32';
	const desktopList = windows ? readWindowList(application, version) : undefined;
	mkdirSync(output, { recursive: true });
	const releaseDirectory = path.join(output, releaseName);
	copyRelease(bundled, release, releaseDirectory);
	if (windows) {
		archiveRelease(releaseDirectory);
		const desktopArchive = archiveWindow(application, output, desktopName);
		writeFeed({
			profile,
			archiveUrl,
			release,
			releaseDirectory,
			desktop: { archive: desktopArchive, url: desktopUrl, list: desktopList },
		});
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
