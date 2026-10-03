import { assertRequiredCoverage } from './model-inventory.mjs';

const defaultVariant = { name: 'default' };

const mebibyte = 1024 ** 2;
const minimumHeapMiB = 256;
const processReserveMiB = 512;
const maximumHeapMiB = 4096;

/** Budget explicit JVM heaps together, leaving 40% of memory for other processes. */
export function modelConcurrency(requested, processors, memoryBytes) {
	if (requested !== undefined) {
		if (!/^[1-9]\d*$/u.test(requested) || !Number.isSafeInteger(Number(requested))) {
			throw new Error('--jobs must be a positive safe integer');
		}
		const jobs = Number(requested);
		modelHeapMiB(jobs, memoryBytes);
		return jobs;
	}
	const memorySlots = Math.floor(
		(memoryBytes * 0.6) / ((maximumHeapMiB + processReserveMiB) * mebibyte),
	);
	return Math.max(1, Math.min(processors, memorySlots));
}

export function modelHeapMiB(concurrency, memoryBytes) {
	const heap = Math.floor((memoryBytes * 0.6) / concurrency / mebibyte) - processReserveMiB;
	if (heap < minimumHeapMiB) {
		throw new Error(`TLC concurrency ${concurrency} exceeds the available JVM memory budget`);
	}
	return Math.min(maximumHeapMiB, heap);
}

export function modelWorkers(model) {
	return model.workers ?? 1;
}

/** Keep one completed TLC log and its machine-readable result in one stdout write. */
export function formatTlcOutput(modelId, output, report) {
	const terminatedOutput = output.endsWith('\n') ? output : `${output}\n`;
	const result = report === undefined ? '' : `TLA_RESULT ${JSON.stringify(report)}\n`;
	return `\nTLC output for ${modelId}:\n${terminatedOutput}${result}`;
}

export function writeTlcOutput(stream, modelId, output, report) {
	return new Promise((resolve, reject) => {
		stream.write(formatTlcOutput(modelId, output, report), (error) => {
			if (error) reject(error);
			else resolve();
		});
	});
}

/** Model variants share coverage after all pass, while every variant consumes one bounded slot. */
export async function runModelChecks(models, concurrency, runVariant, onSkipped = () => {}) {
	if (!Number.isSafeInteger(concurrency) || concurrency < 1)
		throw new Error('concurrency must be a positive safe integer');
	const modelStates = models.map((model) => {
		const variants = model.variants ?? [defaultVariant];
		return {
			completed: 0,
			model,
			outputs: Array.from({ length: variants.length }),
			variants,
		};
	});
	const jobs = modelStates.flatMap((state) =>
		state.variants.map((variant, variantIndex) => ({ state, variant, variantIndex })),
	);
	let nextIndex = 0;
	let configurationCount = 0;
	let failure;
	let failed = false;
	function recordFailure(error) {
		if (!failed) failure = error;
		failed = true;
	}
	async function worker() {
		while (!failed && nextIndex < jobs.length) {
			const job = jobs[nextIndex];
			nextIndex += 1;
			try {
				job.state.outputs[job.variantIndex] = await runVariant(
					job.state.model,
					job.variant,
					recordFailure,
				);
				configurationCount += 1;
				job.state.completed += 1;
				if (job.state.completed === job.state.variants.length) {
					assertRequiredCoverage(
						job.state.model.id,
						job.state.outputs,
						job.state.model.requiredCoverage,
					);
				}
			} catch (error) {
				recordFailure(error);
			}
		}
	}
	await Promise.all(Array.from({ length: Math.min(concurrency, jobs.length) }, worker));
	if (failed) {
		for (const { state, variant } of jobs.slice(nextIndex)) onSkipped(state.model, variant);
		throw failure;
	}
	return configurationCount;
}
