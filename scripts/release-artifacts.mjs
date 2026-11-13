// Gathers one platform's release artifacts from a finished installer build into one directory, and
// checks such a directory after it has been moved, as the release workflow does between runners.
//
//   node scripts/release-artifacts.mjs collect --profile <test|release> --output <absolute new directory>
//   node scripts/release-artifacts.mjs check <directory>
//
// `collect` reads what `pnpm desktop:installer:<profile>` left under apps/desktop/out/<profile>/: the
// packaged application's bundled runtime, which that build made a signed release directory, and the
// setup file. It writes, without signing anything again:
//
//   <stem>-<version>-<platform>/   the signed release directory, every file checked against release.json
//   <stem>-<version>-<platform>.zip  that directory as one top-level folder, for the command line alone
//   <setup name>.exe               the installer
//   <npm tarballs>                 the folders of scripts/package-npm.mjs, packed by `npm pack`
//   SHA256SUMS                     `<64 hex>  <name>` and LF for every other top-level file, in name order
//
// In a GitHub Actions run the npm packages name the run's repository (GITHUB_SERVER_URL and
// GITHUB_REPOSITORY), which npm provenance requires.
//
// `check` confirms SHA256SUMS names exactly the other top-level files and that each matches, and that
// every release directory still matches its manifest. Nothing is published or sent anywhere.
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

const repository = fileURLToPath(new URL('..', import.meta.url));
const checksumsName = 'SHA256SUMS';
const checksumLine = /^([0-9a-f]{64}) {2}([^/\\\r\n]+)$/u;
const usage =
	'Usage: node scripts/release-artifacts.mjs collect --profile <test|release> --output <absolute new directory>\n' +
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

/** Writes SHA256SUMS over every other top-level regular file of `directory`. */
function writeChecksums(directory) {
	const names = readdirSync(directory, { withFileTypes: true })
		.filter((entry) => entry.isFile() && entry.name !== checksumsName)
		.map((entry) => entry.name)
		.toSorted(byName);
	const lines = names.map((name) => `${fileDigest(path.join(directory, name))}  ${name}\n`);
	writeFileSync(path.join(directory, checksumsName), lines.join(''), { flag: 'wx' });
}

/** Every problem `check` finds in `directory`; none means intact. */
function checkArtifacts(directory) {
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
		else if (!listed.has(entry.name)) problems.push(`${entry.name} is not in ${checksumsName}`);
		else if (fileDigest(filename) !== listed.get(entry.name))
			problems.push(`${entry.name} does not match ${checksumsName}`);
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

/** Gathers the artifacts of the `profile` installer build into `output`. */
function collect({ profile, output }) {
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
	mkdirSync(output, { recursive: true });
	const releaseDirectory = path.join(
		output,
		`${release.stem}-${version}-${release.manifest.platform}`,
	);
	copyRelease(bundled, release, releaseDirectory);
	if (release.platform.os === 'win32') archiveRelease(releaseDirectory);
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
			if (!['--profile', '--output'].includes(option) || value === undefined)
				throw new UsageError(usage);
			const key = option.slice(2);
			if (options[key] !== undefined) throw new UsageError(usage);
			options[key] = value;
		}
		if (!['test', 'release'].includes(options.profile) || options.output === undefined)
			throw new UsageError(usage);
		return { command, ...options };
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
