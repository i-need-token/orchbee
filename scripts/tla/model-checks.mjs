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
export async function runModelChecks(
	models,
	concurrency,
	runVariant,
	onSkipped = () => {},
	{
		processors = Infinity,
		onModelOutcome = () => {},
		previousModelFailures = new Map(),
		signal,
	} = {},
) {
	if (!Number.isSafeInteger(concurrency) || concurrency < 1)
		throw new Error('concurrency must be a positive safe integer');
	if (processors !== Infinity && (!Number.isSafeInteger(processors) || processors < 1))
		throw new Error('processors must be a positive safe integer');
	const modelStates = models.map((model) => {
		const variants = model.variants ?? [defaultVariant];
		return {
			completed: 0,
			errors: [],
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
	let activeCount = 0;
	let activeWorkers = 0;
	const waiters = new Set();
	function wake() {
		for (const resolve of waiters) resolve();
		waiters.clear();
	}
	function recordFailure(error) {
		failure ??= error;
		failed = true;
		wake();
	}
	const cancel = () =>
		recordFailure(
			signal.reason instanceof Error ? signal.reason : new Error('TLC checking cancelled'),
		);
	if (signal?.aborted) cancel();
	else signal?.addEventListener('abort', cancel, { once: true });
	async function worker() {
		while (!failed && nextIndex < jobs.length) {
			const job = jobs[nextIndex];
			const weight = modelWorkers(job.state.model);
			if (activeCount > 0 && activeWorkers + weight > processors) {
				await new Promise((resolve) => waiters.add(resolve));
				continue;
			}
			nextIndex += 1;
			activeCount += 1;
			activeWorkers += weight;
			try {
				job.state.outputs[job.variantIndex] = await runVariant(
					job.state.model,
					job.variant,
					recordFailure,
				);
				configurationCount += 1;
			} catch (error) {
				job.state.errors.push(error);
				if (error instanceof HistoricalModelFailure) failure ??= error;
				else recordFailure(error);
			} finally {
				job.state.completed += 1;
				try {
					if (job.state.completed === job.state.variants.length) {
						let modelError = job.state.errors[0] ?? previousModelFailures.get(job.state.model.id);
						if (!modelError) {
							try {
								assertRequiredCoverage(
									job.state.model.id,
									job.state.outputs,
									job.state.model.requiredCoverage,
								);
							} catch (error) {
								modelError = error;
								recordFailure(error);
							}
						}
						await onModelOutcome(job.state.model, {
							status: modelError ? 'failed' : 'passed',
							error: modelError,
						});
						failure ??= modelError;
					}
				} catch (error) {
					recordFailure(error);
				}
				activeCount -= 1;
				activeWorkers -= weight;
				wake();
			}
		}
	}
	await Promise.all(Array.from({ length: Math.min(concurrency, jobs.length) }, worker));
	signal?.removeEventListener('abort', cancel);
	if (failure) {
		for (const { state, variant } of jobs.slice(nextIndex)) onSkipped(state.model, variant);
		throw failure;
	}
	return configurationCount;
}

/** Prior terminal outcomes remain failures without closing admission for unchecked work. */
export class HistoricalModelFailure extends Error {}
