import { once } from 'node:events';

export function buildTerminalBenchmarkCommand(file, completionMarker) {
	return `time -p cat -n ${shellQuote(file)}; printf '\\n%s\\n' ${shellQuote(completionMarker)}; read orchbee_benchmark_release; exit`;
}

export function resolveTerminalBenchmarkInteractions(root) {
	return {
		accessibilityAnnouncement: root.locator('[role="status"]'),
		accessibilityScreen: root.locator('[role="list"]'),
		input: root.getByRole('textbox').first(),
		measurementTarget: root,
	};
}

export async function positionTerminalBenchmarkPointer(page, root) {
	const bounds = await root.boundingBox();
	if (
		bounds === null ||
		!Number.isFinite(bounds.x) ||
		!Number.isFinite(bounds.y) ||
		!Number.isFinite(bounds.width) ||
		!Number.isFinite(bounds.height) ||
		bounds.width <= 0 ||
		bounds.height <= 0
	) {
		throw new Error('Terminal benchmark surface has no positive finite bounding box.');
	}
	await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
}

export function summarizeElectronProcessMemory(metrics) {
	const finiteMemoryValues = (field) =>
		metrics.flatMap((metric) => {
			const value = metric.memory?.[field];
			return Number.isFinite(value) && value >= 0 ? [value] : [];
		});
	const sum = (values) =>
		values.length === 0 ? null : values.reduce((total, value) => total + value, 0);
	const peakFor = (type) =>
		sum(
			metrics
				.filter((metric) => metric.type === type)
				.flatMap((metric) => {
					const value = metric.memory?.peakWorkingSetSize;
					return Number.isFinite(value) && value >= 0 ? [value] : [];
				}),
		);
	return {
		browserPeakWorkingSetKiB: peakFor('Browser'),
		electronCurrentWorkingSetKiB: sum(finiteMemoryValues('workingSetSize')),
		electronPeakWorkingSetKiB: sum(finiteMemoryValues('peakWorkingSetSize')),
		gpuPeakWorkingSetKiB: peakFor('GPU'),
		rendererPeakWorkingSetKiB: peakFor('Tab'),
	};
}

export function summarizeProjectionBrokerMetrics(brokers) {
	if (!Array.isArray(brokers) || brokers.length === 0) return null;
	const finiteValues = (field) =>
		brokers.flatMap((broker) => {
			const value = broker?.[field];
			return Number.isFinite(value) && value >= 0 ? [value] : [];
		});
	const sum = (field) => finiteValues(field).reduce((total, value) => total + value, 0);
	const maximum = (field) => {
		const values = finiteValues(field);
		return values.length === 0 ? null : Math.max(...values);
	};
	return {
		clipboardCommits: sum('clipboardCommits'),
		lastSelectionCopyMilliseconds: maximum('lastSelectionCopyMilliseconds'),
		rowWindowBytesForwarded: sum('rowWindowBytesForwarded'),
		rowWindowsForwarded: sum('rowWindowsForwarded'),
		selectionExportBytesForwarded: sum('selectionExportBytesForwarded'),
		selectionExportChunksForwarded: sum('selectionExportChunksForwarded'),
	};
}

export async function terminateBenchmarkApplication(application) {
	const childProcess = application.process();
	if (childProcess.exitCode !== null || childProcess.signalCode !== null) return;
	const exit = once(childProcess, 'exit').then(() => true);
	childProcess.kill();
	if (await resolveBeforeTimeout(exit, 5_000)) return;
	childProcess.kill('SIGKILL');
	if (await resolveBeforeTimeout(exit, 5_000)) return;
	throw new Error('Benchmark-owned Electron process did not terminate.');
}

function shellQuote(value) {
	return `'${value.replaceAll("'", `'\\''`)}'`;
}

async function resolveBeforeTimeout(operation, milliseconds) {
	let timeout;
	return await Promise.race([
		operation,
		new Promise((resolve) => {
			timeout = setTimeout(() => resolve(false), milliseconds);
		}),
	]).finally(() => clearTimeout(timeout));
}
