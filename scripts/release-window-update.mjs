// The real-window Update check of the release path, `window-update` in
// scripts/release-smoke-windows.mjs and scripts/release-smoke-macos.mjs. It presses Update in the
// real window of a packaged Test build as a person whose window version is older does when a newer
// one is installed, in the steps of apps/desktop/README.md (Window Update, Manual self-test):
//
// 1. Settings › About › Update relaunches the window into the newer version.
// 2. With the newer version's bundled Client moved aside, the sidebar footer's Update shows the
//    waiting notice, and the window keeps running.
// 3. With the file the relaunch starts moved aside (Windows: the window entry
//    `desktop\<stem>-desktop.exe`; macOS: the newer version's `<stem>-desktop-launcher`), it shows
//    the failure notice, and the window keeps running.
// 4. The footer's Update relaunches the window into the newer version.
//
// A relaunch counts when the older window process exits, the data root's diagnostic log gains a
// `window-relaunch-started` record, one new window process runs (on Windows from
// `desktop\versions\<newer>\`; on macOS from the bundle the person opened, which the launcher
// replaced), it shows the Workspace the older window showed, and its Settings › About names the
// newer version. A terminal the CLI opened prints `tick-<n>` every second throughout and must print
// a new line after each relaunch. Each notice must read as a workbench catalog words it, in en-US or
// zh-CN.
//
// The page is driven through the Chromium DevTools protocol over Node's own WebSocket. A
// development or Test window opens that port on the loopback interface when
// `ORCHBEE_DESKTOP_DEBUG_PORT` names one (apps/desktop/README.md); a release build never does, so
// the check refuses the release stem. The check passes 0, so each window takes a free port and
// Chromium writes it to `DevToolsActivePort` in the window's Electron folder,
// `<data root>/internal/desktop/`, which the check removes before each start. A press is a
// DevTools mouse event at the element's centre once nothing covers that point: no input reaches the
// operating system.
//
// Nothing is downloaded. The packaged Test window is the newer version. Two copies of it whose
// `app.asar` package version reads 0.0.0-window-update.1 and 0.0.0-window-update.2, each with its
// `desktop-files.json` written again (on macOS after an ad hoc signature, as packaging signs),
// stand in for older window versions, and each is installed by its own `--installer-window-v1`
// into a store root and data root inside the work folder. What the stand-ins cannot prove: that an
// older published window, whose own code handles the press, hands over to this one, and that a
// Session refuses an incompatible window, which the moved-aside Client only imitates; the release
// gate proves the handover of published releases.
//
// On a failure it writes a screenshot, the page's DOM and the data root's logs into
// `--diagnostics`, and prints the diagnostic log's tail and the processes still running from the
// work folder. Either way it then stops the window launchers, the windows, the Session and every
// other process of the work folder.
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	cpSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	renameSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { DESKTOP_FILES, desktopFilesDocument } from './desktop-files.mjs';
import { readRelease } from './package-npm.mjs';
import {
	diagnosticLogTail,
	runExactDocument,
	StepFailure,
	UsageError,
} from './release-update-check.mjs';
import { compareVersions } from './release-version.mjs';

/** The variable a development or Test window reads its DevTools port from; 0 takes a free one. */
const debugPortVariable = 'ORCHBEE_DESKTOP_DEBUG_PORT';
/** The older window versions the copies stand in for, below every release. */
const standInVersions = ['0.0.0-window-update.1', '0.0.0-window-update.2'];
/** The Session a Test window joins (apps/desktop/README.md, Window Update). */
const testSession = 'desktop-test';
const releaseStem = 'orchbee';
const commandDeadline = 120_000;
const installDeadline = 600_000;
// A window's first start also starts its Session.
const windowDeadline = 120_000;
const relaunchDeadline = 60_000;
const pageDeadline = 30_000;
const terminalDeadline = 60_000;
const stopDeadline = 60_000;
const callDeadline = 15_000;
// A window tends the store 15 seconds after it starts, which could put a moved-aside file back.
const tendSettled = 25_000;
const poll = 250;
// Each selector names an attribute the web workbench keeps for exactly that element.
const selectors = {
	about: '[data-settings-category="about"]',
	aboutUpdate: '[data-about-update]',
	aboutVersion: '[data-about-version]',
	footerUpdate: '[data-slot="workbench-sidebar-update"]',
	notice: '[data-slot="workbench-status-notice"] span',
	settings: '[data-slot="workbench-sidebar-settings"]',
	settingsClose: '[data-settings-close]',
	settingsSurface: '[data-slot="terminal-settings-surface"]',
	shownWorkspace: '[data-sidebar-nav="workspace"][aria-current="page"]',
};
const catalogs = ['en-US', 'zh-CN'].map((locale) =>
	fileURLToPath(new URL(`../packages/i18n/src/locales/${locale}/workbench.ts`, import.meta.url)),
);

function log(line) {
	process.stdout.write(`${line}\n`);
}

function pause(milliseconds) {
	return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

/** `window-update <packaged Test window> --work <absolute new directory> [--diagnostics <absolute directory>]`. */
export function parseWindowUpdateArguments(argv, usage) {
	const [window, ...rest] = argv;
	const options = {};
	for (let index = 0; index < rest.length; index += 2) {
		const [option, value] = [rest[index], rest[index + 1]];
		if (!['--work', '--diagnostics'].includes(option) || value === undefined)
			throw new UsageError(usage);
		const name = option.slice(2);
		if (name in options || !path.isAbsolute(value)) throw new UsageError(usage);
		options[name] = path.resolve(value);
	}
	if (window === undefined || options.work === undefined) throw new UsageError(usage);
	return { step: 'window-update', window: path.resolve(window), ...options };
}

/** What a failed check tells the owner. */
export function windowUpdateFailureAdvice(stage) {
	return `The real-window Update check failed while ${stage}. Do not publish this release.\n`;
}

/** Runs a program with captured output, which it prints unless `quiet`. */
function run(program, argv, env, { deadline = commandDeadline, quiet = false } = {}) {
	return runExactDocument(program, argv, env, { deadline, quiet });
}

function expectPassed(result, what) {
	if (result.status !== 0) throw new StepFailure(`${what} exited ${result.status}, expected 0`);
}

/** Calls `read` until it answers something other than undefined, or fails naming `what`. */
async function until(read, what, deadline) {
	const end = Date.now() + deadline;
	for (;;) {
		const value = await read();
		if (value !== undefined) return value;
		if (Date.now() >= end) throw new StepFailure(`${what} did not happen within ${deadline} ms`);
		await pause(poll);
	}
}

/** The message `key` of one workbench catalog's source, whose line names it in single quotes. */
function catalogMessage(source, key) {
	const escaped = key.replaceAll('.', '\\.');
	const match = new RegExp(`'${escaped}':\\s*(['"])((?:\\\\.|(?!\\1)[^\\\\])*)\\1`, 'u').exec(
		source,
	);
	if (match === null) throw new StepFailure(`A workbench catalog holds no ${key}`);
	return match[2].replaceAll(/\\(.)/gu, '$1');
}

/** Every locale's wording of the workbench message `key`. */
function noticeTexts(key) {
	return catalogs.map((file) => catalogMessage(readFileSync(file, 'utf8'), key));
}

/**
 * Rewrites the package version in `app.asar`'s `package.json`, the version Electron answers as
 * `app.getVersion()`, and keeps every other file: the files after it move by the size change, and
 * its integrity in the header is computed again. The packaged application turns on no Electron fuse
 * that validates the archive.
 */
export function relabelAsar(file, version) {
	const bytes = readFileSync(file);
	// A Chromium pickle holding the header pickle's size, then the header pickle: its payload size,
	// the JSON's length and the JSON, padded to four bytes.
	const headerSize = bytes.readUInt32LE(4);
	const jsonSize = bytes.readUInt32LE(12);
	const header = JSON.parse(bytes.toString('utf8', 16, 16 + jsonSize));
	const base = 8 + headerSize;
	const entry = header.files?.['package.json'];
	if (entry?.offset === undefined || entry.unpacked === true)
		throw new StepFailure(`${file} holds no packed package.json`);
	const offset = Number(entry.offset);
	const start = base + offset;
	const manifest = JSON.parse(bytes.toString('utf8', start, start + entry.size));
	manifest.version = version;
	const replacement = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
	const delta = replacement.length - entry.size;
	const shift = (folder) => {
		for (const node of Object.values(folder.files ?? {})) {
			if (node.files !== undefined) shift(node);
			else if (node !== entry && node.offset !== undefined && Number(node.offset) > offset)
				node.offset = String(Number(node.offset) + delta);
		}
	};
	shift(header);
	const replacedSize = entry.size;
	entry.size = replacement.length;
	if (entry.integrity !== undefined) {
		const hash = createHash('sha256').update(replacement).digest('hex');
		entry.integrity = { ...entry.integrity, hash, blocks: [hash] };
	}
	const json = Buffer.from(JSON.stringify(header), 'utf8');
	const padded = Math.ceil(json.length / 4) * 4;
	const headerPickle = Buffer.alloc(8 + padded);
	headerPickle.writeUInt32LE(4 + padded, 0);
	headerPickle.writeUInt32LE(json.length, 4);
	json.copy(headerPickle, 8);
	const sizePickle = Buffer.alloc(8);
	sizePickle.writeUInt32LE(4, 0);
	sizePickle.writeUInt32LE(headerPickle.length, 4);
	writeFileSync(
		file,
		Buffer.concat([
			sizePickle,
			headerPickle,
			bytes.subarray(base, start),
			replacement,
			bytes.subarray(start + replacedSize),
		]),
	);
}

/** One page of the window, over the DevTools protocol on the loopback port. */
class WindowPage {
	#next = 1;
	#pending = new Map();
	#socket;

	constructor(socket) {
		this.#socket = socket;
		socket.addEventListener('message', (event) => {
			const message = JSON.parse(String(event.data));
			const waiter = this.#pending.get(message.id);
			if (waiter === undefined) return;
			this.#pending.delete(message.id);
			if (message.error === undefined) waiter.resolve(message.result);
			else waiter.reject(new Error(message.error.message));
		});
		socket.addEventListener('close', () => {
			for (const waiter of this.#pending.values()) waiter.reject(new Error('The page closed'));
			this.#pending.clear();
		});
	}

	/** Connects to the page of the window whose port `portFile` names, once it lists one. */
	static open(portFile, deadline) {
		return until(
			async () => {
				try {
					const port = Number(readFileSync(portFile, 'utf8').split('\n')[0]);
					if (!Number.isInteger(port) || port <= 0) return undefined;
					const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
						signal: AbortSignal.timeout(2_000),
					});
					const target = (await response.json()).find(
						(each) =>
							each.type === 'page' &&
							typeof each.webSocketDebuggerUrl === 'string' &&
							!String(each.url).startsWith('devtools:'),
					);
					if (target === undefined) return undefined;
					const socket = new WebSocket(target.webSocketDebuggerUrl);
					await new Promise((resolve, reject) => {
						socket.addEventListener('open', resolve, { once: true });
						socket.addEventListener('error', () => reject(new Error('refused')), { once: true });
					});
					return new WindowPage(socket);
				} catch {
					return undefined;
				}
			},
			`A window page on the port ${portFile} names`,
			deadline,
		);
	}

	send(method, params = {}) {
		const id = this.#next++;
		return new Promise((resolve, reject) => {
			const timer = setTimeout(() => {
				this.#pending.delete(id);
				reject(new Error(`${method} got no answer within ${callDeadline} ms`));
			}, callDeadline);
			const settle = (done) => (value) => {
				clearTimeout(timer);
				done(value);
			};
			this.#pending.set(id, { resolve: settle(resolve), reject: settle(reject) });
			this.#socket.send(JSON.stringify({ id, method, params }));
		});
	}

	/** The value of `expression` in the page, or undefined when it throws or the page is gone. */
	async evaluate(expression) {
		try {
			const answer = await this.send('Runtime.evaluate', {
				awaitPromise: true,
				expression,
				returnByValue: true,
			});
			return answer.exceptionDetails === undefined ? answer.result.value : undefined;
		} catch {
			return undefined;
		}
	}

	/** Whether `selector` names an element on the page now. */
	async has(selector) {
		return (
			(await this.evaluate(`document.querySelector(${JSON.stringify(selector)}) !== null`)) === true
		);
	}

	/** Waits until `selector` names an element on the page. */
	present(selector, what, deadline = pageDeadline) {
		return until(async () => ((await this.has(selector)) ? true : undefined), what, deadline);
	}

	/** The trimmed text of the element `selector` names, once `accept` admits it. */
	text(selector, what, accept = (value) => value !== '', deadline = pageDeadline) {
		return until(
			async () => {
				const value = await this.evaluate(
					`document.querySelector(${JSON.stringify(selector)})?.textContent?.trim() ?? ''`,
				);
				return typeof value === 'string' && accept(value) ? value : undefined;
			},
			what,
			deadline,
		);
	}

	/**
	 * Presses the element `selector` names with the left mouse button at its centre, once it shows
	 * and nothing covers that point. With `mayClose`, a page that goes away during the press, as a
	 * window that relaunches does, is no failure.
	 */
	async click(selector, what, { mayClose = false } = {}) {
		const point = await until(
			async () =>
				(await this.evaluate(`(() => {
					const element = document.querySelector(${JSON.stringify(selector)});
					if (element === null) return null;
					element.scrollIntoView({ block: 'nearest', inline: 'nearest' });
					const box = element.getBoundingClientRect();
					if (box.width === 0 || box.height === 0) return null;
					const x = box.left + box.width / 2;
					const y = box.top + box.height / 2;
					return element.contains(document.elementFromPoint(x, y)) ? { x, y } : null;
				})()`)) ?? undefined,
			`${what} to show`,
			pageDeadline,
		);
		try {
			await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point });
			for (const type of ['mousePressed', 'mouseReleased'])
				await this.send('Input.dispatchMouseEvent', {
					button: 'left',
					buttons: type === 'mousePressed' ? 1 : 0,
					clickCount: 1,
					type,
					...point,
				});
		} catch (error) {
			if (!mayClose) throw new StepFailure(`Pressing ${what} failed: ${error.message}`);
			log(`The page went away during the press of ${what}: ${error.message}`);
		}
		log(`Pressed ${what}.`);
	}

	/** Writes a screenshot and the DOM into `folder` as `<label>.png` and `<label>.html`. */
	async keep(folder, label) {
		try {
			const shot = await this.send('Page.captureScreenshot', { format: 'png' });
			writeFileSync(path.join(folder, `${label}.png`), Buffer.from(shot.data, 'base64'));
		} catch (error) {
			log(`No screenshot: ${error.message}`);
		}
		const html = await this.evaluate('document.documentElement.outerHTML');
		if (typeof html === 'string') writeFileSync(path.join(folder, `${label}.html`), html);
	}

	close() {
		this.#socket.close();
	}
}

/** How many `event` records the data root's daily diagnostic logs hold. */
function eventCount(dataRoot, event) {
	const folder = path.join(dataRoot, 'logs');
	if (!existsSync(folder)) return 0;
	let count = 0;
	for (const name of readdirSync(folder)) {
		if (!/^\d{4}-\d{2}-\d{2}\.jsonl$/u.test(name)) continue;
		for (const line of readFileSync(path.join(folder, name), 'utf8').split('\n'))
			if (line.includes(`"event":"${event}"`)) count += 1;
	}
	return count;
}

function running(pid) {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

function kill(pid) {
	try {
		process.kill(pid, 'SIGKILL');
	} catch {
		// It has already ended.
	}
}

function samePath(left, right) {
	return process.platform === 'win32'
		? path.resolve(left).toLowerCase() === path.resolve(right).toLowerCase()
		: path.resolve(left) === path.resolve(right);
}

/** Starts `program` detached, so it outlives this process, with nothing on its standard streams. */
export function startDetached(program, argv, { cwd, env }) {
	spawn(program, argv, { cwd, detached: true, env, stdio: 'ignore' }).unref();
	log(`Started ${program}.`);
}

/** The packaged Test window `directory`: its stem, version and, on macOS, its bundle. */
function readWindow(directory, port) {
	const listFile = path.join(directory, DESKTOP_FILES);
	if (!existsSync(listFile))
		throw new StepFailure(`${directory} holds no ${DESKTOP_FILES}; name a packaged Test window`);
	const { version } = JSON.parse(readFileSync(listFile, 'utf8'));
	const application = port.application(directory);
	const { stem } = readRelease(port.cohort(directory, application), version);
	return { application, directory, stem, version };
}

/** A copy of the window `built` in `work` that answers `version`, its list written again. */
function standIn(built, version, work, port) {
	const directory = path.join(work, 'windows', version);
	cpSync(built.directory, directory, { recursive: true, verbatimSymlinks: true });
	relabelAsar(port.asar(directory, built.application), version);
	port.seal(directory, built.application);
	writeFileSync(
		path.join(directory, DESKTOP_FILES),
		desktopFilesDocument(directory, version, built.application),
	);
	log(`The stand-in window ${version} is a copy of ${built.version}.`);
	return { ...built, directory, version };
}

/** Installs the window directory `window` into the store with its own `--installer-window-v1`. */
function installWindow(window, context) {
	const { env, port, storeRoot } = context;
	const client = port.client(port.cohort(window.directory, window.application), window.stem);
	expectPassed(
		run(client, ['--installer-window-v1'], env, { deadline: installDeadline }),
		`--installer-window-v1 of ${window.version}`,
	);
	const pointer = path.join(storeRoot, 'desktop', 'active.json');
	const active = existsSync(pointer)
		? JSON.parse(readFileSync(pointer, 'utf8'))?.active?.product
		: undefined;
	if (active !== window.version)
		throw new StepFailure(`${pointer} names ${active}, not the window version ${window.version}`);
	log(`The window store's active version is ${window.version}.`);
}

/** The one window process running from the work folder other than `previous`, once there is one. */
function windowProcess(context, previous, deadline) {
	return until(
		() => {
			const found = context.port
				.processes(context.work)
				.filter((each) => each.window && each.pid !== previous);
			return found.length === 1 ? found[0] : undefined;
		},
		'One new window process',
		deadline,
	);
}

/** Opens the page of the window that started last and waits for its workbench. */
async function openPage(context) {
	context.page = await WindowPage.open(context.portFile, windowDeadline);
	await context.page.present(selectors.settings, 'The workbench', windowDeadline);
}

/** Opens Settings › About when Settings is not open, reads the version row and closes Settings. */
async function aboutVersion(page) {
	if (!(await page.has(selectors.settingsSurface)))
		await page.click(selectors.settings, 'Settings in the sidebar');
	await page.click(selectors.about, 'About in Settings');
	const version = await page.text(selectors.aboutVersion, 'The About version row');
	await page.click(selectors.settingsClose, 'the Settings close button');
	await until(
		async () => ((await page.has(selectors.settingsSurface)) ? undefined : true),
		'Settings to close',
		pageDeadline,
	);
	return version;
}

/** The id of the Workspace the window shows, once it shows one. */
function shownWorkspace(page) {
	return until(
		async () =>
			(await page.evaluate(
				`document.querySelector(${JSON.stringify(selectors.shownWorkspace)})?.closest('[data-workspace-id]')?.getAttribute('data-workspace-id') ?? null`,
			)) ?? undefined,
		'The window to show a Workspace',
		pageDeadline,
	);
}

/**
 * Requires the terminal to print a tick numbered above every tick it shows now. `wait-output
 * --after now` would see nothing: it matches only output rows, and the rows of a command typed at a
 * prompt count as that prompt's.
 */
function expectTicking(context, terminal, what) {
	const { cli, cliEnv } = context;
	const shown = run(cli, ['terminal', 'read', terminal, '--lines', '40'], cliEnv, { quiet: true });
	expectPassed(shown, `terminal read ${what}`);
	const ticks = [...shown.stdout.matchAll(/tick-(\d+)/gu)].map((match) => Number(match[1]));
	if (ticks.length === 0) throw new StepFailure(`Terminal ${terminal} shows no tick ${what}`);
	// Two above the highest, so a line printed while the terminal was read cannot count.
	const next = Math.max(...ticks) + 2;
	expectPassed(
		run(
			cli,
			[
				'terminal',
				'wait-output',
				terminal,
				'--regex',
				`tick-${next}\\b`,
				'--timeout',
				String(terminalDeadline),
			],
			cliEnv,
		),
		`terminal wait-output for tick-${next} ${what}`,
	);
	log(`Terminal ${terminal} printed tick-${next} ${what}: it still prints a tick every second.`);
}

/**
 * Presses with `press` and requires a relaunch into `version`: the window process ends, the
 * diagnostic log records `window-relaunch-started`, one new window process runs as
 * `port.relaunchedProgram` says and opens a page that shows `workspace`, and its About names
 * `version`. Leaves the new page and process in `context`.
 */
async function expectRelaunch(context, { press, version, workspace }) {
	const older = context.process;
	const started = eventCount(context.dataRoot, 'window-relaunch-started');
	rmSync(context.portFile, { force: true });
	const pressedAt = Date.now();
	await press();
	await until(
		() => (running(older.pid) ? undefined : true),
		`The window process ${older.pid} to exit`,
		relaunchDeadline,
	);
	context.page.close();
	context.page = undefined;
	if (eventCount(context.dataRoot, 'window-relaunch-started') <= started)
		throw new StepFailure('The diagnostic log holds no new window-relaunch-started record');
	context.process = await windowProcess(context, older.pid, relaunchDeadline);
	context.windowStartedAt = Date.now();
	const expected = context.port.relaunchedProgram({ ...context, version });
	if (!samePath(context.process.program, expected))
		throw new StepFailure(`The relaunched window runs ${context.process.program}, not ${expected}`);
	await openPage(context);
	log(
		`Window process ${older.pid} gave way to ${context.process.pid}, ${context.process.program}, ${Date.now() - pressedAt} ms after the press.`,
	);
	const shown = await shownWorkspace(context.page);
	if (shown !== workspace)
		throw new StepFailure(`The relaunched window shows Workspace ${shown}, not ${workspace}`);
	log(`The relaunched window shows Workspace ${workspace}, as the older one did.`);
	const shownVersion = await aboutVersion(context.page);
	if (shownVersion !== version)
		throw new StepFailure(`The relaunched window's About names ${shownVersion}, not ${version}`);
	log(`The relaunched window runs ${version}: Settings › About names it.`);
}

/** Presses the footer's Update with `file` moved aside and requires the notice `key`. */
async function expectNotice(context, { file, key, what }) {
	const expected = noticeTexts(key);
	await pause(Math.max(0, context.windowStartedAt + tendSettled - Date.now()));
	const aside = `${file}.aside`;
	renameSync(file, aside);
	try {
		await context.page.click(selectors.footerUpdate, 'Update in the sidebar footer');
		const shown = await context.page.text(selectors.notice, `The ${what} notice`, (value) =>
			expected.includes(value),
		);
		log(`The window shows the ${what} notice: ${JSON.stringify(shown)}.`);
	} finally {
		renameSync(aside, file);
	}
	await pause(1_000);
	if (!running(context.process.pid))
		throw new StepFailure(`The window process ${context.process.pid} ended after the ${what}`);
	log(`The window keeps running after the ${what}.`);
}

/** Opens a terminal in a new Workspace through the CLI and starts the ticking line in it. */
async function openTerminal(context) {
	const { cli, cliEnv, port, work } = context;
	const shell = port.shell(work);
	const created = run(cli, ['workspace', 'create', ...shell.argv, '--format', 'json'], cliEnv);
	const workspace = created.document?.result?.workspace?.result;
	if (created.status !== 0 || typeof workspace?.panelId !== 'string')
		throw new StepFailure(`workspace create exited ${created.status}`);
	const terminal = await until(
		() =>
			(
				run(cli, ['terminal', 'list', '--format', 'json'], cliEnv, { quiet: true }).document?.result
					?.terminals ?? []
			).find((each) => each.boundTo?.panelId === workspace.panelId)?.terminalId,
		`A terminal in Panel ${workspace.panelId}`,
		terminalDeadline,
	);
	const timeout = String(terminalDeadline);
	expectPassed(
		run(cli, ['terminal', 'wait-output', terminal, '--regex', '\\S', '--timeout', timeout], cliEnv),
		'terminal wait-output for the shell',
	);
	expectPassed(
		run(cli, ['terminal', 'send', terminal, '--text', shell.ticking, '--enter'], cliEnv),
		'terminal send',
	);
	expectPassed(
		run(
			cli,
			['terminal', 'wait-output', terminal, '--regex', 'tick-[0-9]+', '--timeout', timeout],
			cliEnv,
		),
		'terminal wait-output for the first tick',
	);
	log(`Terminal ${terminal} in Workspace ${workspace.workspaceId} prints a tick every second.`);
	return { terminal, workspace: workspace.workspaceId };
}

/**
 * Stops the window launchers, the windows, the Session and then whatever else runs from the work
 * folder. A launcher goes first: one that watches a window starts the version it replaced when
 * that window dies.
 */
async function stopEverything(context) {
	context.page?.close();
	context.page = undefined;
	const sweep = (filter) => {
		for (const each of context.port.processes(context.work).filter(filter)) kill(each.pid);
	};
	sweep((each) => each.launcher);
	await pause(500);
	sweep((each) => each.window);
	await pause(1_000);
	if (context.cli !== undefined && existsSync(context.cli))
		run(context.cli, ['server', 'stop', '--confirm'], context.cliEnv, { quiet: true });
	const end = Date.now() + stopDeadline;
	for (;;) {
		const left = context.port.processes(context.work);
		if (left.length === 0) return;
		if (Date.now() >= end)
			throw new StepFailure(
				`Still running from ${context.work}:\n${left.map((each) => `  ${each.pid} ${each.program}`).join('\n')}`,
			);
		sweep((each) => each.launcher);
		sweep(() => true);
		await pause(1_000);
	}
}

/** Keeps what explains a failure: the page, the data root's logs and the work folder's processes. */
async function keepDiagnostics(context, diagnostics, stage) {
	if (diagnostics !== undefined) {
		mkdirSync(diagnostics, { recursive: true });
		await context.page?.keep(diagnostics, stage.replaceAll(/[^a-z0-9]+/giu, '-'));
		const logs = path.join(context.dataRoot, 'logs');
		if (existsSync(logs)) cpSync(logs, path.join(diagnostics, 'logs'), { recursive: true });
		log(`The page's screenshot and DOM, when a page was open, and the logs are in ${diagnostics}.`);
	}
	log(`Diagnostic log:\n${diagnosticLogTail(context.dataRoot)}`);
	const left = context.port.processes(context.work);
	log(
		`Running from ${context.work}:\n${left.length === 0 ? '  none' : left.map((each) => `  ${each.pid} ${each.program}`).join('\n')}`,
	);
}

/**
 * Runs the check on the packaged Test window `options.window` in the empty folder `work`. `port`
 * says where this platform's window keeps its files and how its processes look:
 *
 * - `application(window)`: the `<name>.app` a macOS window directory holds; undefined on Windows.
 * - `cohort(window, application)`: the folder of a window's bundled Client.
 * - `client(folder, stem)`: the Client in a cohort or version folder.
 * - `asar(window, application)`: the window's `app.asar`.
 * - `seal(window, application)`: makes a changed copy start again.
 * - `environment(work, stem)`: the environment of every program the check runs, with no inherited
 *   `ORCHBEE_*` setting and with `ORCHBEE_INSTALL_DIR` and `ORCHBEE_HOME` naming a store root and
 *   a data root in `work`, whose temporary folders lie there too.
 * - `storedCohort(storeRoot, version, application)`: the bundled cohort of a stored window version.
 * - `relaunchStarter(storeRoot, stem, version, application)`: the file a relaunch into `version`
 *   starts.
 * - `startWindow(context)`: starts the store's active window version as a person does, where
 *   `context.window` is the window directory installed last; answers the entry it started.
 * - `relaunchedProgram(context)`: the program of a window relaunched into `context.version`.
 * - `shell(work)`: `{ argv, ticking }`, the `workspace create` options of an interactive shell in
 *   `work`, and the line that prints `tick-<n>` every second in it.
 * - `processes(work)`: every process whose program lies in `work`, as `{ pid, program, window,
 *   launcher }`; `window` marks a window's main process and `launcher` a window launcher.
 */
export async function windowUpdateCheck(options, work, progress, port) {
	progress.stage = 'reading the Test window';
	const built = readWindow(options.window, port);
	if (built.stem === releaseStem)
		throw new StepFailure(
			'A release build never opens a DevTools port; window-update drives a Test build',
		);
	for (const version of standInVersions)
		if (compareVersions(version, built.version) >= 0)
			throw new StepFailure(`The stand-in version ${version} is not older than ${built.version}`);
	const env = { ...port.environment(work, built.stem), [debugPortVariable]: '0' };
	const dataRoot = env.ORCHBEE_HOME;
	const context = {
		built,
		cli: undefined,
		cliEnv: { ...env, ORCHBEE_SESSION: testSession },
		dataRoot,
		entry: undefined,
		env,
		page: undefined,
		port,
		portFile: path.join(dataRoot, 'internal', 'desktop', 'DevToolsActivePort'),
		process: undefined,
		stem: built.stem,
		storeRoot: env.ORCHBEE_INSTALL_DIR,
		windowStartedAt: undefined,
		work,
	};
	const startedAt = Date.now();
	try {
		progress.stage = 'staging the stand-in windows';
		const [oldest, older] = standInVersions.map((version) => standIn(built, version, work, port));

		progress.stage = 'installing the runtime and the oldest window';
		const bundled = port.client(port.cohort(built.directory, built.application), built.stem);
		expectPassed(
			run(bundled, ['install', '--no-path'], env, { deadline: installDeadline }),
			`install --no-path of ${built.version}`,
		);
		context.cli = port.client(path.join(context.storeRoot, 'versions', built.version), built.stem);
		installWindow(oldest, context);

		progress.stage = `starting the window ${oldest.version}`;
		rmSync(context.portFile, { force: true });
		context.entry = port.startWindow({ ...context, window: oldest });
		context.process = await windowProcess(context, undefined, windowDeadline);
		context.windowStartedAt = Date.now();
		await openPage(context);
		log(`The window ${oldest.version} runs as process ${context.process.pid}.`);

		progress.stage = 'opening the terminal';
		const { terminal, workspace } = await openTerminal(context);

		progress.stage = `Settings › About › Update from ${oldest.version} to ${older.version}`;
		installWindow(older, context);
		await context.page.present(selectors.footerUpdate, 'Update in the sidebar footer');
		const firstPlace = await shownWorkspace(context.page);
		await context.page.click(selectors.settings, 'Settings in the sidebar');
		await context.page.click(selectors.about, 'About in Settings');
		await expectRelaunch(context, {
			press: () =>
				context.page.click(selectors.aboutUpdate, 'Update in Settings › About', {
					mayClose: true,
				}),
			version: older.version,
			workspace: firstPlace,
		});
		expectTicking(context, terminal, 'after the first relaunch');

		progress.stage = `the waiting notice from ${older.version}`;
		installWindow(built, context);
		await context.page.present(selectors.footerUpdate, 'Update in the sidebar footer');
		// The row, not its button: the row's name lies above the button, and a press anywhere on the
		// row opens its Workspace.
		await context.page.click(
			`[data-workspace-id=${JSON.stringify(workspace)}] > [data-navigation-reorder-row]`,
			"the terminal's Workspace in the sidebar",
		);
		const cohort = port.storedCohort(context.storeRoot, built.version, built.application);
		await expectNotice(context, {
			file: port.client(cohort, built.stem),
			key: 'workbench.update.waiting',
			what: 'waiting',
		});

		progress.stage = `the failure notice from ${older.version}`;
		await expectNotice(context, {
			file: port.relaunchStarter(context.storeRoot, built.stem, built.version, built.application),
			key: 'workbench.update.unavailable',
			what: 'failure',
		});

		progress.stage = `the footer's Update from ${older.version} to ${built.version}`;
		await expectRelaunch(context, {
			press: () =>
				context.page.click(selectors.footerUpdate, 'Update in the sidebar footer', {
					mayClose: true,
				}),
			version: built.version,
			workspace,
		});
		expectTicking(context, terminal, 'after the second relaunch');

		progress.stage = 'stopping the window and the Session';
		await stopEverything(context);
		log(`The real-window Update check passed in ${Math.round((Date.now() - startedAt) / 1000)} s.`);
	} catch (error) {
		await keepDiagnostics(context, options.diagnostics, progress.stage).catch((kept) =>
			log(`Keeping the diagnostics failed: ${kept.message}`),
		);
		await stopEverything(context).catch((stopped) => log(stopped.message));
		throw error;
	}
}
