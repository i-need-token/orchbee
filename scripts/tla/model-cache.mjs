import { execFileSync } from 'node:child_process';
import { lstat, mkdir, open, readFile, readdir, realpath, rm } from 'node:fs/promises';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';

import { assertCompleteTlcOutput, sha256Bytes } from './model-inputs.mjs';
import { assertRequiredCoverage } from './model-inventory.mjs';
import { atomicJson, inside, rejectLinks } from './model-records.mjs';

const marker = 'owner.json';
const leaseName = '.tla-cache.lock';
const maximumEnvelopeBytes = 64 * 1024 ** 2;
const filename = (model) => `${sha256Bytes(model)}.json`;
const configurationKey = (plan) => JSON.stringify([plan.model.id, plan.variant.name]);

function exactKeys(value, keys) {
	if (
		!value ||
		typeof value !== 'object' ||
		Array.isArray(value) ||
		Object.keys(value).length !== keys.length ||
		keys.some((key) => !Object.hasOwn(value, key))
	)
		throw new Error('Invalid cache schema');
}

export function configurationIdentity(plan, runtime) {
	return {
		runtime,
		proof: plan.proof,
		module: plan.model.module,
		configName: plan.configName,
		variants: (plan.model.variants ?? [{ name: 'default' }]).map((variant) => variant.name),
		requiredCoverage: plan.model.requiredCoverage,
	};
}

function decodeEnvelope(envelope, model) {
	exactKeys(envelope, ['schemaVersion', 'model', 'configurations', 'sha256']);
	if (
		envelope.schemaVersion !== 1 ||
		envelope.model !== model.id ||
		!Array.isArray(envelope.configurations)
	)
		throw new Error('Wrong cache envelope');
	if (
		envelope.sha256 !==
		sha256Bytes(
			JSON.stringify({
				schemaVersion: envelope.schemaVersion,
				model: envelope.model,
				configurations: envelope.configurations,
			}),
		)
	)
		throw new Error('Corrupt cache envelope');
	const names = new Set();
	return envelope.configurations.map((record) => {
		exactKeys(record, ['identity', 'durationMilliseconds', 'log', 'logSha256']);
		const variant = record.identity?.proof?.variant;
		if (
			typeof variant !== 'string' ||
			names.has(variant) ||
			typeof record.log !== 'string' ||
			!Number.isFinite(record.durationMilliseconds) ||
			record.durationMilliseconds < 0
		)
			throw new Error('Invalid cached configuration');
		names.add(variant);
		const bytes = Buffer.from(record.log, 'base64');
		if (bytes.toString('base64') !== record.log || sha256Bytes(bytes) !== record.logSha256)
			throw new Error('Corrupt cached log');
		assertCompleteTlcOutput(bytes.toString('utf8'));
		return { ...record, bytes };
	});
}

/** One trusted checkout cache, with whole-model atomic visibility and no historical generations. */
export class ModelSuccessCache {
	constructor(root, lease) {
		this.root = root;
		this.lease = lease;
		this.hits = new Map();
		this.dirty = new Set();
	}

	static async acquire(directory, repository, models) {
		const checkout = await realpath(repository);
		const requested = path.resolve(directory);
		let parent = requested;
		const missingSegments = [];
		for (;;) {
			try {
				await lstat(parent);
				break;
			} catch (error) {
				if (error.code !== 'ENOENT') throw error;
				missingSegments.unshift(path.basename(parent));
				parent = path.dirname(parent);
			}
		}
		await rejectLinks(parent);
		if (path.join(await realpath(parent), ...missingSegments) !== requested)
			throw new Error('TLC cache path must use its canonical spelling without aliases');
		if (
			inside(requested, checkout) ||
			(inside(checkout, requested) && requested !== path.join(checkout, '.tools', 'tla-cache'))
		)
			throw new Error('TLC cache must use .tools/tla-cache or a separate external directory');
		if (inside(checkout, requested))
			execFileSync('git', ['check-ignore', '--quiet', '--', '.tools/tla-cache/owner.json'], {
				cwd: checkout,
				stdio: 'pipe',
			});
		await rejectLinks(requested);
		await mkdir(requested, { recursive: true });
		const root = await realpath(requested);
		await rejectLinks(requested);
		if (
			root !== requested ||
			inside(root, checkout) ||
			(inside(checkout, root) && root !== path.join(checkout, '.tools', 'tla-cache'))
		)
			throw new Error('TLC cache resolved outside its admitted directory');
		const entries = await readdir(root);
		for (const entry of entries) {
			const stat = await lstat(path.join(root, entry));
			if (!stat.isFile() || stat.isSymbolicLink())
				throw new Error('TLC cache entries must be regular files');
		}
		const lease = await open(path.join(root, leaseName), 'wx');
		const cache = new ModelSuccessCache(root, lease);
		try {
			await lease.writeFile(`${process.pid}\n`);
			const ownership = { schemaVersion: 1, kind: 'orchbee-local-tlc-cache', repository: checkout };
			if (entries.length === 0) await atomicJson(path.join(root, marker), ownership);
			else if (
				!entries.includes(marker) ||
				!isDeepStrictEqual(JSON.parse(await readFile(path.join(root, marker), 'utf8')), ownership)
			)
				throw new Error('TLC cache directory is not owned by this checkout');
			const current = new Set(models.map((model) => filename(model.id)));
			for (const entry of entries) {
				if (entry === marker) continue;
				if (!/^[a-f\d]{64}\.json(?:\.[a-f\d-]{36}\.tmp)?$/u.test(entry))
					throw new Error(`Unknown TLC cache file: ${entry}`);
			}
			for (const entry of entries)
				if (entry !== marker && (!current.has(entry) || entry.endsWith('.tmp')))
					await rm(path.join(root, entry));
			return cache;
		} catch (error) {
			await cache.close();
			throw error;
		}
	}

	async prepare(plans, runtime, force = false) {
		this.hits.clear();
		const reasons = [];
		for (const id of new Set(plans.map((plan) => plan.model.id))) {
			const selected = plans.filter((plan) => plan.model.id === id);
			const file = path.join(this.root, filename(id));
			let bytes;
			try {
				const stat = await lstat(file);
				if (!stat.isFile() || stat.isSymbolicLink())
					throw new Error('TLC cache envelope must be a regular file');
				if (!force && stat.size <= maximumEnvelopeBytes) bytes = await readFile(file);
			} catch (error) {
				if (error.code !== 'ENOENT') throw error;
			}
			let records = [];
			let reason = force ? 'forced-refresh' : 'absent-or-oversized';
			if (bytes !== undefined) {
				try {
					records = decodeEnvelope(JSON.parse(bytes.toString('utf8')), selected[0].model);
					reason = 'identity-mismatch';
				} catch {
					reason = 'corrupt';
				}
			}
			for (const plan of selected) {
				const hit = records.find((record) =>
					isDeepStrictEqual(record.identity, configurationIdentity(plan, runtime)),
				);
				if (hit) this.hits.set(configurationKey(plan), hit);
				else reasons.push({ model: id, variant: plan.variant.name, reason });
			}
			if (selected.some((plan) => !this.hits.has(configurationKey(plan)))) {
				this.dirty.add(id);
				await rm(file, { force: true });
			} else {
				// Coverage is a model obligation even when every configuration is reused.
				try {
					assertRequiredCoverage(
						id,
						selected.map((plan) => this.configuration(plan).bytes.toString('utf8')),
						selected[0].model.requiredCoverage,
					);
				} catch {
					for (const plan of selected) {
						this.hits.delete(configurationKey(plan));
						reasons.push({ model: id, variant: plan.variant.name, reason: 'corrupt-coverage' });
					}
					this.dirty.add(id);
					await rm(file);
				}
			}
		}
		return reasons;
	}

	configuration(plan) {
		return this.hits.get(configurationKey(plan));
	}

	async commitModel(plans, runtime, outcomes, validate, signal) {
		const model = plans[0].model;
		if (!this.dirty.has(model.id)) return;
		const configurations = plans.map((plan) => {
			const outcome = outcomes.get(configurationKey(plan));
			if (!outcome) throw new Error(`Incomplete cache model ${model.id}`);
			assertCompleteTlcOutput(outcome.bytes.toString('utf8'));
			return {
				identity: configurationIdentity(plan, runtime),
				durationMilliseconds: outcome.durationMilliseconds,
				log: outcome.bytes.toString('base64'),
				logSha256: sha256Bytes(outcome.bytes),
			};
		});
		assertRequiredCoverage(
			model.id,
			configurations.map((record) => Buffer.from(record.log, 'base64').toString('utf8')),
			model.requiredCoverage,
		);
		const body = { schemaVersion: 1, model: model.id, configurations };
		const envelope = { ...body, sha256: sha256Bytes(JSON.stringify(body)) };
		if (Buffer.byteLength(JSON.stringify(envelope)) > maximumEnvelopeBytes)
			throw new Error('TLC cache envelope exceeds 64 MiB');
		await validate();
		signal?.throwIfAborted();
		await atomicJson(path.join(this.root, filename(model.id)), envelope, { signal });
		this.dirty.delete(model.id);
	}

	async close() {
		if (!this.lease) return;
		await this.lease.close();
		await rm(path.join(this.root, leaseName));
		this.lease = undefined;
	}
}
