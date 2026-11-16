// Gathers one platform's release artifacts from a finished installer build into one directory, and
// checks such a directory after it has been moved, as the release workflow does between runners.
//
//   node scripts/release-artifacts.mjs collect --profile <test|release> --output <absolute new directory>
//     [--repository-url https://<host>/<owner>/<name>]
//   node scripts/release-artifacts.mjs check <directory>
//   node scripts/release-artifacts.mjs join --output <absolute new directory> <directory>...
//
// `collect` runs on Windows or macOS and reads what `pnpm desktop:installer:<profile>` (on macOS,
// `apps/desktop/scripts/package.mjs --installer`) left under apps/desktop/out/<profile>/: the
// packaged application with its window file list (on macOS beside the bundle), its bundled runtime,
// which that build made a signed release directory, and the installer. `<platform>` is the signed
// manifest's. It writes, without signing the release directory again:
//
//   <stem>-<version>-<platform>/   the signed release directory, every file checked against release.json
//   <stem>-<version>-<platform>.zip  Windows: that directory as one top-level folder, for the command
//                                  line alone
//   <stem>-<version>-<platform>.tar.gz  macOS: the same as a tar.gz (scripts/tar-gzip.mjs); the
//                                  programs release.json names record mode 0755, whatever the copy
//                                  on disk says, since `upload-artifact` drops modes
//   <stem>-desktop-<version>-<platform>.zip  the window archive (scripts/desktop-files.mjs): one
//                                  top-level folder of that name holding the packaged application
//                                  and its `desktop-files.json`; on macOS just the list and the
//                                  signed bundle, links and modes kept
//   <stem>-<version>-<platform>.feed.json  the update feed document naming the runtime archive and,
//                                  in `extensions.desktop`, the window archive
//                                  (scripts/release-feed.mjs)
//   <stem>-<version>-<platform>.feed.json.sig  test profile only: its signature with the test key;
//                                  the release path's feed job signs with the release key
//   <setup name>.exe | <name>.dmg  the installer: Setup on Windows, the disk image on macOS
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
// digest and names exactly the folder's other files (on macOS also its links and execute bits), the
// bundled `release.json` being the feed's own. A feed signature, which SHA256SUMS
// does not list, must verify with a key of the build's table: RELEASE_KEYS for the release stem,
// TEST_KEYS otherwise. The release gate's copy of the feed, `<...>.gate.feed.json` with its `.sig`,
// placed beside the archive after the feed job, is not listed either: it must carry its signature
// and name the archive by its bare name, and is checked like the feed otherwise. A `.tar.gz`
// release archive must hold one top-level folder named like it with exactly the signed release
// directory beside it, each file matching release.json and each program executable; a macOS or
// Linux npm platform package must hold its release the same way under `package/release/`. The
// release directory's own modes are not checked: a moved copy has lost them. Nothing is published
// or sent anywhere.
//
// `join` gathers the checked directories of a release run's platform rows, each with its feed
// signature beside the feed, into the one set the run publishes. One row is copied unchanged.
// Several must carry one stem and version and distinct names, apart from SHA256SUMS and the main npm
// package `<stem>-<version>.tgz`: the main package's files must be the same in every row but for
// the optional dependencies its package.json lists, and the joined set holds it once, listing every
// row's platform package, with SHA256SUMS written again by the rule above. The result must pass
// `check`.
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
import { copyRelease, describeRelease, packageNpm, readRelease } from './package-npm.mjs';
import {
	GATE_FEED_SUFFIX,
	archiveFormat,
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
import { readTarGzip, tarGzip } from './tar-gzip.mjs';

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
const tarGzipSuffix = '.tar.gz';
// The npm platform packages whose programs must stay executable, as `npm pack` names them.
const posixNpmPackage = /-cli-(?:darwin|linux)-(?:arm64|x64)-[^/\\]+\.tgz$/u;
const npmRelease = 'package/release/';
const npmManifest = 'package/package.json';
const usage =
	'Usage: node scripts/release-artifacts.mjs collect --profile <test|release> --output <absolute new directory> [--repository-url https://<host>/<owner>/<name>]\n' +
	'       node scripts/release-artifacts.mjs check <directory>\n' +
	'       node scripts/release-artifacts.mjs join --output <absolute new directory> <directory>...';

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
function readWindowList(application, version, macos) {
	const filename = path.join(application, DESKTOP_FILES);
	if (!existsSync(filename))
		throw new Error(`${application} holds no ${DESKTOP_FILES}; package it again on this commit`);
	const bytes = readFileSync(filename);
	const { list, problems } = readDesktopFiles(bytes, { macos });
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
 * Writes the zip `archive` holding the Windows directory `directory` as its one top-level folder,
 * with the system's bsdtar.
 */
function archiveRelease(directory, archive) {
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

/** A file's modification time in whole seconds, as a tar entry records it. */
function seconds(metadata) {
	return Math.floor(metadata.mtimeMs / 1000);
}

/**
 * Writes the tar.gz `archive` holding the macOS or Linux release directory `directory` as its one
 * top-level folder. Directories and the programs `release.json` names record mode 0755, other files
 * 0755 when their owner may run them and 0644 otherwise: the programs' bit comes from the manifest,
 * since a copy that went through `upload-artifact` has lost it.
 */
function archiveTarGzip(directory, release, archive) {
	const top = path.basename(directory);
	const programs = new Set(release.programs);
	const entries = [
		{ path: top, directory: true, mode: 0o755, mtime: seconds(lstatSync(directory)) },
	];
	const found = readdirSync(directory, { recursive: true, withFileTypes: true })
		.map((entry) => path.relative(directory, path.join(entry.parentPath, entry.name)))
		.map((relative) => relative.split(path.sep).join('/'))
		.toSorted(byName);
	for (const relative of found) {
		const filename = path.join(directory, ...relative.split('/'));
		const metadata = lstatSync(filename);
		const entry = { path: `${top}/${relative}`, mtime: seconds(metadata) };
		if (metadata.isDirectory()) entries.push({ ...entry, directory: true, mode: 0o755 });
		else if (metadata.isFile()) {
			const runnable = programs.has(relative) || (metadata.mode & 0o100) !== 0;
			entries.push({ ...entry, mode: runnable ? 0o755 : 0o644, bytes: readFileSync(filename) });
		} else throw new Error(`${filename} is neither a file nor a directory`);
	}
	writeFileSync(archive, tarGzip(entries), { flag: 'wx' });
}

/**
 * Writes the window archive `<output>/<name>.zip`: the packaged application as one top-level folder
 * `name`. Windows' bsdtar cannot rename stored paths, so a scratch copy carries the folder's name;
 * `check` then confirms every file against the list. On macOS the folder holds the list and the
 * signed bundle `bundle` alone, and the system bsdtar keeps the bundle's links and modes and leaves
 * out macOS metadata, so no AppleDouble entry joins them.
 */
function archiveWindow(application, output, name, bundle) {
	const archive = path.join(output, `${name}.zip`);
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-release-window-'));
	const copy = { recursive: true, errorOnExist: true, verbatimSymlinks: true };
	try {
		const folder = path.join(scratch, name);
		if (bundle === undefined) {
			cpSync(application, folder, copy);
			archiveRelease(folder, archive);
		} else {
			mkdirSync(folder);
			for (const entry of [DESKTOP_FILES, bundle])
				cpSync(path.join(application, entry), path.join(folder, entry), copy);
			const result = spawnSync(
				'/usr/bin/tar',
				['--format', 'zip', '--no-mac-metadata', '-c', '-f', archive, '-C', scratch, name],
				{ stdio: ['ignore', 'inherit', 'inherit'] },
			);
			if (result.error !== undefined) throw result.error;
			if (result.status !== 0) throw new Error(`tar ${archive} exited ${result.status}`);
		}
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
	// readFeed admits only a platform with an archive format, which is also its extension.
	const archiveName = `${base}.${archiveFormat(feed.platform)}`;
	own.push(...checkArchive(directory, feed.archive, archiveName, 'archive', releaseTag).problems);
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
	const macos = feed.platform.startsWith('macos-');
	const window = checkDesktopArchive(bytes, folder, { macos });
	problems.push(...window.problems.map((problem) => `${archiveName} ${problem}`));
	if (window.problems.length > 0) return problems;
	if (sha256(window.listBytes) !== desktop.manifest.sha256)
		problems.push(`extensions.desktop.manifest.sha256 does not match ${archiveName}'s list`);
	if (window.list.version !== desktop.version)
		problems.push(`${archiveName}'s list names version ${window.list.version}`);
	// A macOS bundle keeps its runtime under Contents/Resources.
	const bundledPath = macos
		? /^[^/]+\.app\/Contents\/Resources\/cohort\/release\.json$/u
		: /^resources\/cohort\/release\.json$/u;
	const bundled = window.list.files.find((file) => bundledPath.test(file.path));
	if (bundled?.sha256 !== feed.manifest.sha256)
		problems.push(`${archiveName} does not bundle the feed's release.json`);
	return problems;
}

/**
 * The problems of a release tree read from an archive, `files` mapping each path below its top to
 * `{ mode, bytes }`: its `release.json`, equal to `manifestBytes` when given, and signature; every
 * file the manifest lists, by size and SHA-256, and nothing else; each program executable off
 * Windows.
 */
function releaseTreeProblems(files, manifestBytes) {
	const manifestEntry = files.get('release.json');
	if (manifestEntry === undefined) return ['holds no release.json'];
	const problems = [];
	if (manifestBytes !== undefined && !manifestEntry.bytes.equals(manifestBytes))
		problems.push('holds another release.json than the release directory beside it');
	if (!files.has('release.json.sig')) problems.push('holds no release.json.sig');
	let release;
	try {
		const manifest = JSON.parse(manifestEntry.bytes.toString('utf8'));
		release = { manifest, ...describeRelease(manifest, manifest?.version, 'its release.json') };
	} catch (error) {
		return [
			...problems,
			error instanceof SyntaxError ? 'holds a release.json that is not JSON' : error.message,
		];
	}
	const listed = new Set(['release.json', 'release.json.sig']);
	for (const file of release.manifest.files) {
		listed.add(file.path);
		const entry = files.get(file.path);
		if (entry === undefined) problems.push(`holds no ${file.path}`);
		else if (entry.bytes.length !== file.size || sha256(entry.bytes) !== file.sha256)
			problems.push(`holds ${file.path} with other bytes than release.json lists`);
	}
	for (const name of files.keys())
		if (!listed.has(name)) problems.push(`holds ${name}, which release.json omits`);
	if (release.platform.os !== 'win32')
		for (const program of release.programs)
			if (files.has(program) && (files.get(program).mode & 0o111) !== 0o111)
				problems.push(`holds ${program} without its executable bit`);
	return problems;
}

/**
 * The files of the tar.gz `name` in `directory` below the folder `top`, as `releaseTreeProblems`
 * reads them, and the problems of its other entries. With `whole`, every entry must lie below `top`.
 */
function archiveTree(directory, name, top, whole) {
	const files = new Map();
	const problems = [];
	for (const entry of readTarGzip(readFileSync(path.join(directory, name)))) {
		if (whole && entry.type === 'directory' && `${entry.path}/` === top) continue;
		if (!entry.path.startsWith(top)) {
			if (whole) problems.push(`holds ${entry.path} outside the one top-level folder ${top}`);
			continue;
		}
		const relative = entry.path.slice(top.length);
		if (entry.type !== 'file') continue;
		if (files.has(relative)) problems.push(`holds ${relative} twice`);
		files.set(relative, entry);
	}
	return { files, problems };
}

/**
 * The problems of the release archive `name`, a tar.gz: one top-level folder named like it, holding
 * the signed release directory beside it with every program executable.
 */
function checkReleaseArchive(directory, name) {
	const base = name.slice(0, -tarGzipSuffix.length);
	const manifestPath = path.join(directory, base, 'release.json');
	try {
		if (!existsSync(manifestPath)) return [`${name} has no release directory ${base} beside it`];
		const { files, problems } = archiveTree(directory, name, `${base}/`, true);
		problems.push(...releaseTreeProblems(files, readFileSync(manifestPath)));
		return problems.map((problem) => `${name} ${problem}`);
	} catch (error) {
		return [`${name}: ${error.message}`];
	}
}

/** The problems of the macOS or Linux npm platform package `name`: its release, programs executable. */
function checkPlatformPackage(directory, name) {
	try {
		const { files, problems } = archiveTree(directory, name, npmRelease, false);
		problems.push(...releaseTreeProblems(files, undefined));
		return problems.map((problem) => `${name} ${problem}`);
	} catch (error) {
		return [`${name}: ${error.message}`];
	}
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
		if (entry.isFile() && entry.name.endsWith(tarGzipSuffix))
			problems.push(...checkReleaseArchive(directory, entry.name));
		if (entry.isFile() && posixNpmPackage.test(entry.name))
			problems.push(...checkPlatformPackage(directory, entry.name));
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
 * Writes the feed document beside `archive`, the archive of `releaseDirectory`, naming on Windows
 * the window archive `desktop.archive` with the window list `desktop.list`, and signs it for a test
 * build.
 */
function writeFeed({ profile, archive, archiveUrl, release, releaseDirectory, desktop }) {
	const archiveBytes = readFileSync(archive);
	const desktopBytes = desktop === undefined ? undefined : readFileSync(desktop.archive);
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
		desktop:
			desktop === undefined
				? undefined
				: {
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
	// macOS keeps the runtime inside the one application bundle the packager wrote.
	const bundle =
		process.platform === 'darwin'
			? path.basename(
					single(
						application,
						(entry) => entry.isDirectory() && entry.name.endsWith('.app'),
						'application bundle',
					),
				)
			: undefined;
	const bundled =
		bundle === undefined
			? path.join(application, 'resources', 'cohort')
			: path.join(application, bundle, 'Contents', 'Resources', 'cohort');
	const version = manifestVersion(bundled);
	const release = readRelease(bundled, version);
	if ((profile === 'release') !== (release.stem === 'orchbee'))
		throw new Error(`The ${profile} profile cannot carry the ${release.stem} programs`);
	const { platform } = release.manifest;
	const windows = release.platform.os === 'win32';
	if (!windows && release.platform.os !== 'darwin')
		throw new Error(`Collecting the ${platform} release is not built`);
	if (release.platform.os !== process.platform)
		throw new Error(`The bundled runtime is for ${platform}, not this host`);
	const installer = windows
		? single(
				path.join(built, 'installer'),
				(entry) => entry.isFile() && entry.name.endsWith('-setup.exe'),
				'setup file',
			)
		: single(
				path.join(built, 'installer'),
				(entry) => entry.isFile() && entry.name.endsWith(`-${version}-${platform}.dmg`),
				`disk image of ${version} for ${platform}`,
			);
	const releaseName = `${release.stem}-${version}-${platform}`;
	const desktopName = desktopReleaseName(release.stem, version, platform);
	const archiveName = `${releaseName}.${archiveFormat(platform)}`;
	// Settled before anything is written, so a missing repository or window list leaves no partial
	// output.
	const archiveUrl = archiveAddress({ profile, repositoryUrl, version, archiveName });
	const desktopUrl = archiveAddress({
		profile,
		repositoryUrl,
		version,
		archiveName: `${desktopName}.zip`,
	});
	const desktopList = readWindowList(application, version, bundle !== undefined);
	mkdirSync(output, { recursive: true });
	const releaseDirectory = path.join(output, releaseName);
	// The copy restores the programs' executable bit off Windows.
	copyRelease(bundled, release, releaseDirectory);
	const archive = path.join(output, archiveName);
	if (windows) archiveRelease(releaseDirectory, archive);
	else archiveTarGzip(releaseDirectory, release, archive);
	const desktop = {
		archive: archiveWindow(application, output, desktopName, bundle),
		url: desktopUrl,
		list: desktopList,
	};
	writeFeed({ profile, archive, archiveUrl, release, releaseDirectory, desktop });
	copyFileSync(installer, path.join(output, path.basename(installer)));
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

/** `manifest` without its optional dependencies, the one member the rows' main packages differ in. */
function withoutOptionalDependencies(manifest) {
	return Object.fromEntries(
		Object.entries(manifest).filter(([member]) => member !== 'optionalDependencies'),
	);
}

/**
 * The main npm package of several rows, from their own main tarballs: the first one's entries with a
 * package.json that lists every row's optional dependencies. Every row's package must hold the same
 * entries with the same modes and bytes, and the same package.json but for those dependencies.
 */
function joinedMainPackage(tarballs) {
	const read = tarballs.map((file) => {
		const entries = readTarGzip(readFileSync(file));
		const manifest = entries.find((entry) => entry.path === npmManifest);
		if (manifest === undefined) throw new Error(`${file} holds no ${npmManifest}`);
		return { file, entries, manifest: JSON.parse(manifest.bytes.toString('utf8')) };
	});
	const [first] = read;
	const base = JSON.stringify(withoutOptionalDependencies(first.manifest));
	const listed = new Map();
	for (const { file, entries, manifest } of read) {
		const same =
			entries.length === first.entries.length &&
			entries.every((entry, index) => {
				const other = first.entries[index];
				return (
					entry.path === other.path &&
					entry.type === other.type &&
					entry.mode === other.mode &&
					(entry.path === npmManifest || entry.bytes.equals(other.bytes))
				);
			});
		if (!same) throw new Error(`${file} holds other files than ${first.file}`);
		if (JSON.stringify(withoutOptionalDependencies(manifest)) !== base)
			throw new Error(`${file} has another package.json than ${first.file}`);
		for (const [name, version] of Object.entries(manifest.optionalDependencies ?? {})) {
			if (listed.has(name)) throw new Error(`Two rows list the optional dependency ${name}`);
			listed.set(name, version);
		}
	}
	// The member keeps its place in the document; package-npm.mjs writes it in name order.
	const manifest = {
		...first.manifest,
		optionalDependencies: Object.fromEntries(
			[...listed].toSorted(([left], [right]) => byName(left, right)),
		),
	};
	const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, '\t')}\n`);
	return tarGzip(
		first.entries.map((entry) =>
			entry.type === 'directory'
				? { path: entry.path, directory: true, mode: entry.mode, mtime: entry.mtime }
				: {
						path: entry.path,
						mode: entry.mode,
						mtime: entry.mtime,
						bytes: entry.path === npmManifest ? manifestBytes : entry.bytes,
					},
		),
	);
}

/**
 * Joins the artifacts directories `rows` of one run, each holding its feed signature, into
 * `output`, a new directory, and returns its names. One row is copied unchanged. Several must carry
 * one stem and version, each its own platform, and distinct names apart from SHA256SUMS and the
 * main npm package; the joined set holds that package once, listing every row's platform package
 * (`joinedMainPackage`), and SHA256SUMS over its files by the collect rule. The result must pass
 * `check`.
 */
export function joinArtifacts({ output, rows }) {
	if (!path.isAbsolute(output)) throw new UsageError('The output directory must be absolute');
	if (existsSync(output)) throw new Error(`Refusing to write into an existing ${output}`);
	const described = rows.map((directory) => {
		const releaseDirectory = single(
			directory,
			(entry) =>
				entry.isDirectory() && existsSync(path.join(directory, entry.name, 'release.json')),
			'release directory',
		);
		const version = manifestVersion(releaseDirectory);
		const { stem, manifest } = readRelease(releaseDirectory, version);
		return { directory, stem, version, platform: manifest.platform };
	});
	const [first] = described;
	const problems = [];
	const platforms = new Set();
	for (const row of described) {
		if (row.stem !== first.stem || row.version !== first.version)
			problems.push(
				`${row.directory} holds ${row.stem} ${row.version}, not ${first.stem} ${first.version}`,
			);
		if (platforms.has(row.platform)) problems.push(`Two rows hold ${row.platform}`);
		platforms.add(row.platform);
	}
	const copy = { recursive: true, errorOnExist: true, force: false, verbatimSymlinks: true };
	const mainName = `${first.stem}-${first.version}.tgz`;
	const sources = new Map();
	if (rows.length > 1) {
		for (const { directory } of described) {
			for (const name of readdirSync(directory)) {
				if (name === checksumsName || name === mainName) continue;
				if (sources.has(name))
					problems.push(`${sources.get(name)} and ${directory} both hold ${name}`);
				else sources.set(name, directory);
			}
		}
	}
	if (problems.length > 0) throw new Error(problems.join('\n'));
	if (rows.length === 1) {
		cpSync(first.directory, output, copy);
	} else {
		mkdirSync(output, { recursive: true });
		for (const [name, directory] of sources)
			cpSync(path.join(directory, name), path.join(output, name), copy);
		const joined = joinedMainPackage(
			described.map(({ directory }) => path.join(directory, mainName)),
		);
		writeFileSync(path.join(output, mainName), joined, { flag: 'wx' });
		writeChecksums(output);
	}
	const found = checkArtifacts(output);
	if (found.length > 0) throw new Error(found.join('\n'));
	return readdirSync(output).toSorted(byName);
}

function parseArguments(argv) {
	const [command, ...rest] = argv;
	if (command === 'check' && rest.length === 1) return { command, directory: rest[0] };
	if (command === 'join' && rest[0] === '--output' && rest.length >= 3)
		return { command, output: rest[1], rows: rest.slice(2).map((row) => path.resolve(row)) };
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
		} else if (options.command === 'join') {
			for (const name of joinArtifacts(options)) process.stdout.write(`${name}\n`);
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
