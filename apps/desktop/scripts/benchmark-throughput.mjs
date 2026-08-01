import { spawn, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { access, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { setTimeout as wait } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

import { _electron as electron, expect } from '@playwright/test';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, '..', '..', '..');
const hostBinary = path.join(
	repositoryRoot,
	'apps',
	'terminal-host',
	'zig-out',
	'bin',
	process.platform === 'win32' ? 'terminal-host.exe' : 'terminal-host',
);
const desktopMain = path.join(repositoryRoot, 'apps', 'desktop', 'dist', 'main', 'main.cjs');
const comparativeFixtureMinimumBytes = 500_000_000;
const activeTargetColumns = 113;
const activeTargetRows = 38;
const hostRssSamplingIntervalMilliseconds = 20;

function parseArguments(arguments_) {
	const options = {
		file: undefined,
		ghosttyApplication: '/Applications/Ghostty.app',
		ghosttyRuns: [],
		hostOnlyRuns: [],
		measureGhostty: false,
		runs: 5,
		slice5Median: undefined,
	};
	for (let index = 0; index < arguments_.length; index += 1) {
		const argument = arguments_[index];
		const value = arguments_[index + 1];
		switch (argument) {
			case '--':
				break;
			case '--file':
				options.file = value;
				index += 1;
				break;
			case '--ghostty-runs':
				options.ghosttyRuns = parseRuns(value, '--ghostty-runs');
				index += 1;
				break;
			case '--ghostty-app':
				options.ghosttyApplication = value;
				index += 1;
				break;
			case '--host-only-runs':
				options.hostOnlyRuns = parseRuns(value, '--host-only-runs');
				index += 1;
				break;
			case '--measure-ghostty':
				options.measureGhostty = true;
				break;
			case '--runs':
				options.runs = Number(value);
				index += 1;
				break;
			case '--slice5-median':
				options.slice5Median = Number(value);
				index += 1;
				break;
			default:
				throw new Error(`Unknown benchmark argument: ${argument ?? '<missing>'}`);
		}
	}
	if (options.file === undefined) throw new Error('Benchmark requires --file <absolute-path>.');
	if (!path.isAbsolute(options.file)) throw new Error('Benchmark file path must be absolute.');
	if (!path.isAbsolute(options.ghosttyApplication)) {
		throw new Error('Ghostty application path must be absolute.');
	}
	if (options.measureGhostty && options.ghosttyRuns.length > 0) {
		throw new Error('Use either --measure-ghostty or --ghostty-runs, not both.');
	}
	if (!Number.isInteger(options.runs) || options.runs < 5) {
		throw new Error('Benchmark requires at least five warm runs.');
	}
	if (
		options.slice5Median !== undefined &&
		(!Number.isFinite(options.slice5Median) || options.slice5Median <= 0)
	) {
		throw new Error('--slice5-median requires a positive duration in seconds.');
	}
	return options;
}

function parseRuns(value, name) {
	if (value === undefined) throw new Error(`${name} requires comma-separated seconds.`);
	const runs = value.split(',').map(Number);
	if (runs.some((run) => !Number.isFinite(run) || run <= 0)) {
		throw new Error(`${name} contains an invalid duration.`);
	}
	return runs;
}

function summary(values) {
	if (values.length === 0) return null;
	const sorted = [...values].sort((left, right) => left - right);
	const middle = Math.floor(sorted.length / 2);
	const median =
		sorted.length % 2 === 0 ? (sorted[middle - 1] + sorted[middle]) / 2 : sorted[middle];
	const deviations = sorted.map((value) => Math.abs(value - median)).sort((a, b) => a - b);
	const deviationMiddle = Math.floor(deviations.length / 2);
	const medianAbsoluteDeviation =
		deviations.length % 2 === 0
			? (deviations[deviationMiddle - 1] + deviations[deviationMiddle]) / 2
			: deviations[deviationMiddle];
	return {
		maximum: sorted.at(-1),
		median,
		medianAbsoluteDeviation,
		minimum: sorted[0],
		ninetyFifthPercentile: sorted[Math.max(0, Math.ceil(sorted.length * 0.95) - 1)],
		runs: values,
	};
}

function shellQuote(value) {
	return `'${value.replaceAll("'", `'\\''`)}'`;
}

async function waitForEndpoint(runtimeDirectory, endpointId, hostProcess, stderr) {
	if (process.platform === 'win32') {
		await wait(250);
		return;
	}
	const endpoint = path.join(runtimeDirectory, `terminal-host-${endpointId}.sock`);
	for (let attempt = 0; attempt < 250; attempt += 1) {
		if (hostProcess.exitCode !== null) {
			throw new Error(`Terminal Host exited before publishing its endpoint: ${stderr.join('')}`);
		}
		try {
			await access(endpoint);
			return;
		} catch {
			await wait(20);
		}
	}
	throw new Error(`Terminal Host did not publish its endpoint: ${stderr.join('')}`);
}

async function stopHost(hostProcess) {
	if (hostProcess.exitCode !== null) return;
	hostProcess.kill();
	await Promise.race([new Promise((resolve) => hostProcess.once('exit', resolve)), wait(2_000)]);
}

async function waitForMarker(marker, process_, stderr, timeoutMilliseconds) {
	const deadline = performance.now() + timeoutMilliseconds;
	while (performance.now() < deadline) {
		try {
			await access(marker);
			return;
		} catch {
			if (process_.exitCode !== null) {
				throw new Error(
					`Ghostty exited before publishing ${path.basename(marker)}: ${stderr.join('')}`,
				);
			}
			await wait(20);
		}
	}
	throw new Error(`Timed out waiting for Ghostty marker ${path.basename(marker)}.`);
}

async function waitForProcessExit(process_, timeoutMilliseconds) {
	if (process_.exitCode !== null) return;
	let timeout;
	await Promise.race([
		new Promise((resolve) => process_.once('exit', resolve)),
		new Promise((_, reject) => {
			timeout = setTimeout(
				() => reject(new Error('Timed out waiting for Ghostty to close its benchmark surface.')),
				timeoutMilliseconds,
			);
		}),
	]).finally(() => clearTimeout(timeout));
}

function stopScopedGhosttyProcesses(scope) {
	if (process.platform === 'win32') return;
	const processes = spawnSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' });
	for (const line of processes.stdout.split(/\r?\n/u)) {
		if (!line.includes('/Ghostty.app/Contents/MacOS/ghostty') || !line.includes(scope)) continue;
		const processId = Number(line.trim().split(/\s+/u)[0]);
		if (Number.isInteger(processId) && processId > 1) process.kill(processId, 'SIGTERM');
	}
}

async function runGhosttyBenchmark(file, geometry, runNumber, ghosttyApplication) {
	if (process.platform !== 'darwin') {
		throw new Error('Automatic Ghostty GUI measurement is currently supported only on macOS.');
	}
	const runtimeDirectory = await mkdtemp('/tmp/orchbee-ghostty-benchmark-');
	const readyMarker = path.join(runtimeDirectory, 'ready');
	const triggerMarker = path.join(runtimeDirectory, 'trigger');
	const doneMarker = path.join(runtimeDirectory, 'done');
	const statusFile = path.join(runtimeDirectory, 'status');
	const command = [
		`touch ${shellQuote(readyMarker)}`,
		`while test ! -f ${shellQuote(triggerMarker)}; do sleep 0.01; done`,
		`/usr/bin/time -p /bin/cat -n ${shellQuote(file)}`,
		'command_status=$?',
		`printf '%s\\n' "$command_status" > ${shellQuote(statusFile)}`,
		`touch ${shellQuote(doneMarker)}`,
		'exit "$command_status"',
	].join('; ');
	const stderr = [];
	const ghosttyProcess = spawn(
		'open',
		[
			'-W',
			'-n',
			'-a',
			ghosttyApplication,
			'--args',
			'--config-default-files=false',
			`--window-width=${geometry.columns}`,
			`--window-height=${geometry.rows}`,
			'--font-family=Menlo',
			'--font-size=15',
			'--scrollback-limit=10000000',
			'--shell-integration=none',
			'--confirm-close-surface=false',
			'-e',
			'/bin/sh',
			'-c',
			command,
		],
		{ stdio: ['ignore', 'ignore', 'pipe'] },
	);
	ghosttyProcess.stderr.on('data', (chunk) => stderr.push(chunk.toString()));
	try {
		await waitForMarker(readyMarker, ghosttyProcess, stderr, 10_000);
		await wait(500);
		const started = performance.now();
		await writeFile(triggerMarker, '');
		await waitForMarker(doneMarker, ghosttyProcess, stderr, 180_000);
		const commandWallSeconds = (performance.now() - started) / 1000;
		const status = Number((await readFile(statusFile, 'utf8')).trim());
		if (status !== 0) throw new Error(`Ghostty benchmark command exited with status ${status}.`);
		await waitForProcessExit(ghosttyProcess, 10_000);
		return {
			commandWallSeconds,
			configuration: {
				columns: geometry.columns,
				fontFamily: 'Menlo',
				fontSize: 15,
				rows: geometry.rows,
				scrollbackBytes: 10_000_000,
			},
			run: runNumber,
		};
	} finally {
		if (ghosttyProcess.exitCode === null) {
			stopScopedGhosttyProcesses(runtimeDirectory);
			ghosttyProcess.kill();
		}
		await rm(runtimeDirectory, { force: true, recursive: true });
	}
}

function readHostProcessMetrics(hostProcess) {
	if (hostProcess.pid === undefined || process.platform === 'win32') return null;
	const result = spawnSync('ps', ['-o', 'rss=,%cpu=', '-p', String(hostProcess.pid)], {
		encoding: 'utf8',
	});
	if (!result.stdout.trim()) return null;
	const [rssText, cpuText] = result.stdout.trim().split(/\s+/u);
	const cpuPercent = Number(cpuText);
	const rssKiB = Number(rssText);
	if (!Number.isFinite(cpuPercent) || !Number.isFinite(rssKiB)) return null;
	return { cpuPercent, rssKiB };
}

function parseJsonLines(chunks) {
	return chunks
		.join('')
		.split(/\r?\n/u)
		.map((line) => {
			try {
				return JSON.parse(line);
			} catch {
				return null;
			}
		})
		.filter((value) => value !== null);
}

async function beginMainThreadMeasurement(page, startEvent) {
	const key = `__orchbeeBenchmark${randomBytes(8).toString('hex')}`;
	await page.evaluate((measurementKey) => {
		const state = {
			entries: [],
			observer: null,
			startedAt: performance.now(),
			supported: PerformanceObserver.supportedEntryTypes.includes('longtask'),
		};
		if (state.supported) {
			state.observer = new PerformanceObserver((list) => {
				for (const entry of list.getEntries()) {
					state.entries.push({ duration: entry.duration, startTime: entry.startTime });
				}
			});
			state.observer.observe({ entryTypes: ['longtask'] });
		}
		globalThis[measurementKey] = state;
	}, key);
	if (startEvent !== undefined) {
		await startEvent.locator.evaluate(
			(element, { eventType, measurementKey }) => {
				element.addEventListener(
					eventType,
					() => {
						globalThis[measurementKey].startedAt = performance.now();
					},
					{ capture: true, once: true },
				);
			},
			{ eventType: startEvent.eventType, measurementKey: key },
		);
	}
	return key;
}

async function finishMainThreadMeasurement(page, key, completedAtMilliseconds) {
	return page.evaluate(
		([measurementKey, completedAt]) => {
			const state = globalThis[measurementKey];
			if (state === undefined) throw new Error('Renderer performance measurement is missing.');
			for (const entry of state.observer?.takeRecords() ?? []) {
				state.entries.push({ duration: entry.duration, startTime: entry.startTime });
			}
			state.observer?.disconnect();
			delete globalThis[measurementKey];
			const durations = state.entries
				.filter((entry) => entry.startTime >= state.startedAt)
				.map((entry) => entry.duration);
			return {
				longTaskMaximumMilliseconds: durations.length === 0 ? 0 : Math.max(...durations),
				longTaskObserverSupported: state.supported,
				wallMilliseconds: (completedAt ?? performance.now()) - state.startedAt,
			};
		},
		[key, completedAtMilliseconds],
	);
}

async function measureMainThreadOperation(page, operation, startEvent) {
	const key = await beginMainThreadMeasurement(page, startEvent);
	try {
		const completedAtMilliseconds = await operation();
		return await finishMainThreadMeasurement(page, key, completedAtMilliseconds);
	} catch (error) {
		await finishMainThreadMeasurement(page, key).catch(() => undefined);
		throw error;
	}
}

async function withHostRssSampling(hostProcess, operation) {
	let sampledPeakRssKiB = readHostProcessMetrics(hostProcess)?.rssKiB ?? null;
	const sample = () => {
		const rssKiB = readHostProcessMetrics(hostProcess)?.rssKiB;
		if (rssKiB !== undefined && (sampledPeakRssKiB === null || rssKiB > sampledPeakRssKiB)) {
			sampledPeakRssKiB = rssKiB;
		}
	};
	const interval = setInterval(sample, hostRssSamplingIntervalMilliseconds);
	try {
		const value = await operation();
		sample();
		return { sampledPeakRssKiB, value };
	} finally {
		clearInterval(interval);
	}
}

function mainProcessPeakWorkingSetKiB(metrics) {
	return metrics.find((metric) => metric.type === 'Browser')?.memory?.peakWorkingSetSize ?? null;
}

async function measureDesktopInteractions(page, root, application, hostProcess) {
	const input = page.getByRole('textbox').first();
	const canvas = page.locator('canvas');
	const accessibility = page.locator('[role="log"]');
	let workerStarts = 0;
	page.on('worker', () => {
		workerStarts += 1;
	});
	const workersBefore = page.workers().length;

	await canvas.hover();
	const renderedFramesBeforeHistory = Number(
		await root.getAttribute('data-desktop-terminal-rendered-frames'),
	);
	const history = await measureMainThreadOperation(
		page,
		async () => {
			await page.mouse.wheel(0, -2_000);
			await expect(accessibility).toHaveAttribute('aria-live', 'off');
			await expect
				.poll(async () => Number(await root.getAttribute('data-desktop-terminal-rendered-frames')))
				.toBeGreaterThan(renderedFramesBeforeHistory);
			return Number(await root.getAttribute('data-desktop-terminal-rendered-at-milliseconds'));
		},
		{ eventType: 'wheel', locator: canvas },
	);

	const primaryModifier = process.platform === 'darwin' ? 'Meta' : 'Control';
	await input.focus();
	await page.keyboard.press(`${primaryModifier}+KeyF`);
	const findSurface = page.getByRole('search');
	await expect(findSurface).toBeAttached();
	const findInput = findSurface.getByRole('textbox');
	const search = await measureMainThreadOperation(
		page,
		async () => {
			await findInput.fill('~orchbee-search-literal-miss~');
			await expect(findSurface).toHaveAttribute('data-search-state', 'complete', {
				timeout: 10_000,
			});
			return Number(
				await root.getAttribute('data-desktop-terminal-search-completed-at-milliseconds'),
			);
		},
		{ eventType: 'input', locator: findInput },
	);
	await findInput.press('Escape');
	await expect(findSurface).not.toBeAttached();

	await canvas.hover();
	await page.mouse.wheel(0, 100_000);
	await expect(accessibility).toHaveAttribute('aria-live', 'polite');

	let copy = null;
	if (process.platform === 'darwin') {
		await input.focus();
		await page.keyboard.press('Meta+KeyA');
		await expect(root).toHaveAttribute('data-desktop-terminal-selection-state', 'active');
		const copiedBytesBefore = await root.getAttribute('data-desktop-terminal-copied-bytes');
		const key = await beginMainThreadMeasurement(page, {
			eventType: 'keydown',
			locator: input,
		});
		try {
			const sampled = await withHostRssSampling(hostProcess, async () => {
				await page.keyboard.press('Meta+KeyC');
				await expect
					.poll(async () => {
						const value = await root.getAttribute('data-desktop-terminal-copied-bytes');
						return value !== copiedBytesBefore && Number(value) > 0;
					})
					.toBe(true);
			});
			const copiedAtMilliseconds = Number(
				await root.getAttribute('data-desktop-terminal-copied-at-milliseconds'),
			);
			copy = {
				...(await finishMainThreadMeasurement(page, key, copiedAtMilliseconds)),
				byteLength: Number(await root.getAttribute('data-desktop-terminal-copied-bytes')),
				hostSampledPeakRssKiB: sampled.sampledPeakRssKiB,
			};
		} catch (error) {
			await finishMainThreadMeasurement(page, key).catch(() => undefined);
			throw error;
		}
	}

	const electronProcesses = await application.evaluate(({ app }) => app.getAppMetrics());
	return {
		copy,
		electronProcesses,
		history,
		mainPeakWorkingSetKiB: mainProcessPeakWorkingSetKiB(electronProcesses),
		search,
		workers: {
			liveAfter: page.workers().length,
			started: workerStarts,
			workersBefore,
		},
	};
}

async function runDesktopBenchmark(file, runNumber) {
	const temporaryRoot = process.platform === 'win32' ? tmpdir() : '/tmp';
	const runtimeDirectory = await mkdtemp(path.join(temporaryRoot, 'orchbee-benchmark-'));
	const endpointId = randomBytes(16).toString('hex');
	const stderr = [];
	const hostProcess = spawn(
		hostBinary,
		['serve', '--runtime-directory', runtimeDirectory, '--endpoint-id', endpointId],
		{
			env: { ...process.env, ORCHBEE_TERMINAL_HOST_TRACE: '1' },
			stdio: ['ignore', 'ignore', 'pipe'],
		},
	);
	hostProcess.stderr.on('data', (chunk) => stderr.push(chunk.toString()));
	let application;
	try {
		await waitForEndpoint(runtimeDirectory, endpointId, hostProcess, stderr);
		const environment = Object.fromEntries(
			Object.entries(process.env).filter((entry) => entry[1] !== undefined),
		);
		application = await electron.launch({
			args: [desktopMain],
			env: {
				...environment,
				ORCHBEE_DESKTOP_ENDPOINT_ID: endpointId,
				ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: runtimeDirectory,
				ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: runtimeDirectory,
				ORCHBEE_DESKTOP_TRACE: '1',
				ORCHBEE_TERMINAL_HOST_BINARY: hostBinary,
				SHELL: process.platform === 'win32' ? (environment.ComSpec ?? 'cmd.exe') : '/bin/sh',
			},
		});
		const traces = [];
		application.process().stdout?.on('data', (chunk) => traces.push(chunk.toString()));
		const page = await application.firstWindow();
		const root = page.locator('#root');
		await expect(root).toHaveAttribute('data-desktop-terminal-state', 'open', {
			timeout: 10_000,
		});
		await application.evaluate(({ app }) => app.getAppMetrics());
		await wait(1_000);
		const idleElectronProcesses = await application.evaluate(({ app }) => app.getAppMetrics());
		const idleHost = readHostProcessMetrics(hostProcess);
		const command = `/usr/bin/time -p /bin/cat -n ${shellQuote(file)}; exit`;
		const input = page.getByRole('textbox');
		await input.pressSequentially(command);
		const started = performance.now();
		await input.press('Enter');
		await expect(root).toHaveAttribute('data-desktop-terminal-state', 'exited', {
			timeout: 180_000,
		});
		await expect(root).toHaveAttribute('data-desktop-terminal-final-frame-presented', 'true', {
			timeout: 10_000,
		});
		const wallSeconds = (performance.now() - started) / 1000;
		const renderer = await root.evaluate((element) => ({
			cellPixelHeight: Number(element.getAttribute('data-desktop-terminal-cell-pixel-height')),
			cellPixelWidth: Number(element.getAttribute('data-desktop-terminal-cell-pixel-width')),
			committedFrames: Number(element.getAttribute('data-desktop-terminal-committed-frames')),
			columns: Number(element.getAttribute('data-desktop-terminal-columns')),
			health: element.getAttribute('data-desktop-terminal-renderer-health'),
			renderedFrames: Number(element.getAttribute('data-desktop-terminal-rendered-frames')),
			renderer: element.getAttribute('data-desktop-terminal-renderer'),
			rows: Number(element.getAttribute('data-desktop-terminal-rows')),
		}));
		await expect
			.poll(() =>
				parseJsonLines(traces).some(
					(value) =>
						value.type === 'orchbee.desktop.copy-path' && value.reason === 'terminal-exited',
				),
			)
			.toBe(true);
		const idleFeatureTrace = parseJsonLines(traces)
			.filter(
				(value) => value.type === 'orchbee.desktop.copy-path' && value.reason === 'terminal-exited',
			)
			.at(-1);
		const interactions = await measureDesktopInteractions(page, root, application, hostProcess);
		await application.close();
		application = undefined;
		await expect
			.poll(() =>
				parseJsonLines(traces).some(
					(value) =>
						value.type === 'orchbee.desktop.copy-path' && value.broker?.clipboardCommits > 0,
				),
			)
			.toBe(true);
		const trace = parseJsonLines(traces)
			.filter(
				(value) => value.type === 'orchbee.desktop.copy-path' && value.broker?.clipboardCommits > 0,
			)
			.at(-1);
		const hostSelectionExport = parseJsonLines(stderr)
			.filter((value) => value.type === 'orchbee.terminal-host.selection-export')
			.at(-1);
		return {
			copyPath: trace,
			hostSelectionExport,
			idle: {
				electronProcesses: idleElectronProcesses,
				featureTrace: idleFeatureTrace,
				host: idleHost,
			},
			interactions,
			renderer,
			run: runNumber,
			wallSeconds,
		};
	} finally {
		await application?.close().catch(() => undefined);
		await stopHost(hostProcess);
		await rm(runtimeDirectory, { force: true, recursive: true });
	}
}

const options = parseArguments(process.argv.slice(2));
const file = await stat(options.file);
if (!file.isFile()) throw new Error('Benchmark input is not a regular file.');
const results = [];
const measuredGhosttyResults = [];
let referenceGeometry;
for (let run = 1; run <= options.runs; run += 1) {
	const ghosttyFirst = options.measureGhostty && run > 1 && run % 2 === 0;
	if (ghosttyFirst) {
		const ghosttyResult = await runGhosttyBenchmark(
			options.file,
			referenceGeometry,
			run,
			options.ghosttyApplication,
		);
		measuredGhosttyResults.push(ghosttyResult);
		console.log(JSON.stringify({ type: 'orchbee.ghostty.benchmark-run', ...ghosttyResult }));
	}
	const result = await runDesktopBenchmark(options.file, run);
	results.push(result);
	console.log(JSON.stringify({ type: 'orchbee.desktop.benchmark-run', ...result }));
	const geometry = result.renderer;
	if (geometry.columns <= 0 || geometry.rows <= 0) {
		throw new Error('Desktop benchmark did not produce a positive terminal geometry.');
	}
	if (
		referenceGeometry !== undefined &&
		(referenceGeometry.columns !== geometry.columns || referenceGeometry.rows !== geometry.rows)
	) {
		throw new Error('Desktop benchmark did not produce one stable positive terminal geometry.');
	}
	referenceGeometry ??= geometry;
	if (options.measureGhostty && !ghosttyFirst) {
		const ghosttyResult = await runGhosttyBenchmark(
			options.file,
			referenceGeometry,
			run,
			options.ghosttyApplication,
		);
		measuredGhosttyResults.push(ghosttyResult);
		console.log(JSON.stringify({ type: 'orchbee.ghostty.benchmark-run', ...ghosttyResult }));
	}
}
const desktop = summary(results.map((result) => result.wallSeconds));
const ghosttyRuns = options.measureGhostty
	? measuredGhosttyResults.map((result) => result.commandWallSeconds)
	: options.ghosttyRuns;
const ghostty = summary(ghosttyRuns);
const hostOnly = summary(options.hostOnlyRuns);
const ghosttyRatio = ghostty === null ? null : desktop.median / ghostty.median;
const slice5Ratio =
	options.slice5Median === undefined ? null : desktop.median / options.slice5Median;
const finiteSummary = (values) =>
	values.length === results.length && values.every(Number.isFinite) ? summary(values) : null;
const historyMilliseconds = finiteSummary(
	results.map((result) => result.interactions.history.wallMilliseconds),
);
const historyLongTasks = finiteSummary(
	results.map((result) => result.interactions.history.longTaskMaximumMilliseconds),
);
const searchMilliseconds = finiteSummary(
	results.map((result) => result.interactions.search.wallMilliseconds),
);
const searchLongTasks = finiteSummary(
	results.map((result) => result.interactions.search.longTaskMaximumMilliseconds),
);
const copyBrokerMilliseconds = finiteSummary(
	results.map((result) => result.copyPath?.broker?.lastSelectionCopyMilliseconds),
);
const copyRendererMilliseconds = finiteSummary(
	results.map((result) => result.interactions.copy?.wallMilliseconds),
);
const hostLockHoldMilliseconds = finiteSummary(
	results.map((result) => result.hostSelectionExport?.lockHoldNanoseconds / 1_000_000),
);
const selectionBytes = finiteSummary(
	results.map((result) => result.copyPath?.broker?.selectionExportBytesForwarded),
);
const hostSampledPeakRssKiB = finiteSummary(
	results.map((result) => result.interactions.copy?.hostSampledPeakRssKiB),
);
const mainPeakWorkingSetKiB = finiteSummary(
	results.map((result) => result.interactions.mainPeakWorkingSetKiB),
);
const exactActiveGeometry = results.every(
	(result) =>
		result.renderer.columns === activeTargetColumns && result.renderer.rows === activeTargetRows,
);
const longTaskObservationAvailable = results.every(
	(result) =>
		result.interactions.history.longTaskObserverSupported &&
		result.interactions.search.longTaskObserverSupported &&
		(result.interactions.copy?.longTaskObserverSupported ?? false),
);
const idleFeaturePathPassed = results.every((result) => {
	const metrics = result.idle.featureTrace?.broker;
	return (
		result.interactions.workers.workersBefore === 0 &&
		metrics?.rowWindowsForwarded === 0 &&
		metrics?.selectionExportChunksForwarded === 0 &&
		metrics?.clipboardCommits === 0
	);
});
const activeWorkerLifecyclePassed = results.every(
	(result) =>
		result.interactions.workers.started === 1 && result.interactions.workers.liveAfter === 0,
);
const activeTargetApplicable =
	file.size >= comparativeFixtureMinimumBytes && process.platform === 'darwin';
const activeTargetPassed = activeTargetApplicable
	? exactActiveGeometry &&
		longTaskObservationAvailable &&
		activeWorkerLifecyclePassed &&
		historyMilliseconds !== null &&
		historyMilliseconds.ninetyFifthPercentile <= 50 &&
		historyLongTasks !== null &&
		historyLongTasks.maximum < 50 &&
		searchMilliseconds !== null &&
		searchMilliseconds.ninetyFifthPercentile <= 750 &&
		searchLongTasks !== null &&
		searchLongTasks.maximum < 50 &&
		copyBrokerMilliseconds !== null &&
		copyBrokerMilliseconds.ninetyFifthPercentile <= 1_000 &&
		hostLockHoldMilliseconds !== null &&
		selectionBytes !== null &&
		hostSampledPeakRssKiB !== null &&
		mainPeakWorkingSetKiB !== null
	: null;
const ghosttyTargetApplicable =
	ghosttyRatio !== null && file.size >= comparativeFixtureMinimumBytes;
const slice5TargetApplicable = slice5Ratio !== null && file.size >= comparativeFixtureMinimumBytes;
const comparisonTargetApplicable = ghosttyTargetApplicable || slice5TargetApplicable;
const comparisonTargetPassed = comparisonTargetApplicable
	? (!ghosttyTargetApplicable || ghosttyRatio <= 1.15) &&
		(!slice5TargetApplicable || slice5Ratio <= 1.05) &&
		idleFeaturePathPassed
	: null;
const report = {
	active: {
		copy: {
			brokerMilliseconds: copyBrokerMilliseconds,
			hostLockHoldMilliseconds,
			hostRssSamplingIntervalMilliseconds,
			hostSampledPeakRssKiB,
			mainPeakWorkingSetKiB,
			rendererObservedMilliseconds: copyRendererMilliseconds,
			transferredBytes: selectionBytes,
		},
		history: {
			longTaskMaximumMilliseconds: historyLongTasks,
			wallMilliseconds: historyMilliseconds,
		},
		search: {
			longTaskMaximumMilliseconds: searchLongTasks,
			wallMilliseconds: searchMilliseconds,
		},
		workers: results.map((result) => result.interactions.workers),
	},
	activeTarget: {
		activeWorkerLifecyclePassed,
		applicable: activeTargetApplicable,
		exactGeometry: exactActiveGeometry,
		geometry: { columns: activeTargetColumns, rows: activeTargetRows },
		longTaskObservationAvailable,
		maximumCopyP95Milliseconds: 1_000,
		maximumHistoryP95Milliseconds: 50,
		maximumSearchP95Milliseconds: 750,
		passed: activeTargetPassed,
		referencePlatform: 'darwin',
	},
	desktop,
	file: { bytes: file.size, path: options.file },
	ghostty,
	ghosttyConfiguration: measuredGhosttyResults[0]?.configuration ?? null,
	ghosttyRatio,
	hostOnly,
	hostOnlyIncrementalSeconds: hostOnly === null ? null : desktop.median - hostOnly.median,
	idleFeaturePathPassed,
	samplingOrder: options.measureGhostty ? 'paired-alternating-after-desktop-bootstrap' : null,
	target: {
		applicable: comparisonTargetApplicable,
		fixtureMinimumBytes: comparativeFixtureMinimumBytes,
		ghosttyApplicable: ghosttyTargetApplicable,
		maximumGhosttyRatio: 1.15,
		maximumSlice5Ratio: 1.05,
		passed: comparisonTargetPassed,
		slice5Applicable: slice5TargetApplicable,
		slice5MedianSeconds: options.slice5Median ?? null,
		slice5Ratio,
	},
	type: 'orchbee.desktop.benchmark-summary',
};
console.log(JSON.stringify(report, null, 2));
if (report.target.passed === false) process.exitCode = 1;
if (report.activeTarget.passed === false) process.exitCode = 1;
