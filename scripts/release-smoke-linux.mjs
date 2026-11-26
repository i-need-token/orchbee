// Proves a built Linux release on a fresh machine, one step per subcommand so that a workflow shows
// which step failed: the command-line archive, the install script and the npm route. Each step
// works in a new folder `--work` with its own HOME there, so the store root, the command link, the
// data root and npm's files land inside it; it installs and removes for real inside that folder and
// deletes the folder at the end. It never touches the real HOME.
//
//   node scripts/release-smoke-linux.mjs archive        <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-linux.mjs install-script <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-linux.mjs npm            <artifacts> --work <absolute new directory>
//
// <artifacts> is a directory `node scripts/release-artifacts.mjs collect --release-directory` wrote
// on Linux; its release directory names the version, the platform and the program stem, so the same
// lines serve the test and the release identity. The platform must be this machine's own,
// `linux-x86_64` or `linux-aarch64`, so a row is never proven on the other architecture.
//
// - archive: the system tar unpacks the `.tar.gz` into one folder named like it, whose four programs
//   are executable; its Client's `install` activates the version in `~/.local/share/<stem>` and
//   links `~/.local/bin/<stem>`, which answers `--version`; `uninstall` through the link removes the
//   store root and the link.
// - install-script: runs apps/website/public/install.sh as `curl … | sh` does, the script on the
//   standard input of /bin/sh, against a signed copy of the row's feed (`feed.json` and
//   `feed.json.sig`) that names its archive by file name, in a folder beside the `.tar.gz`, reached
//   through ORCHBEE_INSTALL_SH_FEED, a file: address. It must verify `feed.json.sig`, activate the
//   version, link the command and leave its temporary folder empty; `uninstall` through the link
//   then removes the store root and the link.
// - npm: `npm install --global --offline` of the tarballs into a prefix; the first start installs
//   the carried version, `--version` answers it again, the program's `uninstall` removes the store
//   root, and `npm rm --global` removes the carrier.
//
// Every Linux build's `uninstall` takes `--confirm`: only a release Windows or macOS build asks a
// person to type `orchbee` instead.
//
// Every step then requires that no process runs from the work folder, by the program each process
// in /proc started.
// Exit codes: 0 the step passed; 1 it failed; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import {
	existsSync,
	lstatSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	readlinkSync,
	realpathSync,
	rmSync,
	statSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { readRelease } from './package-npm.mjs';
import { feedName, gateFeed } from './release-feed.mjs';
import { StepFailure, UsageError } from './release-update-check.mjs';
import { FEED_DOMAIN, loadKey, signatureDocument } from './sign-release-manifest.mjs';

const usage =
	'Usage: node scripts/release-smoke-linux.mjs <archive|install-script|npm> <artifacts> --work <absolute new directory>';
const installScript = fileURLToPath(new URL('../apps/website/public/install.sh', import.meta.url));
const testSigningKey = fileURLToPath(
	new URL(
		'../apps/orchbee/src/cli/update/tests/fixtures/test-only-signing-key.pem',
		import.meta.url,
	),
);
// Each command gets this long; an install or an npm run gets longer.
const commandDeadline = 120_000;
const setupDeadline = 600_000;
// A process the uninstall ends gets this long to leave the process list.
const settleDeadline = 15_000;
const hostPlatforms = { arm64: 'linux-aarch64', x64: 'linux-x86_64' };
const hostPlatform = hostPlatforms[process.arch];
function log(line) {
	process.stdout.write(`${line}\n`);
}

function sleep(milliseconds) {
	Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function inside(parent, child) {
	const relative = path.relative(parent, child);
	return relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
}

/** Everything the steps need to know about the build, read from the collected artifacts. */
function readIdentity(artifacts) {
	const entries = readdirSync(artifacts, { withFileTypes: true });
	const releases = entries.filter(
		(entry) => entry.isDirectory() && existsSync(path.join(artifacts, entry.name, 'release.json')),
	);
	if (releases.length !== 1) throw new StepFailure(`${artifacts} must hold one release directory`);
	const releaseName = releases[0].name;
	const releaseDirectory = path.join(artifacts, releaseName);
	const { version } = JSON.parse(readFileSync(path.join(releaseDirectory, 'release.json'), 'utf8'));
	const { stem, manifest, platform, programs } = readRelease(releaseDirectory, version);
	if (manifest.platform !== hostPlatform)
		throw new StepFailure(
			`${manifest.platform} is not this machine's platform ${hostPlatform ?? process.arch}`,
		);
	const archiveFile = path.join(artifacts, `${releaseName}.tar.gz`);
	if (!existsSync(archiveFile))
		throw new StepFailure(`${artifacts} holds no ${releaseName}.tar.gz`);
	return {
		artifacts,
		version,
		stem,
		platform: manifest.platform,
		cpu: platform.cpu,
		programs,
		releaseName,
		archive: archiveFile,
		tarballs: entries
			.filter((entry) => entry.isFile() && entry.name.endsWith('.tgz'))
			.map((entry) => path.join(artifacts, entry.name)),
	};
}

/**
 * This environment with HOME in the work folder, npm's cache and configuration below it, and no
 * Orchbee, XDG or npm setting inherited, so every default root lies inside the work folder.
 */
function scratchEnvironment(home) {
	const kept = Object.entries(process.env).filter(
		([name]) => !/^(?:ORCHBEE_|XDG_|npm_config_|NPM_CONFIG_)/u.test(name),
	);
	return {
		...Object.fromEntries(kept),
		HOME: home,
		npm_config_cache: path.join(home, '.npm'),
	};
}

/** Runs one program with captured output, which it prints; a missing program or a timeout fails. */
function run(program, argv, { env = process.env, deadline = commandDeadline } = {}) {
	log(`> ${program} ${argv.join(' ')}`);
	const result = spawnSync(program, argv, {
		encoding: 'utf8',
		env,
		stdio: ['ignore', 'pipe', 'pipe'],
		timeout: deadline,
		maxBuffer: 64 * 1024 * 1024,
	});
	if (result.error !== undefined)
		throw new StepFailure(`${program} ${argv.join(' ')}: ${result.error.message}`);
	if (result.stdout) process.stdout.write(result.stdout);
	if (result.stderr) process.stdout.write(result.stderr);
	return { status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}

function expectStatus(result, expected, what) {
	if (result.status !== expected)
		throw new StepFailure(`${what} exited ${result.status}, expected ${expected}`);
}

/** Runs `command uninstall --confirm`. */
function uninstall(command, env) {
	expectStatus(
		run(command, ['uninstall', '--confirm'], { env, deadline: setupDeadline }),
		0,
		'uninstall --confirm',
	);
}

function expectVersion(identity, result, what) {
	expectStatus(result, 0, what);
	const answer = result.stdout.trim().split(/\s+/u).at(-1);
	if (answer !== identity.version)
		throw new StepFailure(
			`${what} answered ${JSON.stringify(answer)}, expected ${identity.version}`,
		);
}

/** The store root a release or test build installs into under `home`. */
function storeRoot(identity, home) {
	return path.join(home, '.local', 'share', identity.stem);
}

/** Requires the store root to name the row's version as active. */
function expectActive(identity, root) {
	const pointer = path.join(root, 'active.json');
	if (!existsSync(pointer)) throw new StepFailure(`Nothing was installed into ${root}`);
	const active = JSON.parse(readFileSync(pointer, 'utf8'))?.active?.product;
	if (active !== identity.version)
		throw new StepFailure(`active.json in ${root} names ${active}, not ${identity.version}`);
	log(`${root} runs ${identity.version}.`);
}

/** Requires `~/.local/bin/<stem>` to be a link into the store root that answers the version. */
function expectLink(identity, home, root, env, what) {
	const link = path.join(home, '.local', 'bin', identity.stem);
	if (!lstatSync(link, { throwIfNoEntry: false })?.isSymbolicLink())
		throw new StepFailure(`${what} left no command link ${link}`);
	if (!inside(realpathSync(root), realpathSync(link)))
		throw new StepFailure(`${link} names ${readlinkSync(link)}, outside ${root}`);
	expectVersion(identity, run(link, ['--version'], { env }), `${identity.stem} --version`);
	return link;
}

function expectGone(filename, what) {
	if (lstatSync(filename, { throwIfNoEntry: false }) !== undefined)
		throw new StepFailure(`${what} ${filename} is still there`);
}

/** The processes whose program lies inside `work`, by the program each started from /proc. */
function processesInside(work) {
	const found = [];
	for (const entry of readdirSync('/proc')) {
		if (!/^\d+$/u.test(entry)) continue;
		let program;
		try {
			program = readlinkSync(path.join('/proc', entry, 'exe'));
		} catch {
			// Gone already, or another user's process this one may not read.
			continue;
		}
		const started = program.replace(/ \(deleted\)$/u, '');
		if (inside(work, started)) found.push(`${entry} ${started}`);
	}
	return found;
}

function expectNoProcesses(work) {
	const until = Date.now() + settleDeadline;
	let found = processesInside(work);
	while (found.length > 0 && Date.now() < until) {
		sleep(1_000);
		found = processesInside(work);
	}
	if (found.length > 0)
		throw new StepFailure(
			`Still running from ${work}:\n${found.map((each) => `  ${each}`).join('\n')}`,
		);
	log(`No process runs from ${work}.`);
}

function archive(identity, work) {
	const home = path.join(work, 'home');
	const env = scratchEnvironment(home);
	const unpacked = path.join(work, 'unpacked');
	mkdirSync(unpacked);
	expectStatus(
		run('tar', ['-x', '-z', '-f', identity.archive, '-C', unpacked]),
		0,
		`tar -x ${path.basename(identity.archive)}`,
	);
	const top = readdirSync(unpacked);
	if (top.length !== 1 || top[0] !== identity.releaseName)
		throw new StepFailure(`The archive must hold the one folder ${identity.releaseName}`);
	const folder = path.join(unpacked, identity.releaseName);
	for (const program of identity.programs) {
		if ((statSync(path.join(folder, program)).mode & 0o111) !== 0o111)
			throw new StepFailure(`${program} is not executable after tar unpacked it`);
	}
	const installed = run(path.join(folder, identity.stem), ['install', '--format', 'json'], {
		env,
		deadline: setupDeadline,
	});
	let document;
	try {
		document = JSON.parse(installed.stdout);
	} catch {
		document = null;
	}
	if (installed.status !== 0 || document?.result?.activated !== true)
		throw new StepFailure(
			`install exited ${installed.status}${document?.result?.error ? ` (error ${JSON.stringify(document.result.error)})` : ''}`,
		);
	const root = storeRoot(identity, home);
	expectActive(identity, root);
	const link = expectLink(identity, home, root, env, 'install');
	uninstall(link, env);
	expectGone(root, 'The store root');
	expectGone(link, 'The command link');
	expectNoProcesses(work);
}

function installScriptStep(identity, work) {
	const home = path.join(work, 'home');
	const temporary = path.join(work, 'tmp');
	const served = path.join(work, 'served');
	for (const folder of [home, temporary, served]) mkdirSync(folder);
	const feed = readFileSync(
		path.join(identity.artifacts, feedName(identity.stem, identity.version, identity.platform)),
	);
	// The gate copy names the archive by its bare name, resolved beside the served feed.
	const servedFeed = gateFeed(feed);
	writeFileSync(path.join(served, 'feed.json'), servedFeed);
	writeFileSync(
		path.join(served, 'feed.json.sig'),
		signatureDocument(servedFeed, [loadKey(readFileSync(testSigningKey, 'utf8'))], FEED_DOMAIN),
	);
	symlinkSync(identity.archive, path.join(served, path.basename(identity.archive)));
	const env = {
		...scratchEnvironment(home),
		TMPDIR: temporary,
		ORCHBEE_INSTALL_SH_FEED: `file://${path.join(served, 'feed.json')}`,
	};
	log(`> /bin/sh -s < ${installScript}`);
	const result = spawnSync('/bin/sh', ['-s'], {
		encoding: 'utf8',
		env,
		input: readFileSync(installScript),
		timeout: setupDeadline,
		maxBuffer: 64 * 1024 * 1024,
	});
	if (result.error !== undefined) throw new StepFailure(`/bin/sh: ${result.error.message}`);
	process.stdout.write(`${result.stdout ?? ''}${result.stderr ?? ''}`);
	expectStatus(result, 0, 'install.sh');
	const left = readdirSync(temporary);
	if (left.length > 0)
		throw new StepFailure(`install.sh left ${left.join(', ')} in its temporary folder`);
	const root = storeRoot(identity, home);
	expectActive(identity, root);
	const link = expectLink(identity, home, root, env, 'install.sh');
	log(`install.sh installed ${identity.version}.`);
	uninstall(link, env);
	expectGone(root, 'The store root');
	expectGone(link, 'The command link');
	expectNoProcesses(work);
}

function npm(identity, work) {
	const home = path.join(work, 'home');
	const env = scratchEnvironment(home);
	const prefix = path.join(work, 'prefix');
	if (identity.tarballs.length === 0) throw new StepFailure('The artifacts hold no npm tarball');
	const common = ['--global', '--offline', '--no-audit', '--no-fund', '--prefix', prefix];
	expectStatus(
		run('npm', ['install', ...common, ...identity.tarballs], { env, deadline: setupDeadline }),
		0,
		'npm install',
	);
	const command = path.join(prefix, 'bin', identity.stem);
	expectVersion(
		identity,
		run(command, ['--version'], { env, deadline: setupDeadline }),
		'first start',
	);
	const root = storeRoot(identity, home);
	expectActive(identity, root);
	expectVersion(identity, run(command, ['--version'], { env }), '--version');
	uninstall(command, env);
	expectGone(root, 'The store root');
	const packages = [identity.stem, `@${identity.stem}/cli-linux-${identity.cpu}`];
	expectStatus(
		run('npm', ['rm', ...common, ...packages], { env, deadline: setupDeadline }),
		0,
		'npm rm',
	);
	expectGone(command, 'The npm command');
	expectNoProcesses(work);
}

function parseArguments(argv) {
	const [step, artifacts, option, work, ...rest] = argv;
	if (
		!['archive', 'install-script', 'npm'].includes(step) ||
		artifacts === undefined ||
		option !== '--work' ||
		work === undefined ||
		!path.isAbsolute(work) ||
		rest.length > 0
	)
		throw new UsageError(usage);
	return { step, artifacts: path.resolve(artifacts), work: path.resolve(work) };
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	let work;
	try {
		if (process.platform !== 'linux') throw new StepFailure('This smoke test runs on Linux');
		if (hostPlatform === undefined)
			throw new StepFailure(`${process.arch} is not a Linux release platform`);
		const options = parseArguments(process.argv.slice(2));
		const identity = readIdentity(options.artifacts);
		if (existsSync(options.work))
			throw new StepFailure(`Refusing to work in an existing ${options.work}`);
		mkdirSync(options.work, { recursive: true });
		// The real path, so the process scan compares the programs /proc reports.
		work = realpathSync(options.work);
		const steps = { archive, 'install-script': installScriptStep, npm };
		steps[options.step](identity, work);
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	} finally {
		if (work !== undefined) rmSync(work, { recursive: true, force: true });
	}
}
