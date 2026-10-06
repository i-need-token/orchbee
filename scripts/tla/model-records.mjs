import { randomUUID } from 'node:crypto';
import {
	lstat,
	mkdir,
	open,
	readFile,
	readdir,
	realpath,
	rename,
	rm,
	writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';

import { HistoricalModelFailure } from './model-checks.mjs';
import { assertCompleteTlcOutput, sha256Bytes } from './model-inputs.mjs';
import { assertRequiredCoverage, parseStateReport } from './model-inventory.mjs';

function exactKeys(value, keys, label) {
	if (
		!value ||
		typeof value !== 'object' ||
		Array.isArray(value) ||
		Object.keys(value).length !== keys.length ||
		keys.some((field) => !Object.hasOwn(value, field))
	)
		throw new Error(`Invalid ${label} schema`);
}

async function optionalJson(file) {
	try {
		return JSON.parse(await readFile(file, 'utf8'));
	} catch (error) {
		if (error.code === 'ENOENT') return undefined;
		throw error;
	}
}

function inside(parent, child) {
	const relative = path.relative(parent, child);
	return (
		relative === '' ||
		(!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))
	);
}

/** Check every existing component before creating anything or accepting a run lease. */
async function rejectLinks(target) {
	let current = path.parse(target).root;
	for (const part of path.relative(current, target).split(path.sep).filter(Boolean)) {
		current = path.join(current, part);
		try {
			if ((await lstat(current)).isSymbolicLink())
				throw new Error(`TLC run paths cannot contain links: ${current}`);
		} catch (error) {
			if (error.code === 'ENOENT') return;
			throw error;
		}
	}
}

async function atomicJson(file, value, { replace = false, signal } = {}) {
	const temporary = `${file}.${randomUUID()}.tmp`;
	try {
		await writeFile(temporary, `${JSON.stringify(value)}\n`, { flag: 'wx' });
		if (!replace && (await optionalJson(file)) !== undefined)
			throw new Error(`Refusing to overwrite committed TLC record: ${file}`);
		signal?.throwIfAborted();
		await rename(temporary, file);
	} finally {
		await rm(temporary, { force: true });
	}
}

function key(model, variant) {
	return `${model}--${variant}`;
}

/** Task-owned evidence; there is no discovery, retry, cross-run cache or lock takeover. */
export class ModelRunRecords {
	constructor(root, manifest, lease) {
		this.root = root;
		this.manifest = manifest;
		this.lease = lease;
		this.configurations = new Map();
		this.models = new Map();
		this.requested = [];
	}

	static async acquire(directory, repository, context, plans) {
		const root = path.resolve(directory);
		if (inside(repository, root) || inside(root, repository) || root === path.parse(root).root)
			throw new Error(
				'--run-dir must be an external task directory outside the checkout and its ancestors',
			);
		await rejectLinks(root);
		await mkdir(root, { recursive: true });
		if ((await realpath(root)) !== root) throw new Error('--run-dir must use its canonical path');
		const lock = path.join(root, '.tla-run.lock');
		const lease = await open(lock, 'wx').catch((error) => {
			if (error.code === 'EEXIST')
				throw new Error(
					'TLC run has a live or ambiguous writer lease; confirm all writers have stopped before manually removing it',
				);
			throw error;
		});
		let records;
		try {
			await lease.writeFile(JSON.stringify({ pid: process.pid, token: randomUUID() }));
			const file = path.join(root, 'manifest.json');
			await rejectLinks(file);
			const previous = await optionalJson(file);
			if (previous !== undefined) {
				exactKeys(
					previous,
					['schemaVersion', 'identity', 'root', 'runId', 'context', 'plans'],
					'run manifest',
				);
				if (
					typeof previous.runId !== 'string' ||
					!/^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/u.test(previous.runId)
				)
					throw new Error('Invalid TLC run ID');
			}
			const body = {
				root,
				runId: previous?.runId ?? randomUUID(),
				context,
				plans: plans.map((plan) => plan.proof),
			};
			const manifest = { schemaVersion: 1, identity: sha256Bytes(JSON.stringify(body)), ...body };
			if (previous !== undefined && !isDeepStrictEqual(previous, manifest))
				throw new Error('TLC run identity changed; use a new --run-dir');
			if (previous === undefined) {
				if ((await readdir(root)).some((name) => name !== '.tla-run.lock'))
					throw new Error('A new --run-dir must be empty');
				await atomicJson(file, manifest);
			}
			for (const name of ['records', 'models', 'logs', 'inputs', 'attempts']) {
				const child = path.join(root, name);
				await rejectLinks(child);
				await mkdir(child, { recursive: true });
			}
			records = new ModelRunRecords(root, manifest, lease);
			await records.validateCommitted(plans);
			return records;
		} catch (error) {
			await lease.close();
			await rm(lock);
			throw error;
		}
	}

	async file(relative) {
		if (
			typeof relative !== 'string' ||
			!/^[A-Za-z\d_./-]+$/u.test(relative) ||
			relative.split('/').some((part) => !part || part === '.' || part === '..') ||
			path.isAbsolute(relative) ||
			!inside(this.root, path.resolve(this.root, relative))
		)
			throw new Error('Invalid TLC evidence path');
		const file = path.join(this.root, relative);
		await rejectLinks(file);
		return file;
	}

	async validateCommitted(plans) {
		for (const filename of await readdir(path.join(this.root, 'records'))) {
			if (filename.endsWith('.tmp')) continue;
			const plan = plans.find(
				(entry) => filename === `${key(entry.model.id, entry.variant.name)}.json`,
			);
			if (!plan) throw new Error(`Unknown TLC configuration record: ${filename}`);
			const record = await optionalJson(await this.file(`records/${filename}`));
			exactKeys(
				record,
				[
					'schemaVersion',
					'kind',
					'identity',
					'proof',
					'status',
					'message',
					'report',
					'log',
					'logSha256',
					'inputs',
				],
				'configuration record',
			);
			if (
				record.schemaVersion !== 1 ||
				record.kind !== 'configuration' ||
				record.identity !== this.manifest.identity ||
				!isDeepStrictEqual(record.proof, plan.proof) ||
				!['passed', 'failed'].includes(record.status) ||
				(record.message !== null && typeof record.message !== 'string')
			)
				throw new Error(`Invalid TLC configuration proof: ${filename}`);
			if (
				typeof record.log !== 'string' ||
				!record.log.startsWith(`logs/${key(plan.model.id, plan.variant.name)}--`)
			)
				throw new Error('Invalid TLC log identity');
			const bytes = await readFile(await this.file(record.log));
			if (sha256Bytes(bytes) !== record.logSha256) throw new Error(`Corrupt TLC log: ${filename}`);
			const reportFields = [
				'model',
				'variant',
				'modelConfigSha256',
				'workers',
				'heapMiB',
				'durationMilliseconds',
				'logPath',
			];
			if (
				!record.report ||
				typeof record.report !== 'object' ||
				Array.isArray(record.report) ||
				reportFields.some((field) => !Object.hasOwn(record.report, field)) ||
				Object.keys(record.report).some(
					(field) =>
						![...reportFields, 'depth', 'distinctStates', 'generatedStates', 'statesLeft'].includes(
							field,
						),
				)
			)
				throw new Error('Invalid TLC report schema');
			const expectedFiles = [
				...plan.sources.map((source) => ({
					name: path.basename(source.path),
					sha256: sha256Bytes(source.bytes),
				})),
				{ name: plan.configName, sha256: sha256Bytes(plan.config) },
			];
			if (!Array.isArray(record.inputs) || record.inputs.length !== expectedFiles.length)
				throw new Error('Invalid TLC staged input set');
			for (const [index, input] of record.inputs.entries()) {
				exactKeys(input, ['path', 'sha256'], 'staged input');
				if (
					!input.path.startsWith(`inputs/${key(plan.model.id, plan.variant.name)}--`) ||
					path.posix.basename(input.path) !== expectedFiles[index].name ||
					input.sha256 !== expectedFiles[index].sha256 ||
					sha256Bytes(await readFile(await this.file(input.path))) !== input.sha256
				)
					throw new Error(`Corrupt TLC staged input: ${filename}`);
			}
			{
				const states = parseStateReport(bytes.toString('utf8'));
				if (
					!record.report ||
					record.report.model !== plan.model.id ||
					record.report.variant !== plan.variant.name ||
					record.report.modelConfigSha256 !== plan.proof.modelConfigSha256 ||
					record.report.workers !== plan.proof.workers ||
					record.report.heapMiB !== plan.proof.heapMiB ||
					record.report.logPath !== path.join(this.root, record.log) ||
					!Number.isFinite(record.report.durationMilliseconds) ||
					record.report.durationMilliseconds < 0 ||
					Object.entries(states).some(([name, value]) => record.report[name] !== value)
				)
					throw new Error('Invalid TLC outcome report');
			}
			if (record.status === 'passed') assertCompleteTlcOutput(bytes.toString('utf8'));
			this.configurations.set(key(plan.model.id, plan.variant.name), {
				record,
				output: bytes.toString('utf8'),
			});
		}
		for (const filename of await readdir(path.join(this.root, 'models'))) {
			if (filename.endsWith('.tmp')) continue;
			const model = plans.find((entry) => filename === `${entry.model.id}.json`)?.model;
			if (!model) throw new Error(`Unknown TLC model record: ${filename}`);
			const record = await optionalJson(await this.file(`models/${filename}`));
			exactKeys(
				record,
				['schemaVersion', 'kind', 'identity', 'model', 'status', 'message', 'configurations'],
				'model record',
			);
			const configurations = this.configurationReferences(model);
			if (
				record.schemaVersion !== 1 ||
				record.kind !== 'model' ||
				record.identity !== this.manifest.identity ||
				record.model !== model.id ||
				!['passed', 'failed'].includes(record.status) ||
				!isDeepStrictEqual(record.configurations, configurations) ||
				(record.message !== null && typeof record.message !== 'string')
			)
				throw new Error(`Invalid TLC model proof: ${filename}`);
			if (
				record.status === 'passed' &&
				configurations.some(
					(entry) =>
						this.configurations.get(key(model.id, entry.variant)).record.status !== 'passed',
				)
			)
				throw new Error('A passed model cannot contain failed configurations');
			if (record.status === 'passed')
				assertRequiredCoverage(
					model.id,
					configurations.map(
						(entry) => this.configurations.get(key(model.id, entry.variant)).output,
					),
					model.requiredCoverage,
				);
			this.models.set(model.id, record);
		}
		const scope = await optionalJson(await this.file('scope.json'));
		if (scope !== undefined) {
			exactKeys(scope, ['schemaVersion', 'identity', 'requested'], 'run scope');
			if (
				scope.schemaVersion !== 1 ||
				scope.identity !== this.manifest.identity ||
				!Array.isArray(scope.requested) ||
				scope.requested.some((id) => !plans.some((plan) => plan.model.id === id)) ||
				!isDeepStrictEqual(scope.requested, [...new Set(scope.requested)].toSorted())
			)
				throw new Error('Invalid TLC requested scope');
			this.requested = scope.requested;
		}
	}

	configurationReferences(model) {
		return (model.variants ?? [{ name: 'default' }]).map((variant) => {
			const committed = this.configurations.get(key(model.id, variant.name));
			if (!committed)
				throw new Error(`Model record lacks configuration ${model.id}/${variant.name}`);
			return { variant: variant.name, sha256: sha256Bytes(JSON.stringify(committed.record)) };
		});
	}

	async request(models) {
		this.requested = [
			...new Set([...this.requested, ...models.map((model) => model.id)]),
		].toSorted();
		await atomicJson(
			await this.file('scope.json'),
			{ schemaVersion: 1, identity: this.manifest.identity, requested: this.requested },
			{ replace: true },
		);
	}

	previousModelFailures() {
		return new Map(
			[...this.models]
				.filter(([, record]) => record.status === 'failed')
				.map(([id, record]) => [
					id,
					new HistoricalModelFailure(record.message ?? `${id} failed previously`),
				]),
		);
	}

	configuration(plan) {
		return this.configurations.get(key(plan.model.id, plan.variant.name));
	}

	async commitConfiguration(plan, { report, status, message, log, inputs }, signal) {
		const filename = `${key(plan.model.id, plan.variant.name)}.json`;
		if (this.configuration(plan)) throw new Error(`Refusing to overwrite ${filename}`);
		const bytes = await readFile(await this.file(log));
		if (status === 'passed') assertCompleteTlcOutput(bytes.toString('utf8'));
		const record = {
			schemaVersion: 1,
			kind: 'configuration',
			identity: this.manifest.identity,
			proof: plan.proof,
			status,
			message,
			report,
			log,
			logSha256: sha256Bytes(bytes),
			inputs,
		};
		await atomicJson(await this.file(`records/${filename}`), record, {
			signal: status === 'passed' ? signal : undefined,
		});
		this.configurations.set(key(plan.model.id, plan.variant.name), {
			record,
			output: bytes.toString('utf8'),
		});
	}

	async commitModel(model, outcome, signal) {
		if (this.models.has(model.id)) return;
		const record = {
			schemaVersion: 1,
			kind: 'model',
			identity: this.manifest.identity,
			model: model.id,
			status: outcome.status,
			message: outcome.error?.message ?? null,
			configurations: this.configurationReferences(model),
		};
		await atomicJson(await this.file(`models/${model.id}.json`), record, {
			signal: outcome.status === 'passed' ? signal : undefined,
		});
		this.models.set(model.id, record);
	}

	summary() {
		const models = this.requested.map((id) => {
			const aggregate = this.models.get(id);
			const failed = [...this.configurations.values()].some(
				({ record }) => record.proof.model === id && record.status === 'failed',
			);
			return { model: id, status: failed ? 'failed' : (aggregate?.status ?? 'pending') };
		});
		return {
			requestedModels: models.length,
			passedModels: models.filter((entry) => entry.status === 'passed').length,
			failedModels: models.filter((entry) => entry.status === 'failed').length,
			pendingModels: models.filter((entry) => entry.status === 'pending').length,
			complete: models.every((entry) => entry.status === 'passed'),
			models,
		};
	}

	async close() {
		if (!this.lease) return;
		await this.lease.close();
		await rm(path.join(this.root, '.tla-run.lock'));
		this.lease = undefined;
	}
}
