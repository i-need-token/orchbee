// Proves a built Linux release on a fresh machine, one step per subcommand so that a workflow shows
// which step failed: the command-line archive, the install script with the desktop app, the
// AppImage and Debian package, the npm route, the update of a published release, and the real-window
// Update. Each step works in a new folder `--work` with its own HOME there, so the store root, the
// command link, the menu entry, the data root and npm's files land inside it; it installs and
// removes for real inside that folder and deletes the folder at the end. It never touches the real
// HOME.
//
//   node scripts/release-smoke-linux.mjs archive        <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-linux.mjs install-script <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-linux.mjs packages       <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-linux.mjs npm            <artifacts> --work <absolute new directory>
//   node scripts/release-smoke-linux.mjs update         v<version> --release <assets address>
//     --work <absolute new directory> [--stem <stem>] [--current <absolute folder>]
//   node scripts/release-smoke-linux.mjs window-update <packaged Test window>
//     --work <absolute new directory> [--diagnostics <absolute directory>]
//
// <artifacts> is a directory `node scripts/release-artifacts.mjs collect` wrote on Linux; its
// release directory names the version, the platform and the program stem, so the same lines serve
// the test and the release identity. The platform must be this machine's own,
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
//   version, link the command and leave its temporary folder empty. With `--desktop` it also
//   installs the window archive the feed names: the window store must name that window version, its
//   entry must be executable, and the menu entry and its icon must name that entry. `uninstall`
//   through the link then removes the store root, the link, the menu entry and its icons.
// - packages: the AppImage extracts itself with `--appimage-extract` into an AppDir whose `AppRun`,
//   menu entry, icon and window program are in place, and the Debian package's fields and files are
//   the row's: `/opt/<stem>/<stem>-desktop`, the menu entry, the icons, the AppArmor profile and
//   executable maintainer scripts.
// - npm: `npm install --global --offline` of the tarballs into a prefix; the first start installs
//   the carried version, `--version` answers it again, the program's `uninstall` removes the store
//   root, and `npm rm --global` removes the carrier.
// - update: a published release can be updated to, the check that runs before the update channel
//   moves, with the contract of the Windows script's `update` (scripts/release-update-check.mjs).
//   The version people have now is the one the website's tracked update pair of this machine's
//   platform names (apps/website/public/update/stable/<platform>/, or the folder `--current`
//   names); without one, the release updates an empty scratch store. HOME and TMPDIR lie in the
//   work folder, so `install` links `~/.local/bin/<stem>` there, and `update` runs through that
//   link with `ORCHBEE_UPDATE_FEED` naming the release's feed asset.
// - window-update: the real-window Update check (scripts/release-window-update.mjs) of a packaged
//   Test window, the folder `pnpm desktop:package:test` writes below apps/desktop/out/test, on the
//   display `DISPLAY` names (`xvfb-run` in the workflow). HOME and TMPDIR lie in the work folder, and
//   `ORCHBEE_INSTALL_DIR` and `ORCHBEE_HOME` name the default store root and data root there.
//
// A development or test build's `uninstall` takes `--confirm`; a release build refuses it and asks a
// person to type `orchbee` instead, so the release identity's steps run `uninstall` on a terminal
// that python3 opens and type that word at its prompt.
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
import {
	parseUpdateArguments,
	StepFailure,
	updateCheck,
	updateFailureAdvice,
	UsageError,
} from './release-update-check.mjs';
import {
	parseWindowUpdateArguments,
	startDetached,
	windowUpdateCheck,
	windowUpdateFailureAdvice,
} from './release-window-update.mjs';
import { FEED_DOMAIN, loadKey, signatureDocument } from './sign-release-manifest.mjs';

const usage =
	'Usage: node scripts/release-smoke-linux.mjs <archive|install-script|packages|npm> <artifacts> --work <absolute new directory>\n' +
	'       node scripts/release-smoke-linux.mjs update v<version> --release <assets address> --work <absolute new directory> [--stem <stem>] [--current <absolute folder>]\n' +
	'       node scripts/release-smoke-linux.mjs window-update <packaged Test window> --work <absolute new directory> [--diagnostics <absolute directory>]';
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
// The release stem, whose builds ask for this word in place of `uninstall --confirm`.
const releaseStem = 'orchbee';
// The release prompt in the locale the typed run sets.
const uninstallPrompt = 'Type orchbee to uninstall:';
// Runs argv[3:] on a new terminal, copies its output, types argv[1] and Enter once argv[2] shows,
// and exits as the program did.
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
	const installers = (suffix) =>
		entries
			.filter(
				(entry) =>
					entry.isFile() && entry.name.endsWith(`-${version}-${manifest.platform}${suffix}`),
			)
			.map((entry) => path.join(artifacts, entry.name));
	return {
		artifacts,
		appImages: installers('.AppImage'),
		debianPackages: installers('.deb'),
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
function run(program, argv, { env = process.env, deadline = commandDeadline, cwd } = {}) {
	log(`> ${program} ${argv.join(' ')}`);
	const result = spawnSync(program, argv, {
		cwd,
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
	uninstall(identity, link, env);
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
	const window = JSON.parse(servedFeed.toString('utf8'))?.extensions?.desktop;
	if (typeof window?.archive?.url !== 'string' || typeof window.version !== 'string')
		throw new StepFailure('The feed names no window archive in extensions.desktop');
	for (const file of [identity.archive, path.join(identity.artifacts, window.archive.url)])
		symlinkSync(file, path.join(served, path.basename(file)));
	const env = {
		...scratchEnvironment(home),
		TMPDIR: temporary,
		ORCHBEE_INSTALL_SH_FEED: `file://${path.join(served, 'feed.json')}`,
	};
	log(`> /bin/sh -s -- --desktop < ${installScript}`);
	const result = spawnSync('/bin/sh', ['-s', '--', '--desktop'], {
		encoding: 'utf8',
		env,
		input: readFileSync(installScript),
		timeout: setupDeadline,
		maxBuffer: 64 * 1024 * 1024,
	});
	if (result.error !== undefined) throw new StepFailure(`/bin/sh: ${result.error.message}`);
	process.stdout.write(`${result.stdout ?? ''}${result.stderr ?? ''}`);
	expectStatus(result, 0, 'install.sh --desktop');
	const left = readdirSync(temporary);
	if (left.length > 0)
		throw new StepFailure(`install.sh left ${left.join(', ')} in its temporary folder`);
	const root = storeRoot(identity, home);
	expectActive(identity, root);
	const link = expectLink(identity, home, root, env, 'install.sh');
	const menuFiles = expectWindowStore(identity, home, root, window.version);
	log(`install.sh installed ${identity.version} with the desktop app ${window.version}.`);
	uninstall(identity, link, env);
	expectGone(root, 'The store root');
	expectGone(link, 'The command link');
	for (const file of menuFiles) expectGone(file, 'The menu file');
	expectNoProcesses(work);
}

/**
 * Requires the window store under `root` to name `version` active, that version's window program to
 * be in place and the entry to be executable; answers the entry.
 */
function expectWindowVersion(root, stem, version) {
	const desktop = path.join(root, 'desktop');
	const pointer = path.join(desktop, 'active.json');
	const active = existsSync(pointer)
		? JSON.parse(readFileSync(pointer, 'utf8'))?.active?.product
		: undefined;
	if (active !== version)
		throw new StepFailure(`${pointer} names ${active}, not the window version ${version}`);
	const stored = path.join(desktop, 'versions', version);
	for (const file of ['desktop-files.json', `${stem}-desktop`])
		if (!existsSync(path.join(stored, file)))
			throw new StepFailure(`The window version ${stored} holds no ${file}`);
	const entry = path.join(desktop, `${stem}-desktop`);
	// The store keeps its files private, so the entry is its owner's program alone.
	if (((statSync(entry, { throwIfNoEntry: false })?.mode ?? 0) & 0o100) === 0)
		throw new StepFailure(`The window entry ${entry} is missing or not executable`);
	return entry;
}

/**
 * Requires the window store under `root` to name `version` active with an executable entry, and the
 * menu entry under `home` to start that entry with its icon beside it; answers the menu files.
 */
function expectWindowStore(identity, home, root, version) {
	const entry = expectWindowVersion(root, identity.stem, version);
	const data = path.join(home, '.local', 'share');
	const menuEntry = path.join(data, 'applications', `${identity.stem}.desktop`);
	const exec = readFileSync(menuEntry, 'utf8')
		.split('\n')
		.filter((line) => line.startsWith('Exec='));
	if (exec.length === 0 || exec.some((line) => line !== `Exec="${entry}"`))
		throw new StepFailure(`${menuEntry} does not start ${entry}: ${exec.join(' | ')}`);
	const icon = path.join(data, 'icons', 'hicolor', '256x256', 'apps', `${identity.stem}.png`);
	if (!existsSync(icon)) throw new StepFailure(`The menu icon ${icon} is missing`);
	log(`The window store runs ${version}, and ${menuEntry} starts it.`);
	return [menuEntry, icon];
}

/** Requires `file` below `folder` to exist, and with `executable` to be executable. */
function expectFile(folder, file, executable, what) {
	const mode = statSync(path.join(folder, file), { throwIfNoEntry: false })?.mode;
	if (mode === undefined || (executable && (mode & 0o111) !== 0o111))
		throw new StepFailure(`${what} holds no${executable ? ' executable' : ''} ${file}`);
}

/**
 * The AppImage extracts itself into an AppDir holding the window, and the Debian package holds the
 * row's files under the row's fields.
 */
function packagesStep(identity, work) {
	if (identity.appImages.length !== 1 || identity.debianPackages.length !== 1)
		throw new StepFailure('The artifacts must hold one AppImage and one Debian package');
	const program = `${identity.stem}-desktop`;
	const [appImage] = identity.appImages;
	const extracted = path.join(work, 'appimage');
	mkdirSync(extracted);
	expectStatus(
		run(appImage, ['--appimage-extract'], {
			cwd: extracted,
			env: scratchEnvironment(work),
			deadline: setupDeadline,
		}),
		0,
		'--appimage-extract',
	);
	const appDirectory = path.join(extracted, 'squashfs-root');
	for (const [file, executable] of [
		['AppRun', true],
		[program, true],
		[`${identity.stem}.desktop`, false],
		[`${identity.stem}.png`, false],
		[path.join('resources', 'cohort', 'release.json'), false],
	])
		expectFile(appDirectory, file, executable, 'The AppImage');
	const client = path.join('resources', 'cohort', identity.stem);
	if ((statSync(path.join(appDirectory, client)).mode & 0o777) !== 0o755)
		throw new StepFailure(`The AppImage's ${client} is not open to every user`);
	log(`${path.basename(appImage)} holds the window.`);

	const [deb] = identity.debianPackages;
	const architecture = { 'linux-x86_64': 'amd64', 'linux-aarch64': 'arm64' }[identity.platform];
	for (const [name, expected] of [
		['Package', identity.stem],
		['Version', identity.version.replace('-', '~')],
		['Architecture', architecture],
	]) {
		const value = run('dpkg-deb', ['--field', deb, name]).stdout.trim();
		if (value !== expected)
			throw new StepFailure(`The Debian package's ${name} is ${value}, not ${expected}`);
	}
	const listed = run('dpkg-deb', ['--contents', deb]).stdout.split('\n');
	// Every user runs the installed window and its bundled Client, so both are root's and 0755.
	for (const [file, mode] of [
		[`./opt/${identity.stem}/${program}`, '-rwxr-xr-x'],
		[`./opt/${identity.stem}/chrome-sandbox`, '-'],
		[`./opt/${identity.stem}/resources/cohort/`, 'drwxr-xr-x'],
		[`./opt/${identity.stem}/resources/cohort/${identity.stem}`, '-rwxr-xr-x'],
		[`./usr/share/applications/${identity.stem}.desktop`, '-rw-r--r--'],
		[`./usr/share/icons/hicolor/256x256/apps/${identity.stem}.png`, '-rw-r--r--'],
		[`./usr/share/${identity.stem}/apparmor-profile`, '-rw-r--r--'],
	])
		if (!listed.some((line) => line.startsWith(mode) && line.endsWith(` ${file}`)))
			throw new StepFailure(`The Debian package holds no ${file} with mode ${mode}`);
	const control = path.join(work, 'control');
	expectStatus(run('dpkg-deb', ['--control', deb, control]), 0, 'dpkg-deb --control');
	for (const script of ['postinst', 'postrm'])
		expectFile(control, script, true, 'The Debian package');
	log(`${path.basename(deb)} holds the window under /opt/${identity.stem}.`);
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
	const packages = [identity.stem, `@${identity.stem}/cli-linux-${identity.cpu}`];
	expectStatus(
		run('npm', ['rm', ...common, ...packages], { env, deadline: setupDeadline }),
		0,
		'npm rm',
	);
	expectGone(command, 'The npm command');
	expectNoProcesses(work);
}

/**
 * Every process whose program lies in `work`: a window's main process runs `<stem>-desktop` from a
 * window version folder without Chromium's `--type`, and a window launcher is the store's entry
 * `desktop/<stem>-desktop`.
 */
function windowProcesses(work) {
	return processesInside(work).map((line) => {
		const [, pid, program] = /^(\d+) (.+)$/u.exec(line);
		// Chromium retitles its child processes, which joins their arguments with spaces.
		let argv = [];
		try {
			argv = readFileSync(path.join('/proc', pid, 'cmdline'), 'utf8')
				.split('\0')
				.flatMap((each) => each.split(' '));
		} catch {
			// Gone already.
		}
		const desktopProgram = program.endsWith('-desktop');
		const launcher = desktopProgram && path.basename(path.dirname(program)) === 'desktop';
		return {
			pid: Number(pid),
			program,
			window: desktopProgram && !launcher && !argv.some((each) => each.startsWith('--type=')),
			launcher,
		};
	});
}

/**
 * How the real-window Update check finds a Linux Test window's files and processes
 * (scripts/release-window-update.mjs): the Windows layout with POSIX names, and HOME, TMPDIR, the
 * store root and the data root in the work folder.
 */
const windowUpdatePort = {
	application: () => undefined,
	cohort: (window) => path.join(window, 'resources', 'cohort'),
	client: (folder, stem) => path.join(folder, stem),
	asar: (window) => path.join(window, 'resources', 'app.asar'),
	seal: () => undefined,
	environment(work, stem) {
		const home = path.join(work, 'home');
		const temporary = path.join(work, 'tmp');
		mkdirSync(home, { recursive: true });
		mkdirSync(temporary, { recursive: true });
		return {
			...scratchEnvironment(home),
			TMPDIR: temporary,
			ORCHBEE_HOME: path.join(home, `.${stem}`),
			ORCHBEE_INSTALL_DIR: storeRoot({ stem }, home),
		};
	},
	storedCohort: (root, version) =>
		path.join(root, 'desktop', 'versions', version, 'resources', 'cohort'),
	// A Linux relaunch starts the window entry, which starts the active version.
	relaunchStarter: (root, stem) => path.join(root, 'desktop', `${stem}-desktop`),
	startWindow({ env, stem, storeRoot: root }) {
		const desktop = path.join(root, 'desktop');
		const entry = path.join(desktop, `${stem}-desktop`);
		startDetached(entry, [], { cwd: desktop, env });
		return entry;
	},
	relaunchedProgram: ({ stem, storeRoot: root, version }) =>
		path.join(root, 'desktop', 'versions', version, `${stem}-desktop`),
	shell: (work) => ({
		argv: ['--exec', '/bin/sh', '--arg', '-i', '--cwd', work],
		ticking: 'i=0; while [ $i -lt 3600 ]; do i=$((i+1)); echo tick-$i; sleep 1; done',
	}),
	processes: windowProcesses,
};

/**
 * Requires the store root to name `version` active, its Client to answer it, and the command link,
 * when it names a Client of this store, to name that one.
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
		// A scratch store that ORCHBEE_INSTALL_DIR names keeps the link of the store it came from.
		if (env.ORCHBEE_INSTALL_DIR === undefined)
			throw new StepFailure(`${link} names no Client of ${root}`);
		return;
	}
	if (named !== realpathSync(client))
		throw new StepFailure(`${link} names ${named}, not ${client}`);
	log(`${link} names the Client of ${version}.`);
}

/**
 * How the update check installs and updates on this machine (scripts/release-update-check.mjs):
 * HOME and TMPDIR in the work folder, so the store root, the command link, the menu entry and the
 * data root lie inside it.
 */
const updatePort = {
	platform: hostPlatform,
	tar: '/usr/bin/tar',
	client: (directory, stem) => path.join(directory, stem),
	versionClient: (root, stem, version) => path.join(root, 'versions', version, stem),
	bundledWindowClient: (windowDirectory, stem) =>
		path.join(windowDirectory, 'resources', 'cohort', stem),
	environment(work) {
		const home = path.join(work, 'home');
		const temporary = path.join(work, 'tmp');
		for (const folder of [home, temporary]) mkdirSync(folder);
		return { ...scratchEnvironment(home), TMPDIR: temporary };
	},
	storeRoot: (stem, env) => env.ORCHBEE_INSTALL_DIR ?? storeRoot({ stem }, env.HOME),
	updater: (_root, stem, env) => path.join(env.HOME, '.local', 'bin', stem),
	expectActive: expectRunning,
	expectWindowActive: (root, stem, version) => {
		expectWindowVersion(root, stem, version);
		log(`The window store runs ${version}.`);
	},
};

function parseArguments(argv) {
	if (argv[0] === 'update') return parseUpdateArguments(argv.slice(1), usage, hostPlatform);
	if (argv[0] === 'window-update') return parseWindowUpdateArguments(argv.slice(1), usage);
	const [step, artifacts, option, work, ...rest] = argv;
	if (
		!['archive', 'install-script', 'packages', 'npm'].includes(step) ||
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
		if (process.platform !== 'linux') throw new StepFailure('This smoke test runs on Linux');
		if (hostPlatform === undefined)
			throw new StepFailure(`${process.arch} is not a Linux release platform`);
		options = parseArguments(process.argv.slice(2));
		const identity = ['update', 'window-update'].includes(options.step)
			? undefined
			: readIdentity(options.artifacts);
		if (existsSync(options.work))
			throw new StepFailure(`Refusing to work in an existing ${options.work}`);
		mkdirSync(options.work, { recursive: true });
		// The real path, so the process scan compares the programs /proc reports.
		work = realpathSync(options.work);
		if (options.step === 'update') {
			updateCheck({ ...options, work }, progress, updatePort, usage);
			expectNoProcesses(work);
		} else if (options.step === 'window-update') {
			await windowUpdateCheck(options, work, progress, windowUpdatePort);
			expectNoProcesses(work);
		} else {
			const steps = { archive, 'install-script': installScriptStep, packages: packagesStep, npm };
			steps[options.step](identity, work);
		}
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
		// The channel must not move to a release nobody could update to.
		if (options?.step === 'update' && !(error instanceof UsageError))
			process.stderr.write(updateFailureAdvice(options.version, progress.stage));
		if (options?.step === 'window-update' && !(error instanceof UsageError))
			process.stderr.write(windowUpdateFailureAdvice(progress.stage));
	} finally {
		if (work !== undefined) rmSync(work, { recursive: true, force: true });
	}
}
