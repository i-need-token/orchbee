import { createHash, randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, open, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { availableParallelism, tmpdir, totalmem } from 'node:os';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';

import {
	modelConcurrency,
	HistoricalModelFailure,
	modelHeapMiB,
	modelMaximumHeapMiB,
	runModelChecks,
	writeTlcOutput,
} from './model-checks.mjs';
import {
	inventoryPath,
	inventoryInputPaths,
	assertInputAdmission,
	parseStateReport,
	repositoryRoot,
	selectModelsSince,
	tlcExitError,
	validateInventory,
} from './model-inventory.mjs';
import {
	assertCompleteTlcOutput,
	executionPlans,
	sha256Bytes,
	stageInputs,
} from './model-inputs.mjs';
import { provisionJar } from './toolchain.mjs';
import { ModelRunRecords } from './model-records.mjs';
import { configurationIdentity, ModelSuccessCache } from './model-cache.mjs';
import { cacheRuntimeIdentity } from './model-cache-identity.mjs';

const inventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));
const toolsDirectory = path.join(repositoryRoot, '.tools');
const bundledJarPath = path.join(toolsDirectory, `tla2tools-${inventory.tool.version}.jar`);
const configuredJarPath = process.env.ORCHBEE_TLA2TOOLS_JAR;
const jarPath = configuredJarPath ? path.resolve(configuredJarPath) : bundledJarPath;
let javaExecutable = process.env.ORCHBEE_TLA_JAVA ?? 'java';

function parseArguments(commandArguments) {
	commandArguments = commandArguments.filter((argument) => argument !== '--');
	const result = {
		all: false,
		changedSince: undefined,
		jobs: undefined,
		maximumHeapMiB: undefined,
		models: [],
		workspace: undefined,
		runDirectory: undefined,
		cacheDirectory: undefined,
	};
	for (let index = 0; index < commandArguments.length; index += 1) {
		const argument = commandArguments[index];
		if (argument === '--all') {
			result.all = true;
			continue;
		}
		if (
			[
				'--changed-since',
				'--models',
				'--workspace',
				'--jobs',
				'--max-heap-mib',
				'--run-dir',
				'--cache-dir',
			].includes(argument)
		) {
			const value = commandArguments[index + 1];
			if (value === undefined || value.startsWith('--'))
				throw new Error(`${argument} requires a value`);
			index += 1;
			if (argument === '--changed-since') result.changedSince = value;
			if (argument === '--models') result.models.push(...value.split(',').filter(Boolean));
			if (argument === '--workspace') result.workspace = value;
			if (argument === '--jobs') result.jobs = value;
			if (argument === '--max-heap-mib') result.maximumHeapMiB = value;
			if (argument === '--run-dir') result.runDirectory = value;
			if (argument === '--cache-dir') result.cacheDirectory = value;
			continue;
		}
		throw new Error(`Unknown TLA+ runner argument: ${argument}`);
	}
	const selectorCount =
		Number(result.all) +
		Number(result.changedSince !== undefined) +
		Number(result.models.length > 0) +
		Number(result.workspace !== undefined);
	if (selectorCount !== 1) {
		throw new Error(
			'Select models with exactly one of --all, --changed-since, --models, or --workspace',
		);
	}
	if (result.runDirectory !== undefined && result.cacheDirectory !== undefined)
		throw new Error('--run-dir and --cache-dir are mutually exclusive');
	return result;
}

async function sha256(filePath) {
	return createHash('sha256')
		.update(await readFile(filePath))
		.digest('hex');
}

async function selectModels(selection) {
	if (selection.all) return inventory.models;
	if (selection.workspace !== undefined) {
		const models = inventory.models.filter(({ workspace }) => workspace === selection.workspace);
		if (models.length === 0) throw new Error(`No TLA+ models belong to ${selection.workspace}`);
		return models;
	}
	if (selection.models.length > 0) {
		const requested = new Set(selection.models);
		const models = inventory.models.filter(({ id }) => requested.has(id));
		const missing = selection.models.filter((id) => !models.some((model) => model.id === id));
		if (missing.length > 0) throw new Error(`Unknown TLA+ model ids: ${missing.join(', ')}`);
		return models;
	}
	try {
		return await selectModelsSince(inventory, selection.changedSince);
	} catch (error) {
		process.stderr.write(
			`Could not resolve changed TLA+ models; running every model: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		return inventory.models;
	}
}

function configuredTimeout(model, defaultTimeout = inventory.defaultTimeoutMilliseconds) {
	const override = process.env.ORCHBEE_TLA_TIMEOUT_MILLISECONDS;
	const timeout =
		override === undefined ? (model.timeoutMilliseconds ?? defaultTimeout) : Number(override);
	if (!Number.isSafeInteger(timeout) || timeout <= 0) {
		throw new Error('ORCHBEE_TLA_TIMEOUT_MILLISECONDS must be a positive integer');
	}
	return timeout;
}

async function runChild(
	arguments_,
	timeoutMilliseconds,
	modelId,
	workingDirectory,
	logPath,
	recordFailure,
	signal,
) {
	if (signal?.aborted) throw signal.reason;
	const log = await open(logPath, 'wx');
	let result;
	try {
		if (signal?.aborted) throw signal.reason;
		result = await new Promise((resolve) => {
			const child = spawn(javaExecutable, arguments_, {
				cwd: workingDirectory,
				stdio: ['ignore', log.fd, log.fd],
			});
			let launchError;
			let processError;
			let forceKill;
			let timedOut = false;
			const cancel = () => {
				processError ??= signal.reason;
				recordFailure(processError);
				child.kill('SIGTERM');
				clearTimeout(forceKill);
				forceKill = setTimeout(() => child.kill('SIGKILL'), 5_000);
			};
			signal?.addEventListener('abort', cancel, { once: true });
			const timeout = setTimeout(() => {
				timedOut = true;
				processError ??= tlcExitError(modelId, timeoutMilliseconds, {
					code: undefined,
					signal: undefined,
					timedOut,
				});
				recordFailure(processError);
				child.kill('SIGTERM');
				forceKill = setTimeout(() => child.kill('SIGKILL'), 5_000);
			}, timeoutMilliseconds);
			child.once('error', (error) => {
				launchError = error;
				processError ??= error;
				recordFailure(processError);
			});
			child.once('exit', (code, childSignal) => {
				processError ??= tlcExitError(modelId, timeoutMilliseconds, {
					code,
					signal: childSignal,
					timedOut,
				});
				if (processError) recordFailure(processError);
			});
			child.once('close', (code, childSignal) => {
				clearTimeout(timeout);
				clearTimeout(forceKill);
				processError ??=
					launchError ??
					tlcExitError(modelId, timeoutMilliseconds, { code, signal: childSignal, timedOut });
				if (processError !== undefined) recordFailure(processError);
				resolve({ code, error: processError, signal: childSignal, timedOut });
			});
			child.once('close', () => signal?.removeEventListener('abort', cancel));
		});
	} finally {
		await log.close();
	}
	const bytes = await readFile(logPath);
	return { ...result, bytes, output: bytes.toString('utf8') };
}

async function capture(command, arguments_) {
	return new Promise((resolve, reject) => {
		const child = spawn(command, arguments_, {
			cwd: repositoryRoot,
			stdio: ['ignore', 'pipe', 'pipe'],
		});
		let stdout = '';
		let stderr = '';
		child.stdout.on('data', (chunk) => {
			stdout += chunk.toString();
		});
		child.stderr.on('data', (chunk) => {
			stderr += chunk.toString();
		});
		child.once('error', reject);
		child.once('close', (code, signal) => {
			if (code !== 0 || signal) reject(new Error(command + ' identity probe failed: ' + stderr));
			else resolve({ stdout, stderr });
		});
	});
}

async function frozenContext(slots, processors, heapMiB, maximumHeapMiB) {
	const revision = (await capture('git', ['rev-parse', 'HEAD'])).stdout.trim();
	const status = (await capture('git', ['status', '--porcelain', '--untracked-files=normal']))
		.stdout;
	if (status.trim())
		throw new Error('--run-dir requires a clean frozen checkout, including untracked files');
	const java = await capture(javaExecutable, ['-XshowSettings:properties', '-version']);
	const helpers = {};
	for (const file of (await readdir(path.join(repositoryRoot, 'scripts/tla')))
		.filter((name) => name.endsWith('.mjs') && !name.endsWith('.test.mjs'))
		.toSorted())
		helpers[file] = await sha256(path.join(repositoryRoot, 'scripts/tla', file));
	const registeredPaths = inventoryInputPaths(inventory);
	await assertInputAdmission(
		[...registeredPaths, ...Object.keys(helpers).map((name) => `scripts/tla/${name}`)],
		revision,
	);
	const registeredInputs = Object.fromEntries(
		await Promise.all(
			registeredPaths.map(async (input) => [input, await sha256(path.join(repositoryRoot, input))]),
		),
	);
	return {
		repository: repositoryRoot,
		revision,
		inventorySha256: await sha256(inventoryPath),
		helpers,
		registeredInputs,
		jarSha256: await sha256(jarPath),
		javaExecutable,
		javaIdentity: java.stdout + java.stderr,
		javaEnvironment: Object.fromEntries(
			['JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS'].map((name) => [
				name,
				process.env[name] ?? null,
			]),
		),
		nodeExecutable: process.execPath,
		nodeSha256: await sha256(process.execPath),
		nodeVersion: process.version,
		nodePlatform: process.platform,
		nodeArchitecture: process.arch,
		nodeArguments: process.execArgv,
		nodeOptions: process.env.NODE_OPTIONS ?? null,
		slots,
		processors,
		heapMiB,
		maximumHeapMiB,
		fingerprint: 0,
		liveness: 'final',
		coverage: 1,
		gc: 'UseParallelGC',
	};
}

async function runVariant(plan, recordFailure, context) {
	const { model, variant, proof } = plan;
	const { logsDirectory, records, cache, outcomes, signal, counters } = context;
	const label = variant.name === 'default' ? model.id : model.id + '/' + variant.name;
	const previous = records?.configuration(plan);
	if (previous) {
		if (previous.record.status === 'failed') {
			process.stdout.write(
				'TLA_FAILURE ' +
					JSON.stringify({
						...previous.record.report,
						status: 'failed',
						historical: true,
						message: previous.record.message,
					}) +
					'\n',
			);
			throw new HistoricalModelFailure(previous.record.message ?? label + ' failed previously');
		}
		counters.reused += 1;
		await writeTlcOutput(process.stdout, label, previous.output, {
			...previous.record.report,
			reused: true,
		});
		return previous.output;
	}
	const cached = cache?.configuration(plan);
	if (cached) {
		signal.throwIfAborted();
		const logPath = path.join(
			logsDirectory,
			model.id + '--' + variant.name + '--' + randomUUID() + '.log',
		);
		await writeFile(logPath, cached.bytes, { flag: 'wx' });
		if (sha256Bytes(await readFile(logPath)) !== cached.logSha256)
			throw new Error('Reused TLC log copy failed integrity validation');
		const output = cached.bytes.toString('utf8');
		const report = {
			...parseStateReport(output),
			model: model.id,
			variant: variant.name,
			modelConfigSha256: proof.modelConfigSha256,
			workers: proof.workers,
			heapMiB: proof.heapMiB,
			durationMilliseconds: cached.durationMilliseconds,
			logPath,
			reused: true,
			reuseSource: 'local-cache',
		};
		outcomes.set(JSON.stringify([model.id, variant.name]), {
			bytes: cached.bytes,
			durationMilliseconds: cached.durationMilliseconds,
		});
		counters.reused += 1;
		await writeTlcOutput(process.stdout, label, output, report);
		return output;
	}
	const temporaryParent = records ? path.join(records.root, 'attempts') : toolsDirectory;
	await mkdir(temporaryParent, { recursive: true });
	const temporaryDirectory = await mkdtemp(path.join(temporaryParent, 'tla-' + model.id + '-'));
	const token = model.id + '--' + variant.name + '--' + randomUUID();
	const inputDirectory = records ? path.join(records.root, 'inputs', token) : temporaryDirectory;
	let committed = false;
	try {
		if (records) await mkdir(inputDirectory);
		const inputs = (await stageInputs(plan, inputDirectory)).map((file) => ({
			path: 'inputs/' + token + '/' + file.name,
			sha256: file.sha256,
		}));
		const metadataDirectory = path.join(temporaryDirectory, 'states');
		const jvmTemporaryDirectory = path.join(temporaryDirectory, 'jvm-temp');
		await mkdir(metadataDirectory);
		await mkdir(jvmTemporaryDirectory);
		const log = token + '.log';
		const logPath = path.join(logsDirectory, log);
		process.stdout.write(
			'\nChecking ' +
				label +
				' (timeout ' +
				proof.timeoutMilliseconds +
				'ms, workers ' +
				proof.workers +
				')\nLog: ' +
				logPath +
				'\n',
		);
		counters.fresh += 1;
		const start = Date.now();
		const childResult = await runChild(
			[
				'-Xmx' + proof.heapMiB + 'm',
				'-XX:+UseParallelGC',
				'-Djava.io.tmpdir=' + jvmTemporaryDirectory,
				'-jar',
				jarPath,
				'-fp',
				'0',
				'-lncheck',
				'final',
				'-coverage',
				'1',
				'-workers',
				String(proof.workers),
				'-metadir',
				metadataDirectory,
				'-config',
				plan.configName,
				path.basename(model.module),
			],
			proof.timeoutMilliseconds,
			label,
			inputDirectory,
			logPath,
			recordFailure,
			signal,
		);
		const report = {
			...parseStateReport(childResult.output),
			durationMilliseconds: Date.now() - start,
			model: model.id,
			modelConfigSha256: proof.modelConfigSha256,
			variant: variant.name,
			workers: proof.workers,
			heapMiB: proof.heapMiB,
			logPath,
		};
		let error = childResult.error;
		if ((records || cache) && !error) {
			try {
				assertCompleteTlcOutput(childResult.output);
			} catch (validationError) {
				error = validationError;
				recordFailure(error);
			}
		}
		if (records) {
			await records.commitConfiguration(
				plan,
				{
					report,
					status: error ? 'failed' : 'passed',
					message: error?.message ?? null,
					log: 'logs/' + log,
					inputs,
				},
				signal,
			);
			committed = true;
		}
		if (error) {
			await writeTlcOutput(process.stdout, label, childResult.output);
			process.stdout.write(
				'TLA_FAILURE ' +
					JSON.stringify({
						...report,
						status: childResult.timedOut ? 'timeout' : 'failed',
						exitCode: childResult.code,
						signal: childResult.signal,
						message: error.message,
					}) +
					'\n',
			);
			throw error;
		}
		if (cache)
			outcomes.set(JSON.stringify([model.id, variant.name]), {
				bytes: childResult.bytes,
				durationMilliseconds: report.durationMilliseconds,
			});
		await writeTlcOutput(process.stdout, label, childResult.output, report);
		return childResult.output;
	} catch (error) {
		recordFailure(error);
		throw error;
	} finally {
		await rm(temporaryDirectory, { force: true, recursive: true });
		if (records && !committed) await rm(inputDirectory, { force: true, recursive: true });
	}
}

const selection = parseArguments(process.argv.slice(2));
const maximumHeapMiB = modelMaximumHeapMiB(selection.maximumHeapMiB);
const memoryBytes = Math.min(totalmem(), process.constrainedMemory() || Infinity);
const processors = availableParallelism();
const concurrency = modelConcurrency(selection.jobs, processors, memoryBytes, maximumHeapMiB);
const models = await selectModels(selection);
if (models.length === 0) {
	if (selection.cacheDirectory) {
		const maintenance = await ModelSuccessCache.acquire(
			selection.cacheDirectory,
			repositoryRoot,
			inventory.models,
		);
		await maintenance.close();
	}
	process.stdout.write('No TLA+ model is affected by the selected paths.\n');
	process.exitCode = 0;
} else {
	const plannedModels = selection.runDirectory ? inventory.models : models;
	const slots = Math.min(
		concurrency,
		plannedModels.reduce((count, model) => count + (model.variants?.length ?? 1), 0),
	);
	const heapMiB = modelHeapMiB(slots, memoryBytes, maximumHeapMiB);
	const plans = await executionPlans(plannedModels, repositoryRoot, heapMiB, configuredTimeout);
	await provisionJar(inventory.tool, jarPath, !configuredJarPath);
	const identity = selection.runDirectory
		? await frozenContext(slots, processors, heapMiB, maximumHeapMiB)
		: undefined;
	const cacheIdentity = selection.cacheDirectory
		? await cacheRuntimeIdentity(repositoryRoot, javaExecutable, jarPath, { heapMiB })
		: undefined;
	if (cacheIdentity) javaExecutable = cacheIdentity.javaExecutable;
	let records;
	let cache;
	const outcomes = new Map();
	const validateCacheIdentity = async () => {
		const currentInventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));
		const currentModels = currentInventory.models.filter((model) =>
			models.some((selected) => selected.id === model.id),
		);
		const currentPlans = await executionPlans(currentModels, repositoryRoot, heapMiB, (model) =>
			configuredTimeout(model, currentInventory.defaultTimeoutMilliseconds),
		);
		const currentRuntime = await cacheRuntimeIdentity(repositoryRoot, javaExecutable, jarPath, {
			heapMiB,
		});
		if (
			currentInventory.tool.sha256 !== cacheIdentity.jarSha256 ||
			!isDeepStrictEqual(currentRuntime, cacheIdentity) ||
			!isDeepStrictEqual(
				currentPlans.map((plan) => configurationIdentity(plan, currentRuntime)),
				plans.map((plan) => configurationIdentity(plan, cacheIdentity)),
			)
		)
			throw new Error('TLC cache execution identity changed before completion');
	};
	const controller = new AbortController();
	const cancel = () => controller.abort(new Error('TLC checking cancelled'));
	process.on('SIGINT', cancel);
	process.on('SIGTERM', cancel);
	const counters = { fresh: 0, reused: 0 };
	let executionError;
	let integrityFailure;
	let configurationCount;
	try {
		if (selection.cacheDirectory) {
			await assertInputAdmission([
				...inventoryInputPaths({ models }),
				...Object.keys(cacheIdentity.helpers).map((name) => `scripts/tla/${name}`),
			]);
			cache = await ModelSuccessCache.acquire(
				selection.cacheDirectory,
				repositoryRoot,
				inventory.models,
			);
			const misses = await cache.prepare(plans, cacheIdentity, selection.all);
			process.stdout.write(
				'TLA_CACHE_PLAN ' +
					JSON.stringify({
						configurations: plans.length,
						hits: plans.length - misses.length,
						misses: misses.slice(0, 20),
						omittedMisses: Math.max(0, misses.length - 20),
						forced: selection.all,
					}) +
					'\n',
			);
		}
		if (selection.runDirectory) {
			records = await ModelRunRecords.acquire(
				selection.runDirectory,
				repositoryRoot,
				identity,
				plans,
			);
		}
		if (controller.signal.aborted) throw controller.signal.reason;
		await records?.request(models);
		const logsDirectory = records
			? path.join(records.root, 'logs')
			: await mkdtemp(path.join(tmpdir(), 'orchbee-tla-logs-'));
		process.stdout.write(
			'Checking ' +
				models.length +
				' models with at most ' +
				slots +
				' concurrent TLC processes (' +
				heapMiB +
				' MiB heap each), declared CPU-worker budget ' +
				processors +
				'.\nLogs: ' +
				logsDirectory +
				'\n',
		);
		configurationCount = await runModelChecks(
			models,
			slots,
			(model, variant, recordFailure) =>
				runVariant(
					plans.find((plan) => plan.model.id === model.id && plan.variant.name === variant.name),
					recordFailure,
					{ logsDirectory, records, cache, outcomes, signal: controller.signal, counters },
				),
			(model, variant) =>
				process.stdout.write(
					'TLA_SKIPPED ' +
						JSON.stringify({ model: model.id, variant: variant.name, reason: 'earlier-failure' }) +
						'\n',
				),
			{
				processors,
				signal: controller.signal,
				previousModelFailures: records?.previousModelFailures(),
				onModelOutcome: async (model, outcome) => {
					await records?.commitModel(model, outcome, controller.signal);
					if (cache && outcome.status === 'passed')
						await cache.commitModel(
							plans.filter((plan) => plan.model.id === model.id),
							cacheIdentity,
							outcomes,
							validateCacheIdentity,
							controller.signal,
						);
				},
			},
		);
	} catch (error) {
		executionError = error;
	} finally {
		if (cache) {
			try {
				await validateCacheIdentity();
			} catch (error) {
				integrityFailure = error;
				executionError ??= error;
			}
		}
		if (records) {
			try {
				const current = await frozenContext(slots, processors, heapMiB, maximumHeapMiB);
				const currentPlans = await executionPlans(
					plannedModels,
					repositoryRoot,
					heapMiB,
					configuredTimeout,
				);
				if (
					!isDeepStrictEqual(current, identity) ||
					!isDeepStrictEqual(
						currentPlans.map((plan) => plan.proof),
						plans.map((plan) => plan.proof),
					)
				) {
					integrityFailure = new Error('Frozen TLC execution identity changed before completion');
					executionError ??= integrityFailure;
				}
			} catch (error) {
				integrityFailure = error;
				executionError ??= error;
			}
		}
		try {
			await records?.close();
			await cache?.close();
		} catch (error) {
			executionError ??= error;
		}
		if (controller.signal.aborted) executionError ??= controller.signal.reason;
		process.removeListener('SIGINT', cancel);
		process.removeListener('SIGTERM', cancel);
		if (records) {
			process.stdout.write(
				'TLA_RUN_RESULT ' +
					JSON.stringify({
						identity: records.manifest.identity,
						freshConfigurations: counters.fresh,
						reusedConfigurations: counters.reused,
						...records.summary(),
						...(integrityFailure
							? { complete: false, integrityFailure: integrityFailure.message }
							: {}),
						...(controller.signal.aborted ? { complete: false, cancelled: true } : {}),
					}) +
					'\n',
			);
		}
		if (cache)
			process.stdout.write(
				'TLA_CACHE_RESULT ' +
					JSON.stringify({
						freshConfigurations: counters.fresh,
						reusedConfigurations: counters.reused,
						complete: !executionError && !controller.signal.aborted,
						...(integrityFailure ? { integrityFailure: integrityFailure.message } : {}),
					}) +
					'\n',
			);
	}
	if (executionError) throw executionError;
	process.stdout.write(
		'\nChecked ' +
			configurationCount +
			' TLA+ model configurations with required action coverage.\n',
	);
}
