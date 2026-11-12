// The `orchbee` npm package's launcher. npm carries the signed release directory in the platform
// sub-package; on each start this finds it, installs it into the per-user store through the carried
// program's own `install --no-path` when the store has no active version or a lower one, and then runs
// the store's active program in place of itself. The store's pointer `active.json` alone names the
// active version. Nothing runs at install time.
//
// The launcher is plain JavaScript that runs on the person's own Node, so it uses only Node built-ins
// available since Node 18 and reads nothing beyond its own package, the sub-package, and the pointer.
import { spawn, spawnSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';
import { createRequire } from 'node:module';
import { constants } from 'node:os';
import path from 'node:path';

// SemVer 2.0.0 without build metadata, at most 64 characters, as the store admits it.
const identifier = String.raw`(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)`;
const semanticVersion = new RegExp(
	String.raw`^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-${identifier}(?:\.${identifier})*)?$`,
	'u',
);
const manifestDigest = /^[0-9a-f]{64}$/u;
// The pointer is a few hundred bytes; anything far larger is not one.
const maximumPointer = 64 * 1024;
// A concurrent start holds `install.lock` for one install; the install's own deadline is 600 s.
const busyWaitMilliseconds = 600_000;
const busyRetryMilliseconds = 250;

const platformNames = { win32: 'Windows', darwin: 'macOS', linux: 'Linux' };
const architectureNames = { x64: 'x64', arm64: 'ARM64' };

const messages = {
	'en-US': {
		unsupported: (here, supported) => `Orchbee does not run on ${here}. It runs on ${supported}.`,
		missing: (name) =>
			`The package ${name} is missing. Reinstall orchbee without omitting optional dependencies.`,
		installing: (version) => `Installing Orchbee ${version}…`,
		installFailed: (version, code) => `Orchbee ${version} was not installed (${code}).`,
		storeMismatch: (installed, expected) =>
			`Orchbee was installed in ${installed}, but this launcher reads ${expected}. Reinstall orchbee.`,
		startFailed: (program, code) => `Orchbee could not start ${program} (${code}).`,
	},
	'zh-CN': {
		unsupported: (here, supported) => `Orchbee 不支持 ${here}，支持的平台：${supported}。`,
		missing: (name) => `缺少软件包 ${name}。请重新安装 orchbee，不要省略可选依赖。`,
		installing: (version) => `正在安装 Orchbee ${version}…`,
		installFailed: (version, code) => `Orchbee ${version} 未能安装（${code}）。`,
		storeMismatch: (installed, expected) =>
			`Orchbee 安装到了 ${installed}，但启动器读取的是 ${expected}。请重新安装 orchbee。`,
		startFailed: (program, code) => `Orchbee 无法启动 ${program}（${code}）。`,
	},
};

/** The CLI's locale rule: the first non-empty of LC_ALL, LC_MESSAGES and LANG; `zh…` is zh-CN. */
export function selectLocale(environment) {
	for (const name of ['LC_ALL', 'LC_MESSAGES', 'LANG']) {
		const value = environment[name];
		if (value === undefined || value === '') continue;
		return value.slice(0, 2).toLowerCase() === 'zh' ? 'zh-CN' : 'en-US';
	}
	return 'en-US';
}

// Admitted numeric identifiers have no leading zero, so length orders them before digits do, at any
// size.
function compareNumeric(left, right) {
	if (left.length !== right.length) return left.length - right.length;
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

/** SemVer 2.0.0 precedence of two admitted versions: negative, zero, or positive. */
export function compareVersions(left, right) {
	const [leftCore, ...leftRest] = left.split('-');
	const [rightCore, ...rightRest] = right.split('-');
	const leftParts = leftCore.split('.');
	const rightParts = rightCore.split('.');
	for (let index = 0; index < 3; index++) {
		const order = compareNumeric(leftParts[index], rightParts[index]);
		if (order !== 0) return order;
	}
	const leftPre = leftRest.join('-');
	const rightPre = rightRest.join('-');
	if (leftPre === rightPre) return 0;
	// A version without a pre-release part ranks above every pre-release of the same core.
	if (leftPre === '') return 1;
	if (rightPre === '') return -1;
	const leftIds = leftPre.split('.');
	const rightIds = rightPre.split('.');
	for (let index = 0; index < Math.min(leftIds.length, rightIds.length); index++) {
		const a = leftIds[index];
		const b = rightIds[index];
		if (a === b) continue;
		const aNumeric = /^\d+$/u.test(a);
		const bNumeric = /^\d+$/u.test(b);
		if (aNumeric && bNumeric) return compareNumeric(a, b);
		// Numeric identifiers rank below alphanumeric ones; the rest compare in ASCII order.
		if (aNumeric) return -1;
		if (bNumeric) return 1;
		return a < b ? -1 : 1;
	}
	return leftIds.length - rightIds.length;
}

function validVersion(value) {
	return typeof value === 'string' && value.length <= 64 && semanticVersion.test(value);
}

function plainObject(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function closedObject(value, required, optional) {
	if (!plainObject(value)) return false;
	const keys = Object.keys(value);
	return (
		required.every((key) => keys.includes(key)) &&
		keys.every((key) => required.includes(key) || optional.includes(key))
	);
}

function validEntry(value) {
	return (
		closedObject(value, ['product', 'manifest'], []) &&
		validVersion(value.product) &&
		typeof value.manifest === 'string' &&
		manifestDigest.test(value.manifest)
	);
}

/**
 * The active product version a format-1 pointer names, or null for any other bytes: a missing or
 * unreadable pointer means no active version, which an install repairs.
 */
export function parsePointer(text) {
	let value;
	try {
		value = JSON.parse(text);
	} catch {
		return null;
	}
	if (
		!closedObject(value, ['version', 'active'], ['previous', 'extensions']) ||
		value.version !== 1 ||
		!validEntry(value.active) ||
		(value.previous !== undefined && !validEntry(value.previous)) ||
		(value.extensions !== undefined && !plainObject(value.extensions))
	)
		return null;
	return value.active.product;
}

/**
 * The store root the carried program installs into, by the rule of `orchbee install`:
 * `ORCHBEE_INSTALL_DIR` in every environment; otherwise on Windows `%LOCALAPPDATA%\Programs\Orchbee`
 * for the release stem and `%LOCALAPPDATA%\Programs\<stem>` for a test stem, and elsewhere
 * `~/.local/share/<stem>`. Null when no absolute root follows, which the install then refuses.
 */
export function storeRoot(stem, platform, environment) {
	const paths = platform === 'win32' ? path.win32 : path.posix;
	const configured = environment.ORCHBEE_INSTALL_DIR;
	if (configured !== undefined) return paths.isAbsolute(configured) ? configured : null;
	if (platform === 'win32') {
		const local = environment.LOCALAPPDATA;
		if (!local || !paths.isAbsolute(local)) return null;
		return paths.join(local, 'Programs', stem === 'orchbee' ? 'Orchbee' : stem);
	}
	const home = environment.HOME;
	return home && paths.isAbsolute(home) ? paths.join(home, '.local', 'share', stem) : null;
}

function readPointer(root) {
	if (root === null) return null;
	const file = path.join(root, 'active.json');
	try {
		if (statSync(file).size > maximumPointer) return null;
		return parsePointer(readFileSync(file, 'utf8'));
	} catch {
		return null;
	}
}

function samePath(left, right) {
	const a = path.resolve(left);
	const b = path.resolve(right);
	return process.platform === 'win32' ? a.toLowerCase() === b.toLowerCase() : a === b;
}

function sleep(milliseconds) {
	Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function fail(line, status = 1) {
	process.stderr.write(line + '\n');
	process.exit(status);
}

/**
 * Runs the carried program's `install --no-path` and reads its `orchbee.install` result. Its
 * localized diagnostic is kept for a failure the launcher reports, not shown while it waits.
 */
function install(program) {
	const result = spawnSync(program, ['install', '--no-path', '--format', 'json'], {
		stdio: ['ignore', 'pipe', 'pipe'],
		encoding: 'utf8',
		maxBuffer: 1024 * 1024,
	});
	let document = null;
	try {
		document = JSON.parse(result.stdout ?? '');
	} catch {
		// Anything but the result document reads as an unknown failure below.
	}
	const outcome =
		document !== null && document.schema === 'orchbee.install' ? document.result : null;
	return {
		status: result.status ?? 1,
		error: outcome?.error ?? (result.error ? (result.error.code ?? 'spawn') : 'unknown'),
		activated: outcome?.activated === true,
		releasePath: typeof outcome?.releasePath === 'string' ? outcome.releasePath : null,
		diagnostic: result.stderr ?? '',
	};
}

/** The installed program to run: the active one, after installing the carried version if needed. */
function resolveProgram({ root, programName, carriedProgram, carriedVersion, text }) {
	const deadline = Date.now() + busyWaitMilliseconds;
	let installed = false;
	let announced = false;
	for (;;) {
		const active = readPointer(root);
		if (active !== null && (installed || compareVersions(carriedVersion, active) <= 0))
			return path.join(root, 'versions', active, programName);
		if (installed) fail(text.installFailed(carriedVersion, 'activation'));
		if (!announced) {
			process.stderr.write(text.installing(carriedVersion) + '\n');
			announced = true;
		}
		const outcome = install(carriedProgram);
		if (outcome.activated && outcome.releasePath !== null) {
			// The program reports where it published; a launcher that reads another root would
			// install again on every start, so the two must agree.
			const expected = root === null ? null : path.join(root, 'versions', carriedVersion);
			if (expected === null || !samePath(outcome.releasePath, expected))
				fail(text.storeMismatch(path.dirname(path.dirname(outcome.releasePath)), root ?? '-'));
			installed = true;
			continue;
		}
		// Another start holds `install.lock`: wait for it, then read the pointer again.
		if (outcome.error === 'busy' && Date.now() < deadline) {
			sleep(busyRetryMilliseconds);
			continue;
		}
		if (outcome.error === 'cancelled') process.exit(outcome.status);
		process.stderr.write(outcome.diagnostic);
		fail(text.installFailed(carriedVersion, outcome.error), outcome.status || 1);
	}
}

/**
 * Runs `program` with this process's arguments, standard handles and environment and leaves with its
 * exit status. Console signals (Ctrl+C, Ctrl+Break, close) and terminal signals reach the program
 * directly, so the launcher only outlives them; a signal sent to the launcher alone is passed on.
 */
function run(program, text) {
	const child = spawn(program, process.argv.slice(2), { stdio: 'inherit' });
	for (const signal of forwardedSignals) process.on(signal, () => child.kill(signal));
	child.on('error', (error) => fail(text.startFailed(program, error.code ?? error.message)));
	child.on('exit', (code, signal) => {
		if (code !== null) process.exit(code);
		// Leave by the same signal, so the caller sees what the program saw.
		process.removeAllListeners(signal);
		process.kill(process.pid, signal);
		process.exit(128 + (constants.signals[signal] ?? 0));
	});
}

// Signals the console or terminal delivers to every process attached to it: the program receives
// them itself, so the launcher ignores them instead of passing them on a second time.
const ignoredSignals =
	process.platform === 'win32' ? ['SIGINT', 'SIGBREAK', 'SIGHUP'] : ['SIGINT', 'SIGQUIT'];
// Signals sent to the launcher's process alone.
const forwardedSignals = process.platform === 'win32' ? [] : ['SIGTERM', 'SIGHUP'];

export function main() {
	const text = messages[selectLocale(process.env)];
	// Registered first, so an install started below also receives Ctrl+C alone and is waited for.
	for (const signal of ignoredSignals) process.on(signal, () => {});
	const require = createRequire(import.meta.url);
	const own = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
	const stem = own.name;
	const subPackages = Object.keys(own.optionalDependencies ?? {});
	const suffix = `-${process.platform}-${process.arch}`;
	const subPackage = subPackages.find((name) => name.endsWith(`/cli${suffix}`));
	if (subPackage === undefined) {
		const describe = (platform, architecture) =>
			`${platformNames[platform] ?? platform} ${architectureNames[architecture] ?? architecture}`;
		const supported = subPackages
			.map((name) => /\/cli-([a-z0-9]+)-([a-z0-9]+)$/u.exec(name))
			.filter((match) => match !== null)
			.map((match) => describe(match[1], match[2]))
			.join(', ');
		fail(text.unsupported(describe(process.platform, process.arch), supported));
	}
	let carriedPackage;
	try {
		carriedPackage = require.resolve(`${subPackage}/package.json`);
	} catch {
		fail(text.missing(subPackage));
	}
	const carried = JSON.parse(readFileSync(carriedPackage, 'utf8'));
	if (!validVersion(carried.version)) fail(text.missing(subPackage));
	const programName = process.platform === 'win32' ? `${stem}.exe` : stem;
	const program = resolveProgram({
		root: storeRoot(stem, process.platform, process.env),
		programName,
		carriedProgram: path.join(path.dirname(carriedPackage), 'release', programName),
		carriedVersion: carried.version,
		text,
	});
	run(program, text);
}
