// Proves a built macOS release on a fresh machine, one step per subcommand so that a workflow shows
// which step failed: the disk image, the command-line archive, the install script and the npm
// route. Each step works in a new folder `--work` with its own HOME there, so the store root, the command link, the data root
// and npm's files land inside it; it installs and removes for real inside that folder and deletes
// the folder at the end. It never touches /Applications or the real HOME.
//
//   node scripts/release-smoke-macos.mjs disk-image <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-macos.mjs archive    <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-macos.mjs install-script <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-macos.mjs npm        <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-macos.mjs update     v<version> --release <assets address>
//     --work <absolute new directory> [--stem <stem>] [--current <absolute folder>]
//
// <artifacts> is a directory `node scripts/release-artifacts.mjs collect` wrote on macOS; its release
// directory names the version, the platform and the program stem, so the same lines serve the test
// and the release identity. The platform must be this Mac's own, `macos-aarch64` on Apple silicon
// and `macos-x86_64` on Intel, so a row is never proven on the other architecture; `update` reads
// that platform's feed, tracked update pair and archives.
//
// - disk-image: `hdiutil attach -readonly -nobrowse` mounts the `.dmg` on a folder inside the work
//   folder; the volume holds one application and a link to /Applications; `codesign --verify
//   --deep --strict` accepts the application; its bundled `release.json` is the release
//   directory's; its bundled Client answers `--version` with the version. The image is detached
//   even after a failure, before the work folder goes.
// - archive: the system tar unpacks the `.tar.gz` into one folder named like it, whose four programs
//   are executable; its Client's `install` activates the version in `~/.local/share/<stem>` and
//   links `~/.local/bin/<stem>`, which answers `--version`; `uninstall` through the link removes the
//   store root and the link.
// - install-script: runs apps/website/public/install.sh with `--desktop` as `curl … | sh -s --` does,
//   the script on the standard input of /bin/sh, against a copy of the row's feed that names its
//   archives by file name, in a folder beside the `.tar.gz` and the window archive, reached
//   through the script's test variables: ORCHBEE_INSTALL_SH_FEED, a file: address, and
//   ORCHBEE_INSTALL_SH_APPLICATIONS, a folder in the work folder, under a shell's soft open-file
//   limit of 256. It must activate the version, link the command, print the line that puts
//   ~/.local/bin on PATH, copy the application with a valid signature and the row's release.json,
//   store the window version with the application in the store's `desktop` folder, and leave its
//   temporary folder empty; `uninstall` through the link then removes the store root and the link.
// - npm: `npm install --global --offline` of the tarballs into a prefix; the first start installs
//   the carried version, `--version` answers it again, the program's `uninstall` removes the store
//   root, and `npm rm --global` removes the carrier.
// - update: a published release can be updated to, the check that runs before the update channel
//   moves, with the contract of the Windows script's `update` (scripts/release-update-check.mjs).
//   The version people have now is the one the website's tracked update pair of this Mac's platform
//   names (apps/website/public/update/stable/<platform>/, or the folder `--current` names). Its
//   `.tar.gz`, fetched from the address the pair signs and checked against the signed size and
//   SHA-256, is unpacked by the system tar and installed by its own `install` into
//   `~/.local/share/<stem>` with the link `~/.local/bin/<stem>`; then `update` runs through that
//   link with `ORCHBEE_UPDATE_FEED` naming the release's `<stem>-<version>-<platform>.feed.json` in
//   the assets folder, and must activate the release: the pointer names it, the version's Client
//   and the link answer `--version` with it, and the link names that Client. Without an older
//   tracked version, the release's `update` fills an empty scratch store beside the store root.
//   TMPDIR is a folder in the work folder too, and nothing is uninstalled: the work folder goes. A
//   failure names the stage and tells the owner not to move the update channel.
//
// A test build's `uninstall` takes `--confirm`. A release build refuses it and asks a person to type
// `orchbee` on a terminal, so its `uninstall` runs on a terminal that python3's pty module opens,
// and the word is typed once the prompt shows. script(1) cannot stand in: it refuses Node's socket
// as its input, and on macOS it passes the end of a closed input ahead of a line not yet read.
//
// The archive, install-script, npm and update steps then require that no process runs from the work
// folder.
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
import {
	parseUpdateArguments,
	StepFailure,
	updateCheck,
	updateFailureAdvice,
	UsageError,
} from './release-update-check.mjs';

const usage =
	'Usage: node scripts/release-smoke-macos.mjs <disk-image|archive|install-script|npm> <artifacts> --work <absolute new directory>\n' +
	'       node scripts/release-smoke-macos.mjs update v<version> --release <assets address> --work <absolute new directory> [--stem <stem>] [--current <absolute folder>]';
const installScript = fileURLToPath(new URL('../apps/website/public/install.sh', import.meta.url));
// Each command gets this long; an install, an npm run or a mount gets longer.
const commandDeadline = 120_000;
const setupDeadline = 600_000;
// A process the uninstall ends gets this long to leave the process list.
const settleDeadline = 15_000;
// The soft open-file limit a Terminal or SSH shell starts with on macOS.
const shellOpenFileLimit = 256;
const hostPlatforms = { arm64: 'macos-aarch64', x64: 'macos-x86_64' };
const hostPlatform = hostPlatforms[process.arch];
// The release stem, whose builds ask for this word in place of `uninstall --confirm`.
const releaseStem = 'orchbee';
// The release prompt in the locale the typed run sets.
const uninstallPrompt = 'Type orchbee to uninstall:';
// Runs argv[3:] on a new terminal, copies its output, types argv[1] and Enter once argv[2] shows,
// and exits with the program's status.
const typeAtPrompt = `
import os, pty, sys
word, prompt, program = sys.argv[1].encode(), sys.argv[2].encode(), sys.argv[3:]
pid, terminal = pty.fork()
if pid == 0:
    os.execv(program[0], program)
seen, typed = b"", False
while True:
    try:
        chunk = os.read(terminal, 4096)
    except OSError:
        break
    if not chunk:
        break
    os.write(1, chunk)
    seen += chunk
    if not typed and prompt in seen:
        os.write(terminal, word + b"\\n")
        typed = True
sys.exit(os.waitstatus_to_exitcode(os.waitpid(pid, 0)[1]))
`;

// The mount the disk image step has not detached; the work folder stays while one is left.
const pending = { mount: undefined };

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
	const diskImages = entries.filter((entry) => entry.isFile() && entry.name.endsWith('.dmg'));
	if (releases.length !== 1 || diskImages.length !== 1)
		throw new StepFailure(`${artifacts} must hold one release directory and one disk image`);
	const releaseName = releases[0].name;
	const releaseDirectory = path.join(artifacts, releaseName);
	const manifestBytes = readFileSync(path.join(releaseDirectory, 'release.json'));
	const { version } = JSON.parse(manifestBytes.toString('utf8'));
	const { stem, manifest, platform, programs } = readRelease(releaseDirectory, version);
	if (manifest.platform !== hostPlatform)
		throw new StepFailure(
			`${manifest.platform} is not this Mac's platform ${hostPlatform ?? process.arch}`,
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
		manifestBytes,
		diskImage: path.join(artifacts, diskImages[0].name),
		archive: archiveFile,
		tarballs: entries
			.filter((entry) => entry.isFile() && entry.name.endsWith('.tgz'))
			.map((entry) => path.join(artifacts, entry.name)),
	};
}

/**
 * This environment with HOME in the work folder, npm's cache and configuration below it, and no
 * Orchbee or npm setting inherited, so every default root lies inside the work folder.
 */
function scratchEnvironment(home) {
	const kept = Object.entries(process.env).filter(
		([name]) => !/^(?:ORCHBEE_|npm_config_|NPM_CONFIG_)/u.test(name),
	);
	return { ...Object.fromEntries(kept), HOME: home, npm_config_cache: path.join(home, '.npm') };
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

/**
 * Runs `command uninstall` as its build requires: a test build with `--confirm`, a release build on
 * a terminal, typing the word once the prompt shows.
 */
function uninstall(identity, command, env) {
	if (identity.stem !== releaseStem) {
		expectStatus(
			run(command, ['uninstall', '--confirm'], { env, deadline: setupDeadline }),
			0,
			'uninstall --confirm',
		);
		return;
	}
	log(`> ${command} uninstall, on a terminal, typing ${releaseStem} at its prompt`);
	const result = spawnSync(
		'/usr/bin/python3',
		['-I', '-c', typeAtPrompt, releaseStem, uninstallPrompt, command, 'uninstall'],
		{
			encoding: 'utf8',
			env: { ...env, LC_ALL: 'en_US.UTF-8' },
			stdio: ['ignore', 'pipe', 'pipe'],
			timeout: setupDeadline,
			maxBuffer: 64 * 1024 * 1024,
		},
	);
	if (result.error !== undefined) throw new StepFailure(`python3: ${result.error.message}`);
	process.stdout.write(`${result.stdout ?? ''}${result.stderr ?? ''}`);
	if (!result.stdout.includes(uninstallPrompt))
		throw new StepFailure('uninstall never asked for the word');
	expectStatus(result, 0, `uninstall, typing ${releaseStem}`);
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

/** Requires the store root to name `version` as active. */
function expectActive(identity, root) {
	const pointer = path.join(root, 'active.json');
	if (!existsSync(pointer)) throw new StepFailure(`Nothing was installed into ${root}`);
	const active = JSON.parse(readFileSync(pointer, 'utf8'))?.active?.product;
	if (active !== identity.version)
		throw new StepFailure(`active.json in ${root} names ${active}, not ${identity.version}`);
	log(`${root} runs ${identity.version}.`);
}

/**
 * Requires the store's window pointer to name `version` with its copy of `application` beside its
 * list; test and release builds both install window versions.
 */
function expectWindowVersion(root, version, application) {
	const desktop = path.join(root, 'desktop');
	if (!existsSync(desktop))
		throw new StepFailure(`install.sh left no window version in ${desktop}`);
	const pointer = path.join(desktop, 'active.json');
	const active = existsSync(pointer)
		? JSON.parse(readFileSync(pointer, 'utf8'))?.active?.product
		: undefined;
	if (active !== version)
		throw new StepFailure(`${pointer} names ${active}, not the window version ${version}`);
	const stored = path.join(desktop, 'versions', version);
	for (const entry of ['desktop-files.json', application])
		if (!existsSync(path.join(stored, entry)))
			throw new StepFailure(`The window version ${stored} holds no ${entry}`);
	log(`${desktop} names the window version ${version}.`);
}

function expectGone(filename, what) {
	if (lstatSync(filename, { throwIfNoEntry: false }) !== undefined)
		throw new StepFailure(`${what} ${filename} is still there`);
}

/** The processes whose program lies inside `work`, as `ps` reports their start paths. */
function processesInside(work) {
	const result = spawnSync('/bin/ps', ['-axo', 'pid=,comm='], { encoding: 'utf8' });
	if (result.status !== 0) throw new StepFailure(`ps exited ${result.status}`);
	return result.stdout
		.split('\n')
		.map((line) => /^\s*(\d+)\s+(.+)$/u.exec(line))
		.filter((match) => match !== null && inside(work, match[2].trim()))
		.map((match) => `${match[1]} ${match[2].trim()}`);
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

/** Detaches the image mounted on `mount`, forcing it on a second attempt; true when it is gone. */
function detach(mount) {
	for (const extra of [[], ['-force']]) {
		const result = spawnSync('/usr/bin/hdiutil', ['detach', mount, ...extra], {
			encoding: 'utf8',
			timeout: commandDeadline,
		});
		if (result.status === 0) {
			log(`Detached ${mount}`);
			pending.mount = undefined;
			return true;
		}
		process.stdout.write(`${result.stdout ?? ''}${result.stderr ?? ''}`);
		sleep(2_000);
	}
	return false;
}

function diskImage(identity, work) {
	const env = scratchEnvironment(path.join(work, 'home'));
	const mount = path.join(work, 'volume');
	mkdirSync(mount);
	// hdiutil may mount the image even when it then fails.
	pending.mount = mount;
	const attached = run(
		'/usr/bin/hdiutil',
		['attach', identity.diskImage, '-readonly', '-nobrowse', '-noautoopen', '-mountpoint', mount],
		{ deadline: setupDeadline },
	);
	if (attached.status !== 0) {
		if (readdirSync(mount).length === 0) pending.mount = undefined;
		else detach(mount);
		throw new StepFailure(`hdiutil attach exited ${attached.status}`);
	}
	let failure;
	try {
		const shown = readdirSync(mount, { withFileTypes: true }).filter(
			(entry) => !entry.name.startsWith('.'),
		);
		const applications = shown.filter(
			(entry) => entry.isDirectory() && entry.name.endsWith('.app'),
		);
		const link = path.join(mount, 'Applications');
		if (applications.length !== 1 || shown.length !== 2)
			throw new StepFailure(
				`The image must hold one application and the Applications link, not ${shown.map((entry) => entry.name).join(', ')}`,
			);
		if (!lstatSync(link).isSymbolicLink() || readlinkSync(link) !== '/Applications')
			throw new StepFailure('The image has no link to /Applications');
		const cohort = expectApplication(identity, path.join(mount, applications[0].name));
		expectVersion(
			identity,
			run(path.join(cohort, identity.stem), ['--version'], { env }),
			'the bundled --version',
		);
		log(`${path.basename(identity.diskImage)} mounts, verifies and runs ${identity.version}.`);
	} catch (error) {
		failure = error;
	}
	const detached = detach(mount);
	if (failure !== undefined) throw failure;
	if (!detached) throw new StepFailure(`hdiutil could not detach ${mount}`);
}

function archive(identity, work) {
	const home = path.join(work, 'home');
	const env = scratchEnvironment(home);
	const unpacked = path.join(work, 'unpacked');
	mkdirSync(unpacked);
	expectStatus(
		run('/usr/bin/tar', ['-x', '-z', '-f', identity.archive, '-C', unpacked]),
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
	const link = path.join(home, '.local', 'bin', identity.stem);
	if (!lstatSync(link, { throwIfNoEntry: false })?.isSymbolicLink())
		throw new StepFailure(`install left no command link ${link}`);
	if (!inside(realpathSync(root), realpathSync(link)))
		throw new StepFailure(`${link} names ${readlinkSync(link)}, outside ${root}`);
	expectVersion(identity, run(link, ['--version'], { env }), `${identity.stem} --version`);
	uninstall(identity, link, env);
	expectGone(root, 'The store root');
	expectGone(link, 'The command link');
	expectNoProcesses(work);
}

/** Requires `application` to pass codesign and to bundle the row's release.json; its cohort folder. */
function expectApplication(identity, application) {
	expectStatus(
		run('/usr/bin/codesign', ['--verify', '--deep', '--strict', '--verbose=2', application]),
		0,
		'codesign --verify --deep --strict',
	);
	const cohort = path.join(application, 'Contents', 'Resources', 'cohort');
	if (!readFileSync(path.join(cohort, 'release.json')).equals(identity.manifestBytes))
		throw new StepFailure(`${path.basename(application)} bundles another release.json`);
	return cohort;
}

function installScriptStep(identity, work) {
	const home = path.join(work, 'home');
	const temporary = path.join(work, 'tmp');
	const applications = path.join(work, 'Applications');
	const served = path.join(work, 'served');
	for (const folder of [home, temporary, served]) mkdirSync(folder);
	const feed = readFileSync(
		path.join(identity.artifacts, feedName(identity.stem, identity.version, identity.platform)),
	);
	const servedFeed = gateFeed(feed);
	writeFileSync(path.join(served, 'feed.json'), servedFeed);
	// The gate feed names both archives by their bare names.
	const window = JSON.parse(servedFeed.toString('utf8'))?.extensions?.desktop;
	if (typeof window?.archive?.url !== 'string' || typeof window.version !== 'string')
		throw new StepFailure('The feed names no window archive in extensions.desktop');
	for (const file of [identity.archive, path.join(identity.artifacts, window.archive.url)])
		symlinkSync(file, path.join(served, path.basename(file)));
	const env = {
		...scratchEnvironment(home),
		TMPDIR: temporary,
		ORCHBEE_INSTALL_SH_FEED: `file://${path.join(served, 'feed.json')}`,
		ORCHBEE_INSTALL_SH_APPLICATIONS: applications,
	};
	// Under the soft open-file limit of a Terminal or SSH shell, which Node raises for its children:
	// a window install that holds the bundle's folders open must raise it itself. Only the soft
	// limit, as in those shells; a plain `ulimit -n` would lower the hard limit too.
	log(`> ulimit -S -n ${shellOpenFileLimit}; /bin/sh -s -- --desktop < ${installScript}`);
	const result = spawnSync(
		'/bin/sh',
		['-c', `ulimit -S -n ${shellOpenFileLimit} && exec /bin/sh -s -- --desktop`],
		{
			encoding: 'utf8',
			env,
			input: readFileSync(installScript),
			timeout: setupDeadline,
			maxBuffer: 64 * 1024 * 1024,
		},
	);
	if (result.error !== undefined) throw new StepFailure(`/bin/sh: ${result.error.message}`);
	process.stdout.write(`${result.stdout ?? ''}${result.stderr ?? ''}`);
	expectStatus(result, 0, 'install.sh --desktop');
	if (!result.stdout.includes('export PATH="$HOME/.local/bin:$PATH"'))
		throw new StepFailure('install.sh printed no line that puts ~/.local/bin on PATH');
	const left = readdirSync(temporary);
	if (left.length > 0)
		throw new StepFailure(`install.sh left ${left.join(', ')} in its temporary folder`);
	const root = storeRoot(identity, home);
	expectActive(identity, root);
	const link = path.join(home, '.local', 'bin', identity.stem);
	if (!lstatSync(link, { throwIfNoEntry: false })?.isSymbolicLink())
		throw new StepFailure(`install.sh left no command link ${link}`);
	expectVersion(identity, run(link, ['--version'], { env }), `${identity.stem} --version`);
	const copied = readdirSync(applications);
	if (copied.length !== 1 || !copied[0].endsWith('.app'))
		throw new StepFailure(`${applications} must hold one application, not ${copied.join(', ')}`);
	expectApplication(identity, path.join(applications, copied[0]));
	expectWindowVersion(root, window.version, copied[0]);
	log(`install.sh installed ${identity.version} and ${copied[0]}.`);
	uninstall(identity, link, env);
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
	uninstall(identity, command, env);
	expectGone(root, 'The store root');
	const packages = [identity.stem, `@${identity.stem}/cli-darwin-${identity.cpu}`];
	expectStatus(
		run('npm', ['rm', ...common, ...packages], { env, deadline: setupDeadline }),
		0,
		'npm rm',
	);
	expectGone(command, 'The npm command');
	expectNoProcesses(work);
}

/**
 * Requires the store `root` to run `version`: its pointer names it, the version's Client answers
 * `--version` with it, and the link `~/.local/bin/<stem>`, when it names a Client of this store,
 * names that one and answers the same. The link must name this store unless `ORCHBEE_INSTALL_DIR`
 * points the run at a scratch store.
 */
function expectRunning(root, stem, version, env) {
	expectActive({ version }, root);
	const client = path.join(root, 'versions', version, stem);
	expectVersion({ version }, run(client, ['--version'], { env }), `${client} --version`);
	const link = path.join(env.HOME, '.local', 'bin', stem);
	const named = lstatSync(link, { throwIfNoEntry: false })?.isSymbolicLink()
		? realpathSync(link)
		: undefined;
	if (named === undefined || !inside(realpathSync(root), named)) {
		if (env.ORCHBEE_INSTALL_DIR === undefined)
			throw new StepFailure(`${link} names no Client of ${root}`);
		return;
	}
	if (named !== realpathSync(client))
		throw new StepFailure(`${link} names ${named}, not ${client}`);
	expectVersion({ version }, run(link, ['--version'], { env }), `${link} --version`);
	log(`${link} names the Client of ${version}.`);
}

/**
 * How the update check installs and updates on this Mac: HOME and TMPDIR in the work folder, so the
 * store root, the command link and the data root lie inside it.
 */
const updatePort = {
	platform: hostPlatform,
	tar: '/usr/bin/tar',
	client: (directory, stem) => path.join(directory, stem),
	environment(work) {
		const home = path.join(work, 'home');
		const temporary = path.join(work, 'tmp');
		for (const folder of [home, temporary]) mkdirSync(folder);
		return { ...scratchEnvironment(home), TMPDIR: temporary };
	},
	storeRoot: (stem, env) => env.ORCHBEE_INSTALL_DIR ?? storeRoot({ stem }, env.HOME),
	updater: (_root, stem, env) => path.join(env.HOME, '.local', 'bin', stem),
	expectActive: expectRunning,
};

function update(options, work, progress) {
	updateCheck({ ...options, work }, progress, updatePort, usage);
	expectNoProcesses(work);
}

function parseArguments(argv) {
	if (argv[0] === 'update') return parseUpdateArguments(argv.slice(1), usage, hostPlatform);
	const [step, artifacts, option, work, ...rest] = argv;
	if (
		!['disk-image', 'archive', 'install-script', 'npm'].includes(step) ||
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
	let options;
	const progress = { stage: 'starting' };
	try {
		if (process.platform !== 'darwin') throw new StepFailure('This smoke test runs on macOS');
		if (hostPlatform === undefined)
			throw new StepFailure(`${process.arch} is not a macOS release platform`);
		options = parseArguments(process.argv.slice(2));
		const identity = options.step === 'update' ? undefined : readIdentity(options.artifacts);
		if (existsSync(options.work))
			throw new StepFailure(`Refusing to work in an existing ${options.work}`);
		mkdirSync(options.work, { recursive: true });
		// The real path, so the process scan compares the start paths ps reports.
		work = realpathSync(options.work);
		if (options.step === 'update') {
			update(options, work, progress);
		} else {
			const steps = { 'disk-image': diskImage, archive, 'install-script': installScriptStep, npm };
			steps[options.step](identity, work);
		}
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
		// The channel must not move to a release nobody could update to.
		if (options?.step === 'update' && !(error instanceof UsageError))
			process.stderr.write(updateFailureAdvice(options.version, progress.stage));
	} finally {
		if (work !== undefined && pending.mount === undefined)
			rmSync(work, { recursive: true, force: true });
		else if (work !== undefined)
			process.stderr.write(`The image stays mounted on ${pending.mount}, so ${work} stays\n`);
	}
}
