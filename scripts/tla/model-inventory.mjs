import { spawn } from 'node:child_process';
import { lstat, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

import { documentScopeChanged, workflowTlaScope } from './document-scopes.mjs';

export const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export const inventoryPath = path.join(repositoryRoot, 'scripts/tla/models.json');

function runGitText(arguments_, acceptedCodes = [0]) {
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
		child.once('close', (code, signal) => {
			if (signal) {
				reject(new Error(`git ${arguments_[0]} terminated by ${signal}`));
				return;
			}
			if (!acceptedCodes.includes(code)) {
				reject(new Error(`git ${arguments_[0]} failed: ${errorOutput.trim() || `status ${code}`}`));
				return;
			}
			resolve(output);
		});
	});
}

async function runGit(arguments_, acceptedCodes) {
	return (await runGitText(arguments_, acceptedCodes)).split(/\r?\n/u).filter(Boolean);
}

export function inventoryInputPaths(inventory) {
	return [
		...new Set([
			'scripts/tla/models.json',
			...inventory.models.flatMap((model) => [
				model.module,
				model.config,
				...(model.supportingModules ?? []),
				...model.claims,
				...(model.documentScopes ?? []).map((scope) => scope.path),
			]),
		]),
	];
}

/** Development may stage new inputs; ignored authority never supplies an immutable revision. */
export async function assertInputAdmission(paths, revision) {
	if (revision !== undefined) {
		const tracked = new Set(
			(await runGitText(['ls-tree', '-r', '--name-only', '-z', revision])).split('\0'),
		);
		const missing = paths.filter((input) => !tracked.has(input));
		if (missing.length)
			throw new Error(`Frozen TLA inputs must be tracked in HEAD: ${missing.join(', ')}`);
		const requested = new Set(paths);
		const hidden = (await runGitText(['ls-files', '-v', '-z']))
			.split('\0')
			.filter(
				(entry) =>
					requested.has(entry.slice(2)) &&
					(entry[0] === 'S' || entry[0] === entry[0]?.toLowerCase()),
			)
			.map((entry) => entry.slice(2));
		if (hidden.length)
			throw new Error(
				`Frozen TLA inputs cannot use skip-worktree or assume-unchanged: ${hidden.join(', ')}`,
			);
		return;
	}
	const requests = [];
	for (let index = 0; index < paths.length; index += 30)
		requests.push(runGit(['check-ignore', '--', ...paths.slice(index, index + 30)], [0, 1]));
	const results = await Promise.allSettled(requests);
	for (const result of results) {
		if (result.status === 'rejected') throw result.reason;
		if (result.value.length)
			throw new Error(
				`TLA inputs and claim authority cannot be ignored: ${result.value.join(', ')}`,
			);
	}
}

export async function assertTriggerExclusionAdmission(paths) {
	if (paths.length === 0) return;
	const tracked = new Set(await trackedFiles());
	for (const excluded of paths) {
		assert(tracked.has(excluded), `TLA trigger exclusion must be tracked: ${excluded}`);
		const stat = await lstat(path.join(repositoryRoot, excluded));
		assert(
			stat.isFile() && !stat.isSymbolicLink(),
			`TLA trigger exclusion must be a regular file: ${excluded}`,
		);
	}
}

async function verifiedBase(reference) {
	const revision = (
		await runGitText(['rev-parse', '--verify', '--end-of-options', reference + '^{commit}'])
	).trim();
	await runGitText(['merge-base', '--is-ancestor', revision, 'HEAD']);
	const hidden = (await runGitText(['ls-files', '-v', '-z']))
		.split('\0')
		.filter((entry) => entry && (entry[0] === 'S' || entry[0] === entry[0].toLowerCase()));
	if (hidden.length)
		throw new Error('TLA selection cannot use skip-worktree or assume-unchanged entries');
	return revision;
}

async function pathsFromBase(revision, { includeUntracked = true } = {}) {
	const results = await Promise.allSettled([
		runGitText(['diff', '--no-renames', '--name-only', '-z', revision, 'HEAD', '--']),
		runGitText(['diff', '--cached', '--no-renames', '--name-only', '-z', 'HEAD', '--']),
		runGitText(['diff', '--no-renames', '--name-only', '-z', '--']),
		includeUntracked ? runGitText(['ls-files', '--others', '--exclude-standard', '-z']) : '',
	]);
	for (const result of results) if (result.status === 'rejected') throw result.reason;
	const [committed, staged, unstaged, untracked] = results.map((result) =>
		result.value.split('\0').filter(Boolean),
	);
	return {
		paths: [...new Set([...committed, ...staged, ...unstaged, ...untracked])],
		staged,
		unstaged,
	};
}

async function pathVersions(revision, changedPath) {
	const results = await Promise.allSettled([
		runGitText(['show', `${revision}:${changedPath}`]),
		runGitText(['show', `HEAD:${changedPath}`]),
		runGitText(['show', `:${changedPath}`]),
		readFile(path.join(repositoryRoot, changedPath), 'utf8'),
	]);
	for (const result of results) if (result.status === 'rejected') throw result.reason;
	const [before, head, staged, after] = results.map((result) => result.value);
	return { before, intermediate: [head, staged], after };
}

/** Both local execution and CI selection compare the same requested inventory base. */
export async function selectModelsSince(inventory, reference, options) {
	const revision = await verifiedBase(reference);
	const { paths: changedPaths, staged, unstaged } = await pathsFromBase(revision, options);
	const inventoryRelativePath = 'scripts/tla/models.json';
	let previousInventory;
	if (changedPaths.includes(inventoryRelativePath)) {
		// Mixed inventory snapshots have no single baseline/current registration pair.
		if (staged.includes(inventoryRelativePath) && unstaged.includes(inventoryRelativePath))
			return [...inventory.models];
		const versions = await pathVersions(revision, inventoryRelativePath);
		previousInventory = validateInventory(JSON.parse(versions.before));
		const intermediate = versions.intermediate.map((text) => validateInventory(JSON.parse(text)));
		if (
			intermediate.some(
				(entry) =>
					!isDeepStrictEqual(entry, previousInventory) && !isDeepStrictEqual(entry, inventory),
			)
		)
			return [...inventory.models];
	}
	const scopedPaths = new Set([
		'.github/workflows/verify.yml',
		...[inventory, previousInventory]
			.filter(Boolean)
			.flatMap((entry) =>
				entry.models.flatMap((model) => (model.documentScopes ?? []).map((scope) => scope.path)),
			),
	]);
	const contents = new Map();
	const scopedResults = await Promise.allSettled(
		changedPaths
			.filter((changedPath) => scopedPaths.has(changedPath))
			.map(async (changedPath) => {
				try {
					contents.set(changedPath, await pathVersions(revision, changedPath));
				} catch {
					/* Missing versions retain whole-file selection. */
				}
			}),
	);
	for (const result of scopedResults) if (result.status === 'rejected') throw result.reason;
	return selectModelsForChangedPaths(inventory, changedPaths, previousInventory, contents);
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

function assertRepositoryPath(value, field, { allowRoot = false } = {}) {
	assertNonEmptyString(value, field);
	if (allowRoot && value === '.') return;
	assert(
		!path.posix.isAbsolute(value) && !path.win32.isAbsolute(value),
		`${field} must be repository-relative`,
	);
	assert(value === value.replaceAll('\\', '/'), `${field} must use forward slashes`);
	assert(!/^[a-z]:/iu.test(value), `${field} must be repository-relative`);
	assert(
		value.split('/').every((segment) => segment !== '' && segment !== '.'),
		`${field} must use canonical repository path segments`,
	);
	assert(!value.split('/').includes('..'), `${field} must stay in the repository`);
}

export function validateInventory(inventory) {
	assert(inventory !== null && typeof inventory === 'object', 'inventory must be an object');
	assert(
		Object.keys(inventory).every((key) =>
			['schemaVersion', 'tool', 'defaultTimeoutMilliseconds', 'sharedTriggers', 'models'].includes(
				key,
			),
		),
		'inventory contains unknown execution metadata',
	);
	assert(inventory.schemaVersion === 1, 'inventory.schemaVersion must equal 1');
	assert(
		inventory.tool !== null && typeof inventory.tool === 'object',
		'inventory.tool is required',
	);
	assert(
		Object.keys(inventory.tool).every((field) =>
			['version', 'downloadUrl', 'sha256'].includes(field),
		),
		'inventory.tool contains unknown execution metadata',
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
	for (const trigger of inventory.sharedTriggers)
		assertRepositoryPath(trigger, 'inventory.sharedTriggers[]');
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
		assert(
			Object.keys(model).every((key) =>
				[
					'id',
					'owner',
					'workspace',
					'module',
					'config',
					'claims',
					'triggers',
					'triggerExclusions',
					'requiredCoverage',
					'variants',
					'workers',
					'timeoutMilliseconds',
					'supportingModules',
					'documentScopes',
				].includes(key),
			),
			`${field} contains unknown execution metadata`,
		);
		assertNonEmptyString(model.id, `${field}.id`);
		assert(/^[a-z\d]+(?:-[a-z\d]+)*$/u.test(model.id), `${field}.id must be kebab-case`);
		assert(!identifiers.has(model.id), `duplicate model id: ${model.id}`);
		identifiers.add(model.id);
		assertNonEmptyString(model.owner, `${field}.owner`);
		assertRepositoryPath(model.workspace, `${field}.workspace`, { allowRoot: true });
		assertRepositoryPath(model.module, `${field}.module`);
		assertRepositoryPath(model.config, `${field}.config`);
		assert(!modules.has(model.module), `duplicate model module: ${model.module}`);
		assert(!configs.has(model.config), `duplicate model config: ${model.config}`);
		modules.add(model.module);
		configs.add(model.config);
		assert(model.module.endsWith('.tla'), `${field}.module must end in .tla`);
		assert(model.config.endsWith('.cfg'), `${field}.config must end in .cfg`);
		const formalPrefix = model.workspace === '.' ? 'formal/tla/' : `${model.workspace}/formal/tla/`;
		assert(
			model.module.startsWith(formalPrefix),
			`${field}.module must belong to its workspace formal/tla directory`,
		);
		assert(
			model.config.startsWith(formalPrefix),
			`${field}.config must belong to its workspace formal/tla directory`,
		);
		if (model.supportingModules !== undefined) {
			assertStringArray(model.supportingModules, `${field}.supportingModules`);
			for (const supportingModule of model.supportingModules) {
				assertRepositoryPath(supportingModule, `${field}.supportingModules[]`);
				assert(supportingModule.endsWith('.tla'), `${field}.supportingModules[] must end in .tla`);
				assert(
					path.dirname(supportingModule) === path.dirname(model.module),
					`${field}.supportingModules[] must be alongside its model module`,
				);
				assert(
					supportingModule !== model.module,
					`${field}.supportingModules[] must not repeat its model module`,
				);
			}
		}
		assertStringArray(model.claims, `${field}.claims`);
		for (const claim of model.claims) assertRepositoryPath(claim, `${field}.claims[]`);
		assertStringArray(model.triggers, `${field}.triggers`);
		for (const trigger of model.triggers) assertRepositoryPath(trigger, `${field}.triggers[]`);
		if (model.triggerExclusions !== undefined) {
			assertStringArray(model.triggerExclusions, `${field}.triggerExclusions`, {
				allowEmpty: true,
			});
			const direct = [
				model.module,
				model.config,
				...(model.supportingModules ?? []),
				...model.claims,
				...model.triggers.filter((trigger) => !trigger.includes('*')),
			];
			for (const excluded of model.triggerExclusions) {
				assertRepositoryPath(excluded, `${field}.triggerExclusions[]`);
				assert(!excluded.includes('*'), `${field}.triggerExclusions must be exact file paths`);
				assert(
					!direct.includes(excluded),
					`${field}.triggerExclusions cannot exclude direct dependencies`,
				);
				assert(
					model.triggers.some((trigger) => trigger.includes('*') && globMatches(trigger, excluded)),
					`${field}.triggerExclusions must match a wildcard trigger`,
				);
			}
		}
		if (model.documentScopes !== undefined) {
			assert(
				Array.isArray(model.documentScopes) && model.documentScopes.length > 0,
				`${field}.documentScopes must not be empty`,
			);
			const scopedPaths = new Set();
			for (const scope of model.documentScopes) {
				assert(
					scope !== null &&
						typeof scope === 'object' &&
						Object.keys(scope).length === 3 &&
						Object.keys(scope).every((key) =>
							['path', 'relevantHeadings', 'unrelatedHeadings'].includes(key),
						),
					`${field}.documentScopes[] has unknown or missing fields`,
				);
				assertRepositoryPath(scope.path, `${field}.documentScopes[].path`);
				assert(!scopedPaths.has(scope.path), `${field}.documentScopes has duplicate paths`);
				scopedPaths.add(scope.path);
				assert(
					model.claims.includes(scope.path) || model.triggers.includes(scope.path),
					`${field}.documentScopes must scope an exact registered dependency`,
				);
				assertStringArray(scope.relevantHeadings, `${field}.documentScopes[].relevantHeadings`);
				assertStringArray(scope.unrelatedHeadings, `${field}.documentScopes[].unrelatedHeadings`);
				assert(
					new Set([...scope.relevantHeadings, ...scope.unrelatedHeadings]).size ===
						scope.relevantHeadings.length + scope.unrelatedHeadings.length,
					`${field}.documentScopes headings overlap`,
				);
			}
		}
		assertStringArray(model.requiredCoverage, `${field}.requiredCoverage`);
		if (model.timeoutMilliseconds !== undefined) {
			assert(
				Number.isSafeInteger(model.timeoutMilliseconds) && model.timeoutMilliseconds > 0,
				`${field}.timeoutMilliseconds must be a positive integer`,
			);
		}
		if (model.workers !== undefined) {
			assert(
				Number.isSafeInteger(model.workers) && model.workers >= 1 && model.workers <= 8,
				`${field}.workers must be an integer between 1 and 8`,
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
				assert(
					Object.keys(variant).every((variantKey) => ['name', 'constants'].includes(variantKey)),
					`${variantField} contains unknown execution metadata`,
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
				assert(
					Object.values(variant.constants).every(
						(value) =>
							typeof value === 'boolean' ||
							typeof value === 'string' ||
							(typeof value === 'number' && Number.isFinite(value)),
					),
					`${variantField}.constants must contain finite numbers, strings or booleans`,
				);
			}
		}

		const sortKey = `${model.workspace}\0${model.id}`;
		assert(sortKey > previousSortKey, 'inventory.models must be sorted by workspace then id');
		previousSortKey = sortKey;
	}

	return inventory;
}

function globExpression(pattern) {
	const escaped = pattern.replaceAll(/[.+?^${}()|[\]\\]/gu, '\\$&');
	const expression = escaped
		.replaceAll('**', '\u0000')
		.replaceAll('*', '[^/]*')
		.replaceAll('\u0000', '.*');
	return new RegExp(`^${expression}$`, 'su');
}

export function globMatches(pattern, candidate) {
	return globExpression(pattern).test(candidate);
}

/** A trigger that matches no tracked file has gone stale: its owner moved or was deleted. */
export function assertTriggersMatchTrackedFiles(inventory, tracked) {
	const stale = [];
	const check = (owner, trigger) => {
		const expression = globExpression(trigger);
		if (!tracked.some((file) => expression.test(file))) stale.push(`- ${owner}: ${trigger}`);
	};
	for (const trigger of inventory.sharedTriggers) check('sharedTriggers', trigger);
	for (const model of inventory.models)
		for (const trigger of model.triggers) check(model.id, trigger);
	if (stale.length)
		throw new Error(`TLA triggers must match at least one tracked file:\n${stale.join('\n')}`);
}

export async function trackedFiles() {
	return (await runGitText(['ls-files', '--cached', '-z'])).split('\0').filter(Boolean);
}

// These are test entrypoints, never inputs to a TLC invocation. They still run in
// the mandatory inventory gate. Product dependencies still match these original paths.
export const inventoryTestPaths = new Set([
	'scripts/tla/model-inventory.test.mjs',
	'scripts/tla/model-checks.test.mjs',
	'scripts/tla/run-models.test.mjs',
	'scripts/tla/document-scopes.test.mjs',
	'scripts/tla/model-records.test.mjs',
	'scripts/tla/model-cache.test.mjs',
	'scripts/tla/execution-properties.test.mjs',
	'scripts/tla/toolchain.test.mjs',
]);

export const toolingCommand =
	'node --test scripts/tla/model-cache-cli.test.mjs scripts/tla/model-mutations.test.mjs';
const workflowPath = '.github/workflows/verify.yml';
const toolingStandards = new Set(['docs/VERIFICATION.md', 'docs/MODELING-AND-CORRECTNESS.md']);
function toolingPath(changedPath) {
	return (
		changedPath.startsWith('scripts/tla/') ||
		toolingStandards.has(changedPath) ||
		changedPath === workflowPath
	);
}

function modelObligations(model) {
	if (model === undefined) return undefined;
	const { triggers: _triggers, triggerExclusions: _exclusions, ...obligations } = model;
	return obligations;
}

function sourceTriggersChanged(inventory, previousInventory) {
	if (previousInventory === undefined) return false;
	return inventory.models.some((model) => {
		const previous = previousInventory.models.find(({ id }) => id === model.id);
		return (
			previous &&
			(!isDeepStrictEqual(model.triggers, previous.triggers) ||
				!isDeepStrictEqual(model.triggerExclusions ?? [], previous.triggerExclusions ?? []))
		);
	});
}

function modelPathMatches(model, changedPath, versions) {
	const direct = [model.module, model.config, ...(model.supportingModules ?? []), ...model.claims];
	const matches =
		direct.some((trigger) => globMatches(trigger, changedPath)) ||
		(!(model.triggerExclusions ?? []).includes(changedPath) &&
			model.triggers.some((trigger) => globMatches(trigger, changedPath)));
	if (!matches) return false;
	const scope = model.documentScopes?.find((entry) => entry.path === changedPath);
	return (
		!versions ||
		!scope ||
		versionTransitions(versions).some(([before, after]) =>
			documentScopeChanged(scope, before, after),
		)
	);
}

function versionTransitions(versions) {
	const texts = [versions.before, ...(versions.intermediate ?? []), versions.after];
	return texts.slice(1).map((after, index) => [texts[index], after]);
}

export function selectModelsForChangedPaths(
	inventory,
	changedPaths,
	previousInventory,
	contents = new Map(),
) {
	const normalizedPaths = changedPaths.map((changedPath) => changedPath.replaceAll('\\', '/'));
	const workflow = contents.get(workflowPath);
	let workflowScope = 'none';
	if (normalizedPaths.includes(workflowPath)) {
		const scopes = workflow
			? versionTransitions(workflow).map(([before, after]) => workflowTlaScope(before, after))
			: ['all'];
		if (scopes.includes('all')) workflowScope = 'all';
		else if (scopes.includes('tooling')) workflowScope = 'tooling';
	}
	if (workflowScope === 'all') return [...inventory.models];
	if (previousInventory !== undefined) {
		const { models: _previousModels, ...previousShared } = previousInventory;
		const { models: _currentModels, ...currentShared } = inventory;
		if (!isDeepStrictEqual(previousShared, currentShared)) return [...inventory.models];
	}
	// Tool evidence has explicit precedence over sharedTriggers. Never filter the
	// product channel: a product owner may independently declare any of these paths.
	const toolPaths = normalizedPaths.filter(
		(changedPath) =>
			!inventoryTestPaths.has(changedPath) &&
			!(changedPath === workflowPath && workflowScope === 'none') &&
			!(changedPath === 'scripts/tla/models.json' && previousInventory !== undefined),
	);
	if (toolPaths.includes('scripts/tla/models.json')) return [...inventory.models];
	const toolChanged =
		toolPaths.some(toolingPath) || sourceTriggersChanged(inventory, previousInventory);
	if (toolChanged) return [...inventory.models];
	if (
		toolPaths.some(
			(changedPath) =>
				!toolingPath(changedPath) &&
				inventory.sharedTriggers.some((trigger) => globMatches(trigger, changedPath)),
		)
	) {
		return [...inventory.models];
	}

	return inventory.models.filter((model) => {
		const previous = previousInventory?.models.find(({ id }) => id === model.id);
		if (
			previousInventory !== undefined &&
			!isDeepStrictEqual(modelObligations(model), modelObligations(previous))
		)
			return true;
		return normalizedPaths.some((changedPath) =>
			[model, ...(previous === undefined ? [] : [previous])].some((entry) =>
				modelPathMatches(entry, changedPath, contents.get(changedPath)),
			),
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

function parseCoverage(output) {
	const coverage = new Map();
	const expression = /^\s*<([^\s>]+)\b[^>]*>:\s*([\d,]+)(?::([\d,]+))?\s*$/gmu;
	for (const match of output.matchAll(expression)) {
		// Pinned TLC action headers report distinct:generated states. A transition
		// back to an already discovered state is still a reached action witness.
		const counts = [Number.parseInt((match[3] ?? match[2]).replaceAll(',', ''), 10)];
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
