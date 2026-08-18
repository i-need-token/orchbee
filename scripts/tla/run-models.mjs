import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

import {
	applyConfigConstants,
	assertRequiredCoverage,
	changedPathsSince,
	inventoryPath,
	parseStateReport,
	repositoryRoot,
	selectModelsForChangedPaths,
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
	const result = { all: false, changedSince: undefined, models: [], workspace: undefined };
	for (let index = 0; index < commandArguments.length; index += 1) {
		const argument = commandArguments[index];
		if (argument === '--all') {
			result.all = true;
			continue;
		}
		if (argument === '--changed-since' || argument === '--models' || argument === '--workspace') {
			const value = commandArguments[index + 1];
			if (value === undefined || value.startsWith('--'))
				throw new Error(`${argument} requires a value`);
			index += 1;
			if (argument === '--changed-since') result.changedSince = value;
			if (argument === '--models') result.models.push(...value.split(',').filter(Boolean));
			if (argument === '--workspace') result.workspace = value;
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
		return selectModelsForChangedPaths(inventory, await changedPathsSince(selection.changedSince));
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

function runChild(arguments_, timeoutMilliseconds, modelId, workingDirectory) {
	return new Promise((resolve, reject) => {
		const child = spawn(javaExecutable, arguments_, {
			cwd: workingDirectory,
			stdio: ['ignore', 'pipe', 'pipe'],
		});
		let output = '';
		let settled = false;
		let forceKill;
		let timedOut = false;
		const timeout = setTimeout(() => {
			timedOut = true;
			child.kill('SIGTERM');
			forceKill = setTimeout(() => child.kill('SIGKILL'), 5_000);
		}, timeoutMilliseconds);
		child.stdout.on('data', (chunk) => {
			output += chunk.toString();
			process.stdout.write(chunk);
		});
		child.stderr.on('data', (chunk) => {
			output += chunk.toString();
			process.stderr.write(chunk);
		});
		child.once('error', (error) => {
			if (settled) return;
			settled = true;
			clearTimeout(timeout);
			clearTimeout(forceKill);
			reject(error);
		});
		child.once('exit', (code, signal) => {
			if (settled) return;
			settled = true;
			clearTimeout(timeout);
			clearTimeout(forceKill);
			const error = tlcExitError(modelId, timeoutMilliseconds, { code, signal, timedOut });
			if (error !== undefined) {
				reject(error);
				return;
			}
			resolve(output);
		});
	});
}

async function runVariant(model, variant) {
	const modulePath = path.join(repositoryRoot, model.module);
	const configuredConfigPath = path.join(repositoryRoot, model.config);
	await mkdir(toolsDirectory, { recursive: true });
	const temporaryDirectory = await mkdtemp(path.join(toolsDirectory, `tla-${model.id}-`));
	const metadataDirectory = path.join(temporaryDirectory, 'states');
	await mkdir(metadataDirectory);
	let configPath = configuredConfigPath;
	let configArgument = configPath;
	let moduleArgument = modulePath;
	let workingDirectory = repositoryRoot;
	try {
		if (variant.constants !== undefined) {
			configPath = path.join(temporaryDirectory, variantConfigFileName(model.config, variant.name));
			const config = await readFile(configuredConfigPath, 'utf8');
			await writeFile(configPath, applyConfigConstants(config, variant.constants));
			const temporaryModulePath = path.join(temporaryDirectory, path.basename(model.module));
			await writeFile(temporaryModulePath, await readFile(modulePath));
			configArgument = path.basename(configPath);
			moduleArgument = path.basename(temporaryModulePath);
			workingDirectory = temporaryDirectory;
		}
		const label = variant.name === 'default' ? model.id : `${model.id}/${variant.name}`;
		const timeoutMilliseconds = configuredTimeout(model);
		process.stdout.write(`\nChecking ${label} (timeout ${timeoutMilliseconds}ms)\n`);
		const start = Date.now();
		const output = await runChild(
			[
				'-XX:+UseParallelGC',
				'-jar',
				jarPath,
				'-fp',
				'0',
				'-coverage',
				'1',
				'-workers',
				'1',
				'-metadir',
				metadataDirectory,
				'-config',
				configArgument,
				moduleArgument,
			],
			timeoutMilliseconds,
			label,
			workingDirectory,
		);
		const digest = createHash('sha256')
			.update(await readFile(modulePath))
			.update(await readFile(configPath))
			.digest('hex');
		const report = {
			...parseStateReport(output),
			durationMilliseconds: Date.now() - start,
			model: model.id,
			modelConfigSha256: digest,
			variant: variant.name,
		};
		process.stdout.write(`TLA_RESULT ${JSON.stringify(report)}\n`);
		return output;
	} finally {
		await rm(temporaryDirectory, { force: true, recursive: true });
	}
}

const selection = parseArguments(process.argv.slice(2));
const models = await selectModels(selection);
if (models.length === 0) {
	process.stdout.write('No TLA+ model is affected by the selected paths.\n');
	process.exitCode = 0;
} else {
	await provisionJar();
	let configurationCount = 0;
	for (const model of models) {
		const outputs = [];
		for (const variant of model.variants ?? [{ name: 'default' }]) {
			outputs.push(await runVariant(model, variant));
			configurationCount += 1;
		}
		assertRequiredCoverage(model.id, outputs, model.requiredCoverage);
	}
	process.stdout.write(
		`\nChecked ${configurationCount} TLA+ model configurations with required action coverage.\n`,
	);
}
