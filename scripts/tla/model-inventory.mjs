import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export const inventoryPath = path.join(repositoryRoot, 'scripts/tla/models.json');

function runGit(arguments_) {
	return new Promise((resolve, reject) => {
		const child = spawn('git', arguments_, {
			cwd: repositoryRoot,
			stdio: ['ignore', 'pipe', 'pipe'],
		});
		let output = '';
		let errorOutput = '';
		child.stdout.on('data', (chunk) => {
			output += chunk.toString();
		});
		child.stderr.on('data', (chunk) => {
			errorOutput += chunk.toString();
		});
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (signal) {
				reject(new Error(`git ${arguments_[0]} terminated by ${signal}`));
				return;
			}
			if (code !== 0) {
				reject(new Error(`git ${arguments_[0]} failed: ${errorOutput.trim() || `status ${code}`}`));
				return;
			}
			resolve(output.split(/\r?\n/u).filter(Boolean));
		});
	});
}

export async function changedPathsSince(reference, { includeUntracked = true } = {}) {
	const [changed, untracked] = await Promise.all([
		runGit(['diff', '--name-only', reference, '--']),
		includeUntracked ? runGit(['ls-files', '--others', '--exclude-standard']) : [],
	]);
	return [...new Set([...changed, ...untracked])];
}

function assert(condition, message) {
	if (!condition) throw new Error(message);
}

function assertNonEmptyString(value, field) {
	assert(typeof value === 'string' && value.length > 0, `${field} must be a non-empty string`);
}

function assertStringArray(value, field, { allowEmpty = false } = {}) {
	assert(Array.isArray(value), `${field} must be an array`);
	if (!allowEmpty) assert(value.length > 0, `${field} must not be empty`);
	for (const item of value) assertNonEmptyString(item, `${field}[]`);
	assert(new Set(value).size === value.length, `${field} must not contain duplicates`);
}

function assertRepositoryPath(value, field) {
	assertNonEmptyString(value, field);
	assert(!path.isAbsolute(value), `${field} must be repository-relative`);
	assert(value === value.replaceAll('\\', '/'), `${field} must use forward slashes`);
	assert(
		!value.startsWith('../') && !value.includes('/../'),
		`${field} must stay in the repository`,
	);
}

export function validateInventory(inventory) {
	assert(inventory !== null && typeof inventory === 'object', 'inventory must be an object');
	assert(inventory.schemaVersion === 1, 'inventory.schemaVersion must equal 1');
	assert(
		inventory.tool !== null && typeof inventory.tool === 'object',
		'inventory.tool is required',
	);
	assertNonEmptyString(inventory.tool.version, 'inventory.tool.version');
	assertNonEmptyString(inventory.tool.downloadUrl, 'inventory.tool.downloadUrl');
	assert(
		/^[a-f\d]{64}$/u.test(inventory.tool.sha256),
		'inventory.tool.sha256 must be a lowercase SHA-256 digest',
	);
	assert(
		Number.isSafeInteger(inventory.defaultTimeoutMilliseconds) &&
			inventory.defaultTimeoutMilliseconds > 0,
		'inventory.defaultTimeoutMilliseconds must be a positive integer',
	);
	assertStringArray(inventory.sharedTriggers, 'inventory.sharedTriggers');
	assert(
		Array.isArray(inventory.models) && inventory.models.length > 0,
		'inventory.models is required',
	);

	const identifiers = new Set();
	const modules = new Set();
	const configs = new Set();
	let previousSortKey = '';
	for (const [index, model] of inventory.models.entries()) {
		const field = `inventory.models[${index}]`;
		assert(model !== null && typeof model === 'object', `${field} must be an object`);
		assertNonEmptyString(model.id, `${field}.id`);
		assert(/^[a-z\d]+(?:-[a-z\d]+)*$/u.test(model.id), `${field}.id must be kebab-case`);
		assert(!identifiers.has(model.id), `duplicate model id: ${model.id}`);
		identifiers.add(model.id);
		assertNonEmptyString(model.owner, `${field}.owner`);
		assertRepositoryPath(model.workspace, `${field}.workspace`);
		assertRepositoryPath(model.module, `${field}.module`);
		assertRepositoryPath(model.config, `${field}.config`);
		assert(!modules.has(model.module), `duplicate model module: ${model.module}`);
		assert(!configs.has(model.config), `duplicate model config: ${model.config}`);
		modules.add(model.module);
		configs.add(model.config);
		assert(model.module.endsWith('.tla'), `${field}.module must end in .tla`);
		assert(model.config.endsWith('.cfg'), `${field}.config must end in .cfg`);
		assert(
			model.module.startsWith(`${model.workspace}/formal/tla/`),
			`${field}.module must belong to its workspace formal/tla directory`,
		);
		assert(
			model.config.startsWith(`${model.workspace}/formal/tla/`),
			`${field}.config must belong to its workspace formal/tla directory`,
		);
		assertStringArray(model.claims, `${field}.claims`);
		for (const claim of model.claims) assertRepositoryPath(claim, `${field}.claims[]`);
		assertStringArray(model.triggers, `${field}.triggers`);
		assertStringArray(model.requiredCoverage, `${field}.requiredCoverage`);
		if (model.timeoutMilliseconds !== undefined) {
			assert(
				Number.isSafeInteger(model.timeoutMilliseconds) && model.timeoutMilliseconds > 0,
				`${field}.timeoutMilliseconds must be a positive integer`,
			);
		}
		if (model.variants !== undefined) {
			assert(
				Array.isArray(model.variants) && model.variants.length > 0,
				`${field}.variants must not be empty`,
			);
			const variantNames = new Set();
			for (const [variantIndex, variant] of model.variants.entries()) {
				const variantField = `${field}.variants[${variantIndex}]`;
				assert(
					variant !== null && typeof variant === 'object',
					`${variantField} must be an object`,
				);
				assertNonEmptyString(variant.name, `${variantField}.name`);
				assert(
					/^[a-z\d]+(?:-[a-z\d]+)*$/u.test(variant.name),
					`${variantField}.name must be kebab-case`,
				);
				assert(!variantNames.has(variant.name), `${field} has duplicate variant ${variant.name}`);
				variantNames.add(variant.name);
				assert(
					variant.constants !== null &&
						typeof variant.constants === 'object' &&
						Object.keys(variant.constants).length > 0,
					`${variantField}.constants must be a non-empty object`,
				);
			}
		}

		const sortKey = `${model.workspace}\0${model.id}`;
		assert(sortKey > previousSortKey, 'inventory.models must be sorted by workspace then id');
		previousSortKey = sortKey;
	}

	return inventory;
}

export function globMatches(pattern, candidate) {
	const escaped = pattern.replaceAll(/[.+?^${}()|[\]\\]/gu, '\\$&');
	const expression = escaped
		.replaceAll('**', '\u0000')
		.replaceAll('*', '[^/]*')
		.replaceAll('\u0000', '.*');
	return new RegExp(`^${expression}$`, 'u').test(candidate);
}

export function selectModelsForChangedPaths(inventory, changedPaths) {
	const normalizedPaths = changedPaths.map((changedPath) => changedPath.replaceAll('\\', '/'));
	if (
		normalizedPaths.some((changedPath) =>
			inventory.sharedTriggers.some((trigger) => globMatches(trigger, changedPath)),
		)
	) {
		return [...inventory.models];
	}

	return inventory.models.filter((model) => {
		const triggers = [model.module, model.config, ...model.claims, ...model.triggers];
		return normalizedPaths.some((changedPath) =>
			triggers.some((trigger) => globMatches(trigger, changedPath)),
		);
	});
}

export function applyConfigConstants(config, constants) {
	let updated = config;
	for (const [name, value] of Object.entries(constants)) {
		assert(/^[A-Za-z][A-Za-z\d_]*$/u.test(name), `invalid TLA+ constant name: ${name}`);
		let serialized = String(value);
		if (typeof value === 'string') serialized = JSON.stringify(value);
		if (typeof value === 'boolean') serialized = value ? 'TRUE' : 'FALSE';
		const expression = new RegExp(`^(\\s*CONSTANT\\s+${name}\\s*=\\s*).*$`, 'gmu');
		const matches = [...updated.matchAll(expression)];
		assert(matches.length === 1, `expected exactly one assignment for TLA+ constant ${name}`);
		updated = updated.replace(expression, `$1${serialized}`);
	}
	return updated;
}

export function variantConfigFileName(configPath, variantName) {
	const extension = path.extname(configPath);
	return `${path.basename(configPath, extension)}-${variantName}${extension}`;
}

export function parseCoverage(output) {
	const coverage = new Map();
	const expression = /^\s*<([^\s>]+)\b[^>]*>:\s*([\d,]+)(?::([\d,]+))?\s*$/gmu;
	for (const match of output.matchAll(expression)) {
		const counts = match
			.slice(2)
			.filter((value) => value !== undefined)
			.map((value) => Number.parseInt(value.replaceAll(',', ''), 10));
		const previous = coverage.get(match[1]) ?? [];
		coverage.set(match[1], [...previous, ...counts]);
	}
	return coverage;
}

export function assertRequiredCoverage(modelId, outputs, requiredCoverage) {
	const coverage = parseCoverage(outputs.join('\n'));
	const missing = [];
	const unreached = [];
	for (const marker of requiredCoverage) {
		const counts = coverage.get(marker);
		if (counts === undefined) {
			missing.push(marker);
			continue;
		}
		if (!counts.some((count) => Number.isFinite(count) && count > 0)) unreached.push(marker);
	}
	if (missing.length > 0 || unreached.length > 0) {
		const details = [
			missing.length > 0 ? `missing markers: ${missing.join(', ')}` : '',
			unreached.length > 0 ? `unreached markers: ${unreached.join(', ')}` : '',
		].filter(Boolean);
		throw new Error(
			`${modelId} did not satisfy required TLC action coverage (${details.join('; ')})`,
		);
	}
}

export function parseStateReport(output) {
	const stateMatches = [
		...output.matchAll(
			/([\d,]+) states generated, ([\d,]+) distinct states found, ([\d,]+) states left on queue/gu,
		),
	];
	const lastStateMatch = stateMatches.at(-1);
	const depthMatches = [
		...output.matchAll(/depth of the complete state graph search is ([\d,]+)/giu),
	];
	const lastDepthMatch = depthMatches.at(-1);
	return {
		depth: parseCount(lastDepthMatch?.[1]),
		distinctStates: parseCount(lastStateMatch?.[2]),
		generatedStates: parseCount(lastStateMatch?.[1]),
		statesLeft: parseCount(lastStateMatch?.[3]),
	};
}

export function tlcExitError(modelId, timeoutMilliseconds, { code, signal, timedOut }) {
	if (timedOut) return new Error(`${modelId} TLC exceeded its ${timeoutMilliseconds}ms timeout`);
	if (signal) return new Error(`${modelId} TLC terminated by ${signal}`);
	if (code !== 0) return new Error(`${modelId} TLC exited with status ${code}`);
	return undefined;
}

function parseCount(value) {
	return value === undefined ? undefined : Number.parseInt(value.replaceAll(',', ''), 10);
}
