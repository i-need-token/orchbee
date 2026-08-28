import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';

import {
	buildTerminalBenchmarkCommand,
	positionTerminalBenchmarkPointer,
	resolveTerminalBenchmarkInteractions,
	summarizeElectronProcessMemory,
	summarizeProjectionBrokerMetrics,
	terminateBenchmarkApplication,
} from './terminal-benchmark-interactions.mjs';

test('terminal output waits for benchmark interactions before exiting', () => {
	assert.equal(
		buildTerminalBenchmarkCommand("/tmp/large output's.txt", 'orchbee-complete-123'),
		"time -p cat -n '/tmp/large output'\\''s.txt'; printf '\\n%s\\n' 'orchbee-complete-123'; read orchbee_benchmark_release; exit",
	);
});

test('terminal interactions stay scoped to the stable presentation root', async () => {
	const accessibilityAnnouncement = {};
	const accessibilityScreen = {};
	const input = {};
	const calls = [];
	const root = {
		boundingBox: async () => ({ height: 80, width: 100, x: 10, y: 20 }),
		getByRole: (role) => {
			calls.push(['role', role]);
			return { first: () => input };
		},
		locator: (selector) => {
			calls.push(['locator', selector]);
			return selector === '[role="status"]' ? accessibilityAnnouncement : accessibilityScreen;
		},
	};
	const moves = [];
	const page = { mouse: { move: async (x, y) => moves.push([x, y]) } };

	const interactions = resolveTerminalBenchmarkInteractions(root);
	await positionTerminalBenchmarkPointer(page, root);

	assert.equal(interactions.accessibilityAnnouncement, accessibilityAnnouncement);
	assert.equal(interactions.accessibilityScreen, accessibilityScreen);
	assert.equal(interactions.input, input);
	assert.equal(interactions.measurementTarget, root);
	assert.deepEqual(calls, [
		['locator', '[role="status"]'],
		['locator', '[role="list"]'],
		['role', 'textbox'],
	]);
	assert.deepEqual(moves, [[60, 60]]);
});

test('pointer positioning rejects a missing terminal geometry', async () => {
	const root = { boundingBox: async () => null };
	const page = { mouse: { move: async () => assert.fail('mouse must not move') } };

	await assert.rejects(
		positionTerminalBenchmarkPointer(page, root),
		/Terminal benchmark surface has no positive finite bounding box/u,
	);
});

test('electron memory summary separates renderer, GPU, browser, and aggregate working sets', () => {
	assert.deepEqual(
		summarizeElectronProcessMemory([
			{ memory: { peakWorkingSetSize: 100, workingSetSize: 90 }, type: 'Browser' },
			{ memory: { peakWorkingSetSize: 200, workingSetSize: 180 }, type: 'Tab' },
			{ memory: { peakWorkingSetSize: 20, workingSetSize: 15 }, type: 'Tab' },
			{ memory: { peakWorkingSetSize: 80, workingSetSize: 70 }, type: 'GPU' },
			{ memory: {}, type: 'Utility' },
		]),
		{
			browserPeakWorkingSetKiB: 100,
			electronCurrentWorkingSetKiB: 355,
			electronPeakWorkingSetKiB: 400,
			gpuPeakWorkingSetKiB: 80,
			rendererPeakWorkingSetKiB: 220,
		},
	);
});

test('projection broker summary aggregates isolated trace metrics deterministically', () => {
	assert.deepEqual(
		summarizeProjectionBrokerMetrics([
			{
				clipboardCommits: 1,
				lastSelectionCopyMilliseconds: 4,
				rowWindowBytesForwarded: 100,
				rowWindowsForwarded: 2,
				selectionExportBytesForwarded: 300,
				selectionExportChunksForwarded: 3,
			},
			{
				clipboardCommits: 2,
				lastSelectionCopyMilliseconds: null,
				rowWindowBytesForwarded: 50,
				rowWindowsForwarded: 1,
				selectionExportBytesForwarded: 200,
				selectionExportChunksForwarded: 2,
			},
		]),
		{
			clipboardCommits: 3,
			lastSelectionCopyMilliseconds: 4,
			rowWindowBytesForwarded: 150,
			rowWindowsForwarded: 3,
			selectionExportBytesForwarded: 500,
			selectionExportChunksForwarded: 5,
		},
	);
});

test('benchmark application termination bypasses product quit interaction', async () => {
	const childProcess = new EventEmitter();
	childProcess.exitCode = null;
	childProcess.signalCode = null;
	const signals = [];
	childProcess.kill = (signal = 'SIGTERM') => {
		signals.push(signal);
		childProcess.signalCode = signal;
		queueMicrotask(() => childProcess.emit('exit', null, signal));
		return true;
	};
	const application = { process: () => childProcess };

	await terminateBenchmarkApplication(application);

	assert.deepEqual(signals, ['SIGTERM']);
});
