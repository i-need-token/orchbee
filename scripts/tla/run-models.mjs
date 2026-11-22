// Runs registered TLA+ models with the pinned TLC, then their mutation controls. No gate runs this
// by default: `pnpm verify:changed` calls it only when a model or this tooling changed.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { availableParallelism, tmpdir, totalmem } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { provisionJar } from './toolchain.mjs';

export const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
export const inventoryPath = path.join(repositoryRoot, 'scripts/tla/models.json');
const mutationSuite = 'scripts/tla/model-mutations.test.mjs';
const mebibyte = 1024 ** 2;
const maximumHeapMiB = 4096;
const processReserveMiB = 512;

export function applyConfigConstants(config, constants) {
	let updated = config;
	for (const [name, value] of Object.entries(constants)) {
		if (!/^[A-Za-z][A-Za-z\d_]*$/u.test(name)) throw new Error(`Invalid TLA+ constant: ${name}`);
		let serialized = String(value);
		if (typeof value === 'string') serialized = JSON.stringify(value);
		if (typeof value === 'boolean') serialized = value ? 'TRUE' : 'FALSE';
		const expression = new RegExp(`^(\\s*CONSTANT\\s+${name}\\s*=\\s*).*$`, 'gmu');
		if ([...updated.matchAll(expression)].length !== 1)
			throw new Error(`Expected exactly one assignment for TLA+ constant ${name}`);
		updated = updated.replace(expression, `$1${serialized}`);
	}
	return updated;
}

export async function tlcJar(inventory) {
	const configured = process.env.ORCHBEE_TLA2TOOLS_JAR;
	const jar = configured
		? path.resolve(configured)
		: path.join(repositoryRoot, '.tools', `tla2tools-${inventory.tool.version}.jar`);
	await provisionJar(inventory.tool, jar, configured === undefined);
	return jar;
}

function parseArguments(commandArguments) {
	const result = { selector: 'all', value: undefined, jobs: undefined };
	const args = commandArguments.filter((argument) => argument !== '--');
	for (let index = 0; index < args.length; index += 1) {
		const argument = args[index];
		if (argument === '--all') continue;
		const value = args[index + 1];
		if (!['--models', '--workspace', '--changed-since', '--jobs'].includes(argument))
			throw new Error(`Unknown TLA+ runner argument: ${argument}`);
		if (value === undefined || value.startsWith('--'))
			throw new Error(`${argument} requires a value`);
		index += 1;
		if (argument === '--jobs') {
			if (!/^[1-9]\d*$/u.test(value)) throw new Error('--jobs must be a positive integer');
			result.jobs = Number(value);
		} else {
			if (result.selector !== 'all')
				throw new Error('Select models with one of --models, --workspace or --changed-since');
			result.selector = argument.slice(2);
			result.value = value;
		}
	}
	return result;
}

function git(args) {
	return execFileSync('git', args, { cwd: repositoryRoot, encoding: 'utf8' });
}

// A model runs when one of its own files changed. A change to this tooling runs every model.
function changedModels(models, reference) {
	const base = git(['merge-base', reference, 'HEAD']).trim();
	const changed = new Set(
		[
			...git(['diff', '--name-only', '--no-renames', '-z', base]).split('\0'),
			...git(['ls-files', '--others', '--exclude-standard', '-z']).split('\0'),
		].filter(Boolean),
	);
	if ([...changed].some((file) => file.startsWith('scripts/tla/'))) return models;
	const selected = models.filter((model) =>
		[model.module, model.config, ...(model.supportingModules ?? [])].some((file) =>
			changed.has(file),
		),
	);
	const registered = new Set(
		models.flatMap((model) => [model.module, model.config, ...(model.supportingModules ?? [])]),
	);
	for (const file of changed) {
		if (
			/(?:^|\/)formal\/tla\//u.test(file) &&
			!registered.has(file) &&
			existsSync(path.join(repositoryRoot, file))
		)
			process.stdout.write(`Not registered in models.json, not run: ${file}\n`);
	}
	return selected;
}

function selectModels(inventory, selection) {
	const { models } = inventory;
	if (selection.selector === 'all') return models;
	if (selection.selector === 'changed-since') return changedModels(models, selection.value);
	if (selection.selector === 'workspace') {
		const selected = models.filter((model) => model.module.startsWith(`${selection.value}/`));
		if (selected.length === 0) throw new Error(`No TLA+ models belong to ${selection.value}`);
		return selected;
	}
	const requested = selection.value.split(',').filter(Boolean);
	const unknown = requested.filter((id) => !models.some((model) => model.id === id));
	if (unknown.length > 0) throw new Error(`Unknown TLA+ model ids: ${unknown.join(', ')}`);
	return models.filter((model) => requested.includes(model.id));
}

function count(text) {
	return text === undefined ? undefined : Number.parseInt(text.replaceAll(',', ''), 10);
}

// Pass only on positive evidence: a finished search with no error and an empty queue.
function completeSearch(output) {
	const states = [
		...output.matchAll(
			/([\d,]+) states generated, ([\d,]+) distinct states found, ([\d,]+) states left on queue/gu,
		),
	].at(-1);
	const depth = output.match(/depth of the complete state graph search is ([\d,]+)/iu);
	const complete =
		/^Model checking completed\. No error has been found\.$/mu.test(output) &&
		/^Finished in .+$/mu.test(output) &&
		!/^Error:/mu.test(output) &&
		states !== undefined &&
		count(states[3]) === 0 &&
		count(states[2]) > 0;
	return complete ? { distinct: count(states[2]), depth: count(depth?.[1]) } : undefined;
}

// Pinned TLC prints `<Action ...>: distinct:generated` per action; a reached action has generated
// at least one state in some configuration of the model.
function unreachedActions(outputs, required) {
	const reached = new Set();
	for (const match of outputs
		.join('\n')
		.matchAll(/^\s*<([^\s>]+)\b[^>]*>:\s*([\d,]+)(?::([\d,]+))?\s*$/gmu)) {
		if (count(match[3] ?? match[2]) > 0) reached.add(match[1]);
	}
	return required.filter((action) => !reached.has(action));
}

function runTlc(args, cwd, logPath, timeout) {
	return new Promise((resolve) => {
		const child = spawn(process.env.ORCHBEE_TLA_JAVA ?? 'java', args, { cwd });
		const chunks = [];
		child.stdout.on('data', (chunk) => chunks.push(chunk));
		child.stderr.on('data', (chunk) => chunks.push(chunk));
		let timedOut = false;
		const timer = setTimeout(() => {
			timedOut = true;
			child.kill('SIGKILL');
		}, timeout);
		let settled = false;
		const settle = async (code, launchError) => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			const output = Buffer.concat(chunks).toString('utf8');
			await writeFile(logPath, output);
			resolve({ code, output, timedOut, launchError });
		};
		child.once('error', (error) => settle(null, error));
		child.once('close', (code) => settle(code, undefined));
	});
}

async function runConfiguration(job, jar, heapMiB, logs, timeoutDefault) {
	const { model, variant } = job;
	const label = variant ? `${model.id}/${variant.name}` : model.id;
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-'));
	try {
		for (const source of [model.module, ...(model.supportingModules ?? [])])
			await writeFile(
				path.join(directory, path.basename(source)),
				await readFile(path.join(repositoryRoot, source)),
			);
		let config = await readFile(path.join(repositoryRoot, model.config), 'utf8');
		let configName = path.basename(model.config);
		if (variant) {
			config = applyConfigConstants(config, variant.constants);
			configName = `${path.basename(model.config, '.cfg')}-${variant.name}.cfg`;
		}
		await writeFile(path.join(directory, configName), config);
		await mkdir(path.join(directory, 'jvm'));
		const timeout =
			Number(process.env.ORCHBEE_TLA_TIMEOUT_MILLISECONDS) ||
			model.timeoutMilliseconds ||
			timeoutDefault;
		const logPath = path.join(logs, `${label.replace('/', '--')}.log`);
		const started = Date.now();
		const result = await runTlc(
			[
				`-Xmx${heapMiB}m`,
				'-XX:+UseParallelGC',
				`-Djava.io.tmpdir=${path.join(directory, 'jvm')}`,
				'-jar',
				jar,
				'-fp',
				'0',
				'-lncheck',
				'final',
				'-coverage',
				'1',
				'-workers',
				String(model.workers ?? 1),
				'-metadir',
				path.join(directory, 'states'),
				'-config',
				configName,
				path.basename(model.module),
			],
			directory,
			logPath,
			timeout,
		);
		const seconds = ((Date.now() - started) / 1000).toFixed(1);
		const search = result.code === 0 ? completeSearch(result.output) : undefined;
		let failure;
		if (result.launchError) failure = `could not start Java: ${result.launchError.message}`;
		else if (result.timedOut) failure = `exceeded its ${timeout} ms deadline`;
		else if (!search) failure = `did not finish a clean search (exit ${result.code})`;
		if (failure) {
			const tail = result.output.trimEnd().split('\n').slice(-40).join('\n');
			process.stdout.write(`\nFAIL ${label}: ${failure}\n${tail}\nLog: ${logPath}\n`);
			return { label, output: result.output, failed: true };
		}
		process.stdout.write(
			`PASS ${label}: ${search.distinct} distinct states, depth ${search.depth}, ${seconds} s\n`,
		);
		return { label, output: result.output, failed: false };
	} finally {
		await rm(directory, { recursive: true, force: true });
	}
}

async function runModels(models, jar, jobs, timeoutDefault) {
	const memory = Math.min(totalmem(), process.constrainedMemory() || Infinity);
	const processors = availableParallelism();
	const configurations = models.flatMap((model) =>
		(model.variants ?? [undefined]).map((variant) => ({ model, variant })),
	);
	const slots = Math.min(
		configurations.length,
		jobs ??
			Math.max(
				1,
				Math.min(
					processors,
					Math.floor((memory * 0.6) / ((maximumHeapMiB + processReserveMiB) * mebibyte)),
				),
			),
	);
	const heapMiB = Math.min(
		maximumHeapMiB,
		Math.floor((memory * 0.6) / slots / mebibyte) - processReserveMiB,
	);
	if (heapMiB < 256) throw new Error(`${slots} TLC processes do not fit in memory`);
	const logs = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-logs-'));
	process.stdout.write(
		`Checking ${configurations.length} configurations of ${models.length} models, ` +
			`${slots} at a time (${heapMiB} MiB heap each). Logs: ${logs}\n`,
	);
	const results = new Map();
	let next = 0;
	let activeWorkers = 0;
	let stop = false;
	const waiting = new Set();
	const wake = () => {
		for (const resolve of waiting) resolve();
		waiting.clear();
	};
	// Summed TLC workers stay within the processors; a larger model runs alone.
	async function slot() {
		while (!stop && next < configurations.length) {
			const job = configurations[next];
			const weight = job.model.workers ?? 1;
			if (activeWorkers > 0 && activeWorkers + weight > processors) {
				await new Promise((resolve) => waiting.add(resolve));
				continue;
			}
			next += 1;
			activeWorkers += weight;
			try {
				const result = await runConfiguration(job, jar, heapMiB, logs, timeoutDefault);
				results.set(job, result);
				if (result.failed) stop = true;
			} finally {
				activeWorkers -= weight;
				wake();
			}
		}
	}
	await Promise.all(Array.from({ length: slots }, slot));
	if (stop) return false;
	let covered = true;
	for (const model of models) {
		const outputs = configurations
			.filter((job) => job.model === model)
			.map((job) => results.get(job).output);
		const unreached = unreachedActions(outputs, model.requiredCoverage);
		if (unreached.length > 0) {
			covered = false;
			process.stdout.write(`FAIL ${model.id}: actions never taken: ${unreached.join(', ')}\n`);
		}
	}
	await rm(logs, { recursive: true, force: true });
	return covered;
}

function runMutations(models) {
	// Mutation tests are named `<model id>: <fault>` inside one suite.
	const names = models.map((model) => model.id).join('|');
	return new Promise((resolve) => {
		const child = spawn(
			process.execPath,
			[
				'--test',
				'--test-reporter=spec',
				`--test-name-pattern=(?:^| )(?:${names}): `,
				mutationSuite,
			],
			{ cwd: repositoryRoot, stdio: 'inherit' },
		);
		child.once('close', (code) => resolve(code === 0));
	});
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	try {
		const selection = parseArguments(process.argv.slice(2));
		const inventory = JSON.parse(await readFile(inventoryPath, 'utf8'));
		const models = selectModels(inventory, selection);
		if (models.length === 0) {
			process.stdout.write('No TLA+ model changed.\n');
		} else {
			const jar = await tlcJar(inventory);
			const passed =
				(await runModels(models, jar, selection.jobs, inventory.defaultTimeoutMilliseconds)) &&
				(await runMutations(models));
			if (passed) process.stdout.write(`\nChecked ${models.length} TLA+ models.\n`);
			process.exitCode = passed ? 0 : 1;
		}
	} catch (error) {
		process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
		process.exitCode = 1;
	}
}
