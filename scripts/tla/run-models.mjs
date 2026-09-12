import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, open, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { availableParallelism, tmpdir, totalmem } from 'node:os';
import path from 'node:path';

import {
	modelConcurrency,
	modelHeapMiB,
	modelWorkers,
	runModelChecks,
	writeTlcOutput,
} from './model-checks.mjs';
import {
	applyConfigConstants,
	inventoryPath,
	parseStateReport,
	repositoryRoot,
	selectModelsSince,
	tlcExitError,
	validateInventory,
	variantConfigFileName,
} from './model-inventory.mjs';

const inventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));
const toolsDirectory = path.join(repositoryRoot, '.tools');
const bundledJarPath = path.join(toolsDirectory, `tla2tools-${inventory.tool.version}.jar`);
const configuredJarPath = process.env.ORCHBEE_TLA2TOOLS_JAR;
const jarPath = configuredJarPath ? path.resolve(configuredJarPath) : bundledJarPath;
const javaExecutable = process.env.ORCHBEE_TLA_JAVA ?? 'java';

function parseArguments(commandArguments) {
	const result = {
		all: false,
		changedSince: undefined,
		jobs: undefined,
		models: [],
		workspace: undefined,
	};
	for (let index = 0; index < commandArguments.length; index += 1) {
		const argument = commandArguments[index];
		if (argument === '--all') {
			result.all = true;
			continue;
		}
		if (['--changed-since', '--models', '--workspace', '--jobs'].includes(argument)) {
			const value = commandArguments[index + 1];
			if (value === undefined || value.startsWith('--'))
				throw new Error(`${argument} requires a value`);
			index += 1;
			if (argument === '--changed-since') result.changedSince = value;
			if (argument === '--models') result.models.push(...value.split(',').filter(Boolean));
			if (argument === '--workspace') result.workspace = value;
			if (argument === '--jobs') result.jobs = value;
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
	return result;
}

async function sha256(filePath) {
	return createHash('sha256')
		.update(await readFile(filePath))
		.digest('hex');
}

async function jarIsValid(filePath) {
	try {
		return (await sha256(filePath)) === inventory.tool.sha256;
	} catch (error) {
		if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
			return false;
		}
		throw error;
	}
}

async function provisionJar() {
	if (await jarIsValid(jarPath)) return;
	if (configuredJarPath) {
		throw new Error(
			`ORCHBEE_TLA2TOOLS_JAR does not match pinned TLA+ ${inventory.tool.version} SHA-256`,
		);
	}
	await mkdir(toolsDirectory, { recursive: true });
	const response = await fetch(inventory.tool.downloadUrl);
	if (!response.ok) {
		throw new Error(`Could not download ${inventory.tool.downloadUrl}: HTTP ${response.status}`);
	}
	const temporaryPath = `${jarPath}.download`;
	await writeFile(temporaryPath, new Uint8Array(await response.arrayBuffer()));
	if (!(await jarIsValid(temporaryPath))) {
		await rm(temporaryPath, { force: true });
		throw new Error(
			`Downloaded TLA+ ${inventory.tool.version} jar failed its pinned SHA-256 check`,
		);
	}
	await rename(temporaryPath, jarPath);
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

function configuredTimeout(model) {
	const override = process.env.ORCHBEE_TLA_TIMEOUT_MILLISECONDS;
	const timeout =
		override === undefined
			? (model.timeoutMilliseconds ?? inventory.defaultTimeoutMilliseconds)
			: Number(override);
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
) {
	const log = await open(logPath, 'w');
	let result;
	try {
		result = await new Promise((resolve) => {
			const child = spawn(javaExecutable, arguments_, {
				cwd: workingDirectory,
				stdio: ['ignore', log.fd, log.fd],
			});
			let launchError;
			let processError;
			let forceKill;
			let timedOut = false;
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
			child.once('close', (code, signal) => {
				clearTimeout(timeout);
				clearTimeout(forceKill);
				processError ??=
					launchError ?? tlcExitError(modelId, timeoutMilliseconds, { code, signal, timedOut });
				if (processError !== undefined) recordFailure(processError);
				resolve({ code, error: processError, signal, timedOut });
			});
		});
	} finally {
		await log.close();
	}
	const output = await readFile(logPath, 'utf8');
	return { ...result, output };
}

async function runVariant(model, variant, recordFailure, { logsDirectory, heapMiB }) {
	const modulePath = path.join(repositoryRoot, model.module);
	const sourceModulePaths = [model.module, ...(model.supportingModules ?? [])];
	const configuredConfigPath = path.join(repositoryRoot, model.config);
	await mkdir(toolsDirectory, { recursive: true });
	const temporaryDirectory = await mkdtemp(path.join(toolsDirectory, `tla-${model.id}-`));
	const metadataDirectory = path.join(temporaryDirectory, 'states');
	const jvmTemporaryDirectory = path.join(temporaryDirectory, 'jvm-temp');
	await mkdir(metadataDirectory);
	await mkdir(jvmTemporaryDirectory);
	let configPath = configuredConfigPath;
	let configArgument = configPath;
	let moduleArgument = modulePath;
	let workingDirectory = repositoryRoot;
	try {
		if (variant.constants !== undefined) {
			configPath = path.join(temporaryDirectory, variantConfigFileName(model.config, variant.name));
			const config = await readFile(configuredConfigPath, 'utf8');
			await writeFile(configPath, applyConfigConstants(config, variant.constants));
			await Promise.all(
				sourceModulePaths.map(async (sourceModulePath) => {
					await writeFile(
						path.join(temporaryDirectory, path.basename(sourceModulePath)),
						await readFile(path.join(repositoryRoot, sourceModulePath)),
					);
				}),
			);
			const temporaryModulePath = path.join(temporaryDirectory, path.basename(model.module));
			configArgument = path.basename(configPath);
			moduleArgument = path.basename(temporaryModulePath);
			workingDirectory = temporaryDirectory;
		}
		const label = variant.name === 'default' ? model.id : `${model.id}/${variant.name}`;
		const logPath = path.join(logsDirectory, `${model.id}--${variant.name}.log`);
		const timeoutMilliseconds = configuredTimeout(model);
		const workers = modelWorkers(model);
		const digestBuilder = createHash('sha256');
		for (const sourceModulePath of sourceModulePaths) {
			digestBuilder
				.update(sourceModulePath)
				.update('\0')
				.update(await readFile(path.join(repositoryRoot, sourceModulePath)))
				.update('\0');
		}
		const digest = digestBuilder
			.update(model.config)
			.update('\0')
			.update(await readFile(configPath))
			.digest('hex');
		process.stdout.write(
			`\nChecking ${label} (timeout ${timeoutMilliseconds}ms, workers ${workers})\nLog: ${logPath}\n`,
		);
		const start = Date.now();
		const childResult = await runChild(
			[
				`-Xmx${heapMiB}m`,
				'-XX:+UseParallelGC',
				`-Djava.io.tmpdir=${jvmTemporaryDirectory}`,
				'-jar',
				jarPath,
				'-fp',
				'0',
				'-lncheck',
				'final',
				'-coverage',
				'1',
				'-workers',
				String(workers),
				'-metadir',
				metadataDirectory,
				'-config',
				configArgument,
				moduleArgument,
			],
			timeoutMilliseconds,
			label,
			workingDirectory,
			logPath,
			recordFailure,
		);
		const report = {
			...parseStateReport(childResult.output),
			durationMilliseconds: Date.now() - start,
			model: model.id,
			modelConfigSha256: digest,
			variant: variant.name,
			workers,
			heapMiB,
			logPath,
		};
		if (childResult.error !== undefined) {
			await writeTlcOutput(process.stdout, label, childResult.output);
			process.stdout.write(
				`TLA_FAILURE ${JSON.stringify({
					...report,
					status: childResult.timedOut ? 'timeout' : 'failed',
					exitCode: childResult.code,
					signal: childResult.signal,
					message: childResult.error.message,
				})}\n`,
			);
			throw childResult.error;
		}
		await writeTlcOutput(process.stdout, label, childResult.output, report);
		return childResult.output;
	} catch (error) {
		recordFailure(error);
		throw error;
	} finally {
		await rm(temporaryDirectory, { force: true, recursive: true });
	}
}

const selection = parseArguments(process.argv.slice(2));
const memoryBytes = Math.min(totalmem(), process.constrainedMemory() || Infinity);
const concurrency = modelConcurrency(selection.jobs, availableParallelism(), memoryBytes);
const models = await selectModels(selection);
if (models.length === 0) {
	process.stdout.write('No TLA+ model is affected by the selected paths.\n');
	process.exitCode = 0;
} else {
	await provisionJar();
	const logsDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-logs-'));
	const slots = Math.min(
		concurrency,
		models.reduce((count, model) => count + (model.variants?.length ?? 1), 0),
	);
	const heapMiB = modelHeapMiB(slots, memoryBytes);
	process.stdout.write(
		`Checking ${models.length} models with at most ${slots} concurrent TLC processes (${heapMiB} MiB heap each).\nLogs: ${logsDirectory}\n`,
	);
	const configurationCount = await runModelChecks(
		models,
		slots,
		(model, variant, recordFailure) =>
			runVariant(model, variant, recordFailure, { logsDirectory, heapMiB }),
		(model, variant) => {
			process.stdout.write(
				`TLA_SKIPPED ${JSON.stringify({ model: model.id, variant: variant.name, reason: 'earlier-failure' })}\n`,
			);
		},
	);
	process.stdout.write(
		`\nChecked ${configurationCount} TLA+ model configurations with required action coverage.\n`,
	);
}
