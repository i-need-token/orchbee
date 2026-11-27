// The `orchbee` npm package's launcher. npm carries the signed release directory in the platform
// sub-package; on each start this finds it, installs it into the per-user store through the carried
// program's own `install --no-path` when the store has no active version or a lower one, and then runs
// the store's active program in place of itself. The store's pointer `active.json` alone names the
// active version; a carried version the pointer holds after a failed move is never installed, and the
// active one runs instead. It follows the relaunch protocol of `apps/orchbee-launcher` with the same
// rule, and records a failed Client through the previous version's program as that launcher does.
// Nothing runs at install time.
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
const activationIdentifier = /^[0-9a-f]{32}$/u;
// The pointer is a few hundred bytes; anything far larger is not one.
const maximumPointer = 64 * 1024;
// A concurrent start holds `install.lock` for one install; the install's own deadline is 600 s.
const busyWaitMilliseconds = 600_000;
const busyRetryMilliseconds = 250;

// The relaunch protocol, the same as `apps/orchbee-launcher/src/relaunch.rs`: the variable carries the
// launcher's process id, and a Client uses the statuses below only when that process is its parent.
// The variable is set in both spellings, since Clients up to 0.1.0-beta.24 read only the second.
const relaunchVariables = ['ORCHBEE_RELAUNCH', 'ORCHBEE_RELAUNCH_V1'];
const relaunchStatus = 75;
// A Client that could not join its Session; without the protocol it ends as a startup failure.
const joinFailedStatus = 76;
const startupFailureStatus = 1;
// Rust's status for a panic that unwinds out of `main`; no CLI exit code uses it.
const panicStatus = 101;
// Error-severity NTSTATUS values end a process that did not choose to end; STATUS_CONTROL_C_EXIT is a
// console Ctrl+C, Ctrl+Break or close.
const errorSeverity = 0xc0000000;
const consoleExitStatus = 0xc000013a;
const relaunchLimit = 3;
const relaunchWindowMilliseconds = 10_000;
// The helper that records a failed Client through the previous version, `CLIENT_FAILURE_HELPER` in
// `crates/local-runtime/src/automatic_updates.rs`, in its current and its earlier spelling, and how
// long the launcher waits for it. The releases that read only the earlier spelling are those
// `CrossBuildName` in `crates/release-records/src/cross_build.rs` names.
const clientFailureHelperArgument = '--record-client-failure';
const earlierClientFailureHelperArgument = '--record-client-failure-v1';
const earlierSpellingRelease = /^0\.1\.0-beta\.(?:[1-9]|1[0-9]|2[0-4])$/u;
const clientFailureWaitMilliseconds = 5_000;
// The release-build half of the local runtime's automatic-update switch, `IN_RELEASE_BUILDS` in
// `crates/local-runtime/src/automatic_updates.rs`: the change that turns one on turns on both.
const automaticUpdatesInReleaseBuilds = true;

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

/** An array of at most `limit` distinct strings that each pass `valid`. */
function distinctList(value, limit, valid) {
	return (
		Array.isArray(value) &&
		value.length <= limit &&
		value.every(valid) &&
		new Set(value).size === value.length
	);
}

function validWindowStart(value) {
	return Number.isSafeInteger(value) && value >= 0;
}

/**
 * Whether `value` is the pointer's fallback record, `extensions.fallback`, in the closed shape of
 * `crates/release-records/src/fallback.rs`; a member of any other shape counts as absent.
 */
function validFallback(value) {
	if (!closedObject(value, ['held', 'move'], []) || !distinctList(value.held, 8, validVersion))
		return false;
	const move = value.move;
	if (
		!closedObject(move, ['entry', 'back'], ['restore', 'client', 'session']) ||
		!validEntry(move.entry) ||
		typeof move.back !== 'boolean' ||
		(move.restore !== undefined && move.restore !== true)
	)
		return false;
	const client = move.client;
	if (
		client !== undefined &&
		!(
			closedObject(client, ['since', 'failures'], []) &&
			validWindowStart(client.since) &&
			Number.isInteger(client.failures) &&
			client.failures >= 0 &&
			client.failures <= 3
		)
	)
		return false;
	const session = move.session;
	const activation = (id) => typeof id === 'string' && activationIdentifier.test(id);
	return (
		session === undefined ||
		(closedObject(session, ['since', 'starts', 'failed'], []) &&
			validWindowStart(session.since) &&
			distinctList(session.starts, 8, activation) &&
			distinctList(session.failed, 3, activation) &&
			session.failed.every((id) => session.starts.includes(id)))
	);
}

/**
 * The active version a format-1 pointer names, with the versions its fallback record holds above
 * it, its previous version or null, and whether the record's move counts for the active entry and
 * may fall back; or null for any other bytes: a missing or unreadable pointer means no active
 * version, which an install repairs.
 */
function pointerRecord(text) {
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
	const active = value.active.product;
	const fallback = value.extensions?.fallback;
	const valid = validFallback(fallback);
	const held = valid ? fallback.held.filter((version) => compareVersions(version, active) > 0) : [];
	const back =
		valid &&
		fallback.move.back &&
		fallback.move.entry.product === active &&
		fallback.move.entry.manifest === value.active.manifest;
	return { active, held, previous: value.previous?.product ?? null, back };
}

/** The active product version a format-1 pointer names, or null for any other bytes. */
export function parsePointer(text) {
	return pointerRecord(text)?.active ?? null;
}

/**
 * The versions a format-1 pointer holds after a failed move, which nothing makes active until a
 * higher version is: those its fallback record lists above the active version.
 */
export function heldVersions(text) {
	return pointerRecord(text)?.held ?? [];
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

/**
 * The helper that records a failed start or join of the Client at `started`,
 * `<root>/versions/<version>/<name>` on `platform`'s paths, given the pointer `text` read again after
 * it failed: `{ program: <root>/versions/<previous>/<name>, argument, version }` when the pointer
 * still makes `<version>` active through a counting move that may fall back and names a previous
 * version, or null, where `argument` is the helper spelled as the previous version reads it. The
 * previous version runs it because the failed Client may be what broke. Only the relaunch protocol
 * uses it.
 */
export function clientFailureHelper(started, text, platform = process.platform) {
	const paths = platform === 'win32' ? path.win32 : path.posix;
	const record = text === null ? null : pointerRecord(text);
	const versions = paths.dirname(paths.dirname(started));
	const version = paths.basename(paths.dirname(started));
	if (record === null || !record.back || record.previous === null || record.active !== version)
		return null;
	return {
		program: paths.join(versions, record.previous, paths.basename(started)),
		argument: earlierSpellingRelease.test(record.previous)
			? earlierClientFailureHelperArgument
			: clientFailureHelperArgument,
		version,
	};
}

function readPointerText(root) {
	if (root === null) return null;
	const file = path.join(root, 'active.json');
	try {
		if (statSync(file).size > maximumPointer) return null;
		return readFileSync(file, 'utf8');
	} catch {
		return null;
	}
}

function readPointer(root) {
	const text = readPointerText(root);
	return text === null ? null : pointerRecord(text);
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

/**
 * The installed program to run: the active one, after installing the carried version if needed. A
 * carried version the store holds after a failed move is not installed, and the active one runs.
 */
function resolveProgram({ root, programName, carriedProgram, carriedVersion, text }) {
	const deadline = Date.now() + busyWaitMilliseconds;
	let installed = false;
	let held = false;
	let announced = false;
	for (;;) {
		const pointer = readPointer(root);
		const active = pointer?.active ?? null;
		if (
			active !== null &&
			(installed ||
				held ||
				pointer.held.includes(carriedVersion) ||
				compareVersions(carriedVersion, active) <= 0)
		)
			return path.join(root, 'versions', active, programName);
		if (installed) fail(text.installFailed(carriedVersion, 'activation'));
		if (held) fail(text.installFailed(carriedVersion, 'version-held'));
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
		// A fallback held the carried version after the pointer was read: run the active one.
		if (outcome.error === 'version-held') {
			held = true;
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
 * Whether automatic-update behaviors run for this stem, as the local runtime's switch answers for the
 * same build: always for a development or test stem, and for the release stem once the switch is on.
 */
export function automaticUpdatesLive(stem) {
	return (
		stem.startsWith('ob-dev-') || stem.startsWith('ob-test-') || automaticUpdatesInReleaseBuilds
	);
}

/**
 * How the protocol reads a Client's exit `code`, or null when no Client started: `relaunch`, `failed`
 * for a failed start or join, or `normal` for any other end, CLI failures included.
 */
export function judgeClientExit(code) {
	if (code === null) return 'failed';
	if (code === relaunchStatus) return 'relaunch';
	if (code === joinFailedStatus || code === panicStatus) return 'failed';
	// Node reports a Windows status unsigned; either sign reads as the same 32-bit value.
	const status = code >>> 0;
	return status >= errorSeverity && status !== consoleExitStatus ? 'failed' : 'normal';
}

/**
 * The step after a start (`first`, `relaunch`, or `return` to the version a failed relaunch
 * replaced) ended with `code`, given the times of the earlier relaunches: `{ step: 'end', status }`,
 * `{ step: 'unavailable' }`, `{ step: 'relaunch' }`, or `{ step: 'return' }`. A relaunched Client that
 * fails to start or to join returns to the one it replaced; a fourth relaunch inside ten seconds is
 * refused and ends as a startup failure, so no status outside the CLI exit codes reaches the caller.
 */
export function nextStart(start, code, relaunches, now) {
	const verdict = judgeClientExit(code);
	if (verdict === 'relaunch') {
		const recent = relaunches.filter((at) => now - at < relaunchWindowMilliseconds).length;
		return recent < relaunchLimit
			? { step: 'relaunch' }
			: { step: 'end', status: startupFailureStatus };
	}
	if (verdict === 'failed' && start === 'relaunch') return { step: 'return' };
	if (code === null) return { step: 'unavailable' };
	return { step: 'end', status: code === joinFailedStatus ? startupFailureStatus : code };
}

/** Leaves with a program's exit status, or by the same signal, so the caller sees what it saw. */
function leave(code, signal) {
	if (code !== null) process.exit(code);
	process.removeAllListeners(signal);
	process.kill(process.pid, signal);
	process.exit(128 + (constants.signals[signal] ?? 0));
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
	child.on('exit', leave);
}

/**
 * Runs the helper `clientFailureHelper` named with null standard streams and calls `then` once it
 * ends, cannot start, or has run for five seconds; a helper still running then is left to finish.
 */
function recordClientFailure(helper, then) {
	let done = false;
	let timer = null;
	const finish = () => {
		if (done) return;
		done = true;
		if (timer !== null) clearTimeout(timer);
		then();
	};
	let child;
	try {
		child = spawn(helper.program, [helper.argument, helper.version], {
			stdio: 'ignore',
		});
	} catch {
		finish();
		return;
	}
	timer = setTimeout(finish, clientFailureWaitMilliseconds);
	child.on('error', finish);
	child.on('exit', finish);
}

/**
 * `run` under the relaunch protocol: each start receives this process's id in both spellings of
 * the protocol variable, and `nextStart` decides after each one. A relaunch reads the pointer again; a
 * pointer that names no version counts as a relaunched Client that did not start. After a failed
 * start or join, the failure is recorded through the previous version first when the pointer still
 * makes the failed version active through a move that may fall back.
 */
function runWithRelaunch(program, root, programName, text) {
	const environment = { ...process.env };
	for (const name of relaunchVariables) environment[name] = String(process.pid);
	const relaunches = [];
	let replaced = null;
	function decide(current, start, code, error) {
		const helper =
			current !== null && judgeClientExit(code) === 'failed'
				? clientFailureHelper(current, readPointerText(root))
				: null;
		if (helper === null) proceed(current, start, code, error);
		else recordClientFailure(helper, () => proceed(current, start, code, error));
	}
	function proceed(current, start, code, error) {
		const now = performance.now();
		const next = nextStart(start, code, relaunches, now);
		if (next.step === 'end') process.exit(next.status);
		if (next.step === 'unavailable')
			fail(text.startFailed(current ?? '-', error?.code ?? error?.message ?? 'ENOENT'));
		if (next.step === 'return') {
			startClient(replaced, 'return');
			return;
		}
		if (relaunches.length === relaunchLimit) relaunches.shift();
		relaunches.push(now);
		replaced = current;
		const active = readPointer(root)?.active ?? null;
		startClient(
			active === null ? null : path.join(root, 'versions', active, programName),
			'relaunch',
		);
	}
	function startClient(current, start) {
		if (current === null) {
			decide(current, start, null, null);
			return;
		}
		const child = spawn(current, process.argv.slice(2), { stdio: 'inherit', env: environment });
		let settled = false;
		child.on('error', (error) => {
			if (settled) return;
			settled = true;
			decide(current, start, null, error);
		});
		child.on('exit', (code, signal) => {
			if (settled) return;
			settled = true;
			if (code === null) leave(code, signal);
			else decide(current, start, code, null);
		});
	}
	startClient(program, 'first');
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
	if (automaticUpdatesLive(stem))
		runWithRelaunch(program, storeRoot(stem, process.platform, process.env), programName, text);
	else run(program, text);
}
