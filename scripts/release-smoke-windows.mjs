// Proves a built Windows release on a fresh machine, one step per subcommand so that a workflow shows
// which step failed: the installer route, the npm route, and a scan for anything of the build's
// identity left behind. It installs and removes for real, so run it only on a disposable machine,
// such as a fresh CI runner, never on one whose Orchbee installation matters.
//
//   node scripts/release-smoke-windows.mjs install   <artifacts> --logs <absolute directory>
//   node scripts/release-smoke-windows.mjs command   <artifacts>
//   node scripts/release-smoke-windows.mjs uninstall <artifacts> --logs <absolute directory>
//   node scripts/release-smoke-windows.mjs npm       <artifacts> --prefix <absolute new directory>
//   node scripts/release-smoke-windows.mjs leftovers <artifacts>
//
// <artifacts> is a directory `node scripts/release-artifacts.mjs collect` wrote; its release directory
// names the version and the program stem, so the same lines serve the test and the release identity.
//
// - install: the setup file with Inno Setup's silent switches exits 0 and leaves the pointer and the
//   launcher in the store root.
// - command: a new process whose `Path` is the machine value plus the user value, as a new terminal
//   gets it, finds the command in `<store root>\bin`; `--version` answers the version; `server start`
//   and `status` exit 0; a release build refuses each withheld command with exit 4.
// - uninstall: the registered uninstaller with the silent switches exits 0, and its copy that keeps
//   running after the exit code has finished.
// - npm: `npm install -g --offline` of the tarballs into the prefix; the first start installs the
//   carried version, `--version` answers it twice, the program's uninstall exits 0, and
//   `npm rm -g` removes the carrier.
// - leftovers: once the removal has settled, nothing of the identity remains in %LOCALAPPDATA%,
//   %APPDATA% or %TEMP%, among the uninstall registrations, or in the user `Path`; the data root stays.
//
// A test build's uninstall that runs with `ORCHBEE_TEST_REGISTRY_KEY` (`Software\ob-test-scratch\<name>`
// below HKCU) uses that scratch key's `Environment` and `Uninstall` subkeys instead of the user's, and
// so does this script; `ORCHBEE_INSTALL_DIR` and `ORCHBEE_HOME` move the store and data roots for both.
//
// Exit codes: 0 the step passed; 1 it failed; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { readRelease } from './package-npm.mjs';

const usage =
	'Usage: node scripts/release-smoke-windows.mjs <install|uninstall> <artifacts> --logs <absolute directory>\n' +
	'       node scripts/release-smoke-windows.mjs <command|leftovers> <artifacts>\n' +
	'       node scripts/release-smoke-windows.mjs npm <artifacts> --prefix <absolute new directory>';
// Each command a person runs gets this long; an install or a removal gets longer.
const commandDeadline = 120_000;
const setupDeadline = 600_000;
// The uninstaller's copy and the Client's deferred file removal finish about a minute after exit.
const settleDeadline = 180_000;
const settleInterval = 3_000;
// The release surfaces a release build withholds (apps/orchbee/src/invocation/grammar/release.rs);
// a refusal exits 4.
const withheldCommands = [['plugin'], ['popup'], ['update'], ['channel']];
const withheldStatus = 4;
const silentSwitches = ['/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART'];
// Bounds of the name scan below each profile folder.
const scanDepth = 8;
const scanEntries = 500_000;

class UsageError extends Error {}
class StepFailure extends Error {}

function system32(program) {
	return path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', program);
}

function sleep(milliseconds) {
	Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function samePath(left, right) {
	return path.resolve(left).toLowerCase() === path.resolve(right).toLowerCase();
}

function inside(parent, child) {
	const relative = path.relative(path.resolve(parent), path.resolve(child));
	return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function log(line) {
	process.stdout.write(`${line}\n`);
}

/** The last lines of a log file, for a failure that the log explains. */
function logTail(filename, lines = 60) {
	if (!existsSync(filename)) return `(no log at ${filename})`;
	return readFileSync(filename, 'utf8').split(/\r?\n/u).slice(-lines).join('\n');
}

/** Everything the steps need to know about the build, read from the collected artifacts. */
function readIdentity(artifacts) {
	const entries = readdirSync(artifacts, { withFileTypes: true });
	const releases = entries.filter(
		(entry) => entry.isDirectory() && existsSync(path.join(artifacts, entry.name, 'release.json')),
	);
	const setups = entries.filter((entry) => entry.isFile() && entry.name.endsWith('-setup.exe'));
	if (releases.length !== 1 || setups.length !== 1)
		throw new StepFailure(`${artifacts} must hold one release directory and one setup file`);
	const releaseDirectory = path.join(artifacts, releases[0].name);
	const { version } = JSON.parse(readFileSync(path.join(releaseDirectory, 'release.json'), 'utf8'));
	const { stem, manifest } = readRelease(releaseDirectory, version);
	if (manifest.platform !== 'windows-x86_64' && manifest.platform !== 'windows-aarch64')
		throw new StepFailure(`${manifest.platform} is not a Windows release`);
	const release = stem === 'orchbee';
	// The installer's names: a release is `Orchbee`; a test build uses its stem in every name.
	const displayName = release ? 'Orchbee' : stem;
	const environment = process.env;
	const programs = environment.LOCALAPPDATA ?? '';
	const storeRoot = environment.ORCHBEE_INSTALL_DIR ?? path.join(programs, 'Programs', displayName);
	const dataRoot = environment.ORCHBEE_HOME ?? path.join(environment.USERPROFILE ?? '', `.${stem}`);
	const scratchKey = release ? undefined : environment.ORCHBEE_TEST_REGISTRY_KEY;
	const keyRoot = scratchKey === undefined ? undefined : `HKCU\\${scratchKey}`;
	return {
		artifacts,
		version,
		stem,
		release,
		displayName,
		setup: path.join(artifacts, setups[0].name),
		tarballs: entries
			.filter((entry) => entry.isFile() && entry.name.endsWith('.tgz'))
			.map((entry) => path.join(artifacts, entry.name)),
		storeRoot,
		dataRoot,
		// Inno Setup always registers under the user's own key; only the program honours the scratch key.
		installerRegistration: `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\com.${stem}.desktop_is1`,
		uninstallKey:
			keyRoot === undefined
				? 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall'
				: `${keyRoot}\\Uninstall`,
		environmentKey: keyRoot === undefined ? 'HKCU\\Environment' : `${keyRoot}\\Environment`,
	};
}

/**
 * Runs one program and returns its status and output; a missing program or a timeout fails. An
 * installer or uninstaller shares this process's output instead, so that a helper it leaves running
 * cannot hold a pipe open until the deadline.
 */
function run(
	program,
	argv,
	{ env = process.env, deadline = commandDeadline, quiet = false, capture = true } = {},
) {
	const result = spawnSync(program, argv, {
		encoding: 'utf8',
		env,
		stdio: capture ? ['ignore', 'pipe', 'pipe'] : ['ignore', 'inherit', 'inherit'],
		timeout: deadline,
		windowsHide: true,
	});
	if (result.error !== undefined)
		throw new StepFailure(`${program} ${argv.join(' ')}: ${result.error.message}`);
	if (!quiet) {
		if (result.stdout) process.stdout.write(result.stdout);
		if (result.stderr) process.stdout.write(result.stderr);
	}
	return { status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}

/** One registry value as `reg query` reports it, or undefined when the key or value is absent. */
function readRegistryValue(key, name) {
	const result = spawnSync(system32('reg.exe'), ['query', key, '/v', name], {
		encoding: 'utf8',
		windowsHide: true,
	});
	if (result.status !== 0) return undefined;
	for (const line of result.stdout.split(/\r?\n/u)) {
		const match = /^\s+(.+?)\s{4}(REG_\w+)\s{4}(.*)$/u.exec(line);
		if (match !== null && match[1].toLowerCase() === name.toLowerCase())
			return { type: match[2], data: match[3] };
	}
	return { type: 'REG_SZ', data: '' };
}

function registryKeyExists(key) {
	return spawnSync(system32('reg.exe'), ['query', key], { windowsHide: true }).status === 0;
}

/** The names of the direct subkeys of `key`. */
function registrySubkeys(key) {
	const result = spawnSync(system32('reg.exe'), ['query', key], {
		encoding: 'utf8',
		windowsHide: true,
	});
	if (result.status !== 0) return [];
	// `reg query` prints each subkey by its full name, with the hive spelled out.
	const prefix = `${key.replace(/^HKCU\\/iu, 'HKEY_CURRENT_USER\\')}\\`.toLowerCase();
	return result.stdout
		.split(/\r?\n/u)
		.filter((line) => line.toLowerCase().startsWith(prefix))
		.map((line) => line.slice(prefix.length));
}

/** `%NAME%` references expanded from this process's environment, as Windows expands REG_EXPAND_SZ. */
function expand(value) {
	return value.replace(/%([^%]+)%/gu, (whole, name) => {
		const key = Object.keys(process.env).find((each) => each.toLowerCase() === name.toLowerCase());
		return key === undefined ? whole : process.env[key];
	});
}

function pathEntries(value) {
	return value
		.split(';')
		.map((entry) => entry.trim().replace(/^"(.*)"$/u, '$1'))
		.filter((entry) => entry !== '');
}

function userPath(identity) {
	const value = readRegistryValue(identity.environmentKey, 'Path');
	if (value === undefined) return '';
	return value.type === 'REG_EXPAND_SZ' ? expand(value.data) : value.data;
}

/** This environment with `Path` replaced, whatever the spelling of its name. */
function withPath(value) {
	const env = Object.fromEntries(
		Object.entries(process.env).filter(([name]) => name.toLowerCase() !== 'path'),
	);
	env.Path = value;
	return env;
}

/** The environment a newly started terminal gets: the machine `Path`, then the user's. */
function newTerminalEnvironment(identity) {
	const machine = readRegistryValue(
		'HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment',
		'Path',
	);
	const machinePath = machine === undefined ? '' : expand(machine.data);
	return withPath([machinePath, userPath(identity)].filter((part) => part !== '').join(';'));
}

/** Runs `<stem> <argv>` through cmd.exe, which looks the command up in `env`'s `Path` as a person's shell does. */
function runCommand(identity, argv, env, deadline) {
	log(`> ${identity.stem} ${argv.join(' ')}`);
	return run(system32('cmd.exe'), ['/d', '/c', identity.stem, ...argv], { env, deadline });
}

function expectStatus(result, expected, what) {
	if (result.status !== expected)
		throw new StepFailure(`${what} exited ${result.status}, expected ${expected}`);
}

function expectVersion(identity, result, what) {
	expectStatus(result, 0, what);
	const answer = result.stdout.trim().split(/\s+/u).at(-1);
	if (answer !== identity.version)
		throw new StepFailure(
			`${what} answered ${JSON.stringify(answer)}, expected ${identity.version}`,
		);
}

function install(identity, logs) {
	mkdirSync(logs, { recursive: true });
	const logFile = path.join(logs, 'setup.log');
	log(`> ${path.basename(identity.setup)} ${silentSwitches.join(' ')}`);
	const result = run(identity.setup, [...silentSwitches, `/LOG=${logFile}`], {
		deadline: setupDeadline,
		capture: false,
	});
	if (result.status !== 0)
		throw new StepFailure(`Setup exited ${result.status}\n${logTail(logFile)}`);
	for (const required of ['active.json', path.join('bin', `${identity.stem}.exe`)]) {
		if (!existsSync(path.join(identity.storeRoot, required)))
			throw new StepFailure(`Setup left no ${required} in ${identity.storeRoot}`);
	}
	if (!registryKeyExists(identity.installerRegistration))
		throw new StepFailure(`Setup registered no ${identity.installerRegistration}`);
	log(`Installed ${identity.stem} ${identity.version} into ${identity.storeRoot}`);
}

function command(identity) {
	const env = newTerminalEnvironment(identity);
	const where = run(system32('where.exe'), [identity.stem], { env, quiet: true });
	const found = where.stdout.split(/\r?\n/u).find((line) => line !== '');
	const launcher = path.join(identity.storeRoot, 'bin', `${identity.stem}.exe`);
	if (where.status !== 0 || found === undefined)
		throw new StepFailure(`A new terminal does not find ${identity.stem} on the user Path`);
	if (!samePath(found, launcher))
		throw new StepFailure(`A new terminal finds ${found} first, not ${launcher}`);
	log(`A new terminal finds ${found}`);
	expectVersion(identity, runCommand(identity, ['--version'], env), '--version');
	expectStatus(runCommand(identity, ['server', 'start'], env), 0, 'server start');
	expectStatus(runCommand(identity, ['status'], env), 0, 'status');
	if (!identity.release) {
		log('Withheld commands: a test build offers them, so only a release build is checked.');
		return;
	}
	for (const words of withheldCommands)
		expectStatus(runCommand(identity, words, env), withheldStatus, words.join(' '));
}

function uninstall(identity, logs) {
	mkdirSync(logs, { recursive: true });
	const registered = readRegistryValue(identity.installerRegistration, 'UninstallString');
	if (registered === undefined || registered.data === '')
		throw new StepFailure(`No uninstaller is registered at ${identity.installerRegistration}`);
	const uninstaller = registered.data.trim().replace(/^"([^"]+)".*$/u, '$1');
	const logFile = path.join(logs, 'uninstall.log');
	log(`> ${uninstaller} ${silentSwitches.join(' ')}`);
	const result = run(uninstaller, [...silentSwitches, `/LOG=${logFile}`], {
		deadline: setupDeadline,
		capture: false,
	});
	if (result.status !== 0)
		throw new StepFailure(`The uninstaller exited ${result.status}\n${logTail(logFile)}`);
	// The uninstaller answers from a copy of itself that goes on removing after the exit code.
	const until = Date.now() + settleDeadline;
	while (existsSync(uninstaller) && Date.now() < until) sleep(1_000);
	if (existsSync(uninstaller))
		throw new StepFailure(`${uninstaller} is still there\n${logTail(logFile)}`);
	log(logTail(logFile, 20));
}

function npm(identity, prefix) {
	if (!path.isAbsolute(prefix)) throw new UsageError('The npm prefix must be absolute');
	if (existsSync(prefix)) throw new StepFailure(`Refusing to install into an existing ${prefix}`);
	if (identity.tarballs.length === 0) throw new StepFailure('The artifacts hold no npm tarball');
	// npm is a command script, which only cmd.exe starts.
	const npmCommand = (argv) => {
		log(`> npm ${argv.join(' ')}`);
		return run(system32('cmd.exe'), ['/d', '/c', 'npm', ...argv], { deadline: setupDeadline });
	};
	const common = ['--global', '--offline', '--no-audit', '--no-fund', '--prefix', prefix];
	expectStatus(npmCommand(['install', ...common, ...identity.tarballs]), 0, 'npm install');
	// A global prefix on Windows holds the command shims at its top.
	const env = withPath(`${prefix};${process.env.Path ?? process.env.PATH ?? ''}`);
	expectVersion(identity, runCommand(identity, ['--version'], env, setupDeadline), 'first start');
	if (!existsSync(path.join(identity.storeRoot, 'active.json')))
		throw new StepFailure(`The first start installed nothing into ${identity.storeRoot}`);
	expectVersion(identity, runCommand(identity, ['--version'], env), '--version');
	// A release build asks a person to type the confirmation, so only its platform uninstaller's
	// helper removes it without one; a test build takes `--confirm`.
	const removal = identity.release ? ['--installer-uninstall-v1'] : ['uninstall', '--confirm'];
	expectStatus(runCommand(identity, removal, env, setupDeadline), 0, removal.join(' '));
	const packages = [identity.stem, `@${identity.stem}/cli-win32-${process.arch}`];
	expectStatus(npmCommand(['rm', ...common, ...packages]), 0, 'npm rm');
}

/** Names of the identity below `root`, searched to a bounded depth without following links. */
function scanNames(root, needle, skip, findings) {
	if (!existsSync(root)) return;
	let visited = 0;
	const pending = [{ directory: root, depth: 0 }];
	while (pending.length > 0) {
		const { directory, depth } = pending.pop();
		let entries;
		try {
			entries = readdirSync(directory, { withFileTypes: true });
		} catch {
			continue;
		}
		for (const entry of entries) {
			if (++visited > scanEntries) {
				// An unfinished scan found nothing yet; it says so instead of failing or passing quietly.
				log(`Warning: the scan of ${root} stopped after ${scanEntries} entries`);
				return;
			}
			const filename = path.join(directory, entry.name);
			if (skip(filename)) continue;
			if (entry.name.toLowerCase().includes(needle)) findings.push(filename);
			else if (entry.isDirectory() && !entry.isSymbolicLink() && depth < scanDepth)
				pending.push({ directory: filename, depth: depth + 1 });
		}
	}
}

/** What remains of the identity at the places a removal must clear, cheap enough to poll. */
function quickFindings(identity) {
	const findings = [];
	if (existsSync(identity.storeRoot)) findings.push(identity.storeRoot);
	// Both the installer's registration and the program's carry the stem.
	const needle = identity.stem.toLowerCase();
	for (const name of registrySubkeys(identity.uninstallKey)) {
		if (name.toLowerCase().includes(needle)) findings.push(`${identity.uninstallKey}\\${name}`);
	}
	for (const entry of pathEntries(userPath(identity))) {
		if (entry.toLowerCase().includes(needle) || inside(identity.storeRoot, entry))
			findings.push(`user Path entry ${entry}`);
	}
	return [...new Set(findings)];
}

function leftovers(identity) {
	const until = Date.now() + settleDeadline;
	let findings = quickFindings(identity);
	while (findings.length > 0 && Date.now() < until) {
		sleep(settleInterval);
		findings = quickFindings(identity);
	}
	const needle = identity.stem.toLowerCase();
	// The data root stays until a person deletes it.
	const skip = (filename) => inside(identity.dataRoot, filename);
	for (const name of ['LOCALAPPDATA', 'APPDATA', 'TEMP']) {
		const root = process.env[name];
		if (root === undefined || !path.isAbsolute(root))
			throw new StepFailure(`%${name}% names no absolute folder`);
		scanNames(root, needle, skip, findings);
	}
	findings = [...new Set(findings)];
	if (findings.length > 0)
		throw new StepFailure(
			`Left behind by ${identity.stem}:\n${findings.map((each) => `  ${each}`).join('\n')}`,
		);
	const kept = existsSync(identity.dataRoot) ? `; the data root ${identity.dataRoot} stays` : '';
	log(`Nothing of ${identity.stem} remains${kept}.`);
}

function parseArguments(argv) {
	const [step, artifacts, ...rest] = argv;
	const options = {};
	for (let index = 0; index < rest.length; index += 2) {
		const [option, value] = [rest[index], rest[index + 1]];
		if (!['--logs', '--prefix'].includes(option) || value === undefined)
			throw new UsageError(usage);
		options[option.slice(2)] = value;
	}
	const needs = {
		install: 'logs',
		uninstall: 'logs',
		npm: 'prefix',
		command: null,
		leftovers: null,
	};
	if (artifacts === undefined || !(step in needs)) throw new UsageError(usage);
	const allowed = needs[step];
	const given = Object.keys(options);
	if (given.length !== (allowed === null ? 0 : 1) || (allowed !== null && !given.includes(allowed)))
		throw new UsageError(usage);
	if (options.logs !== undefined && !path.isAbsolute(options.logs)) throw new UsageError(usage);
	return { step, artifacts: path.resolve(artifacts), ...options };
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	try {
		if (process.platform !== 'win32') throw new StepFailure('This smoke test runs on Windows');
		const options = parseArguments(process.argv.slice(2));
		const identity = readIdentity(options.artifacts);
		const steps = {
			install: () => install(identity, options.logs),
			command: () => command(identity),
			uninstall: () => uninstall(identity, options.logs),
			npm: () => npm(identity, options.prefix),
			leftovers: () => leftovers(identity),
		};
		steps[options.step]();
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	}
}
