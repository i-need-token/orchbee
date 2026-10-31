import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
	applyConfigConstants,
	assertRequiredCoverage,
	assertTriggersMatchTrackedFiles,
	globMatches,
	inventoryPath,
	parseStateReport,
	repositoryRoot,
	selectModelsForChangedPaths,
	tlcExitError,
	validateInventory,
	variantConfigFileName,
} from './model-inventory.mjs';

const repositoryInventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));

test('root verification evidence uses canonical root formal paths', () => {
	const rootModel = {
		...repositoryInventory.models[0],
		id: 'root-verification-probe',
		owner: 'orchbee',
		workspace: '.',
		module: 'formal/tla/RootVerificationProbe.tla',
		config: 'formal/tla/RootVerificationProbe.cfg',
	};
	delete rootModel.supportingModules;
	const inventory = { ...repositoryInventory, models: [rootModel] };
	assert.equal(validateInventory(inventory).models[0].workspace, '.');
	for (const invalidModule of [
		'apps/orchbee/formal/tla/RootVerificationProbe.tla',
		'./formal/tla/RootVerificationProbe.tla',
		'formal/RootVerificationProbe.tla',
	]) {
		assert.throws(
			() =>
				validateInventory({
					...inventory,
					models: [{ ...rootModel, module: invalidModule }],
				}),
			/formal\/tla directory|canonical repository path segments/u,
		);
	}
});

function selectionInventory() {
	const inventory = structuredClone(repositoryInventory);
	inventory.models = ['alpha', 'beta'].map((id) =>
		Object.assign({}, inventory.models[0], {
			id,
			module: `apps/${id}/formal/tla/Example.tla`,
			config: `apps/${id}/formal/tla/Example.cfg`,
			claims: [`apps/${id}/README.md`],
			triggers: [`apps/${id}/src/**`],
		}),
	);
	return inventory;
}

test('inventory additions are checked and removals preserve surviving dependencies without global replay', () => {
	const previous = selectionInventory();
	const current = structuredClone(previous);
	current.models.push({ ...current.models[1], id: 'gamma' });
	assert.deepEqual(
		selectModelsForChangedPaths(current, ['scripts/tla/models.json'], previous).map(({ id }) => id),
		['gamma'],
	);
	const removed = structuredClone(previous);
	removed.models.pop();
	assert.deepEqual(selectModelsForChangedPaths(removed, ['scripts/tla/models.json'], previous), []);
	removed.models[0].triggers.push('apps/beta/src/**');
	assert.deepEqual(
		selectModelsForChangedPaths(
			removed,
			['scripts/tla/models.json', 'apps/beta/src/operation.rs'],
			previous,
		),
		removed.models,
	);
	for (const change of [
		(inventory) => {
			inventory.defaultTimeoutMilliseconds += 1;
		},
		(inventory) => {
			inventory.tool.version = 'changed';
		},
		(inventory) => {
			inventory.sharedTriggers.push('apps/additional/**');
		},
		(inventory) => {
			inventory.futureExecutionOption = true;
		},
	]) {
		const changed = structuredClone(previous);
		change(changed);
		assert.deepEqual(
			selectModelsForChangedPaths(changed, ['scripts/tla/models.json'], previous),
			changed.models,
		);
	}
	assert.deepEqual(
		selectModelsForChangedPaths(current, ['scripts/tla/models.json']),
		current.models,
	);
});

test('format-only inventory and known test entrypoint changes need only the inventory gate', () => {
	const inventory = selectionInventory();
	assert.deepEqual(
		selectModelsForChangedPaths(inventory, ['scripts/tla/models.json'], structuredClone(inventory)),
		[],
	);
	for (const name of ['model-inventory', 'model-checks', 'run-models']) {
		assert.deepEqual(selectModelsForChangedPaths(inventory, [`scripts/tla/${name}.test.mjs`]), []);
	}
	for (const path of ['scripts/tla/run-models.mjs', 'scripts/tla/new-runtime-helper.mjs']) {
		assert.deepEqual(
			selectModelsForChangedPaths(inventory, [path], structuredClone(inventory)),
			inventory.models,
		);
	}
});

function fixtureInventory() {
	return {
		defaultTimeoutMilliseconds: 1,
		models: [
			{
				claims: ['packages/example/README.md'],
				config: 'packages/example/formal/tla/Example.cfg',
				id: 'example',
				module: 'packages/example/formal/tla/Example.tla',
				owner: '@orchbee/example',
				requiredCoverage: ['Advance'],
				triggers: ['packages/example/src/**'],
				workspace: 'packages/example',
			},
		],
		schemaVersion: 1,
		sharedTriggers: ['scripts/tla/**'],
		tool: {
			downloadUrl: 'https://example.test/tla2tools.jar',
			sha256: 'a'.repeat(64),
			version: '1.0.0',
		},
	};
}

test('inventory validation rejects duplicate identities and missing coverage contracts', () => {
	const duplicate = fixtureInventory();
	duplicate.models.push({ ...duplicate.models[0] });
	assert.throws(() => validateInventory(duplicate), /duplicate model id/u);

	const missingCoverage = fixtureInventory();
	missingCoverage.models[0].requiredCoverage = [];
	assert.throws(() => validateInventory(missingCoverage), /requiredCoverage must not be empty/u);
});

test('inventory rejects malformed shared and model trigger paths before narrowing selection', () => {
	for (const trigger of [
		'apps\\alpha\\src\\**',
		'/apps/alpha/src/**',
		'C:/apps/alpha/src/**',
		'../apps/alpha/src/**',
		'apps/alpha/../src/**',
		'apps/alpha/..',
		'./apps/alpha/src/**',
		'apps//alpha/src/**',
		'apps/./alpha/src/**',
		'C:apps/alpha/src/**',
	]) {
		for (const shared of [false, true]) {
			const inventory = fixtureInventory();
			if (shared) inventory.sharedTriggers = [trigger];
			else inventory.models[0].triggers = [trigger];
			assert.throws(
				() => validateInventory(inventory),
				/repository-relative|forward slashes|stay in the repository|canonical repository path segments/u,
				trigger,
			);
		}
	}
	assert.doesNotThrow(() => validateInventory(fixtureInventory()));
});

test('every shared and model trigger must match a tracked file', () => {
	const inventory = fixtureInventory();
	inventory.models[0].triggers.push('packages/example/src/presentation*');
	const tracked = ['packages/example/src/model.ts', 'scripts/tla/run-models.mjs'];
	assert.throws(
		() => assertTriggersMatchTrackedFiles(inventory, tracked),
		(error) =>
			error.message.includes('- example: packages/example/src/presentation*') &&
			!error.message.includes('packages/example/src/**'),
	);
	inventory.sharedTriggers.push('docs/MOVED.md');
	assert.throws(
		() => assertTriggersMatchTrackedFiles(inventory, tracked),
		/- sharedTriggers: docs\/MOVED\.md/u,
	);
	assert.doesNotThrow(() =>
		assertTriggersMatchTrackedFiles(inventory, [
			...tracked,
			'packages/example/src/presentation.ts',
			'docs/MOVED.md',
		]),
	);
});

test('inventory validation allows one to eight model-local TLC workers and defaults by omission', () => {
	assert.doesNotThrow(() => validateInventory(fixtureInventory()));
	for (const workers of [1, 2, 7, 8]) {
		const inventory = fixtureInventory();
		inventory.models[0].workers = workers;
		assert.doesNotThrow(() => validateInventory(inventory));
	}
	for (const workers of [0, 9, -1, 1.5, 2.5, '2', '8', null, NaN, Infinity]) {
		const inventory = fixtureInventory();
		inventory.models[0].workers = workers;
		assert.throws(
			() => validateInventory(inventory),
			/inventory\.models\[0\]\.workers must be an integer between 1 and 8/u,
		);
	}
});

test('inventory validation rejects a module or configuration registered more than once', () => {
	const duplicateModule = fixtureInventory();
	duplicateModule.models.push({
		...duplicateModule.models[0],
		config: 'packages/example/formal/tla/ExampleCopy.cfg',
		id: 'example-copy',
	});
	assert.throws(() => validateInventory(duplicateModule), /duplicate model module/u);

	const duplicateConfig = fixtureInventory();
	duplicateConfig.models.push({
		...duplicateConfig.models[0],
		id: 'example-copy',
		module: 'packages/example/formal/tla/ExampleCopy.tla',
	});
	assert.throws(() => validateInventory(duplicateConfig), /duplicate model config/u);
});

test('inventory validation constrains explicit supporting modules', () => {
	const repeatedModule = fixtureInventory();
	repeatedModule.models[0].supportingModules = [repeatedModule.models[0].module];
	assert.throws(
		() => validateInventory(repeatedModule),
		/supportingModules\[\] must not repeat its model module/u,
	);

	const remoteModule = fixtureInventory();
	remoteModule.models[0].supportingModules = ['packages/other/formal/tla/Other.tla'];
	assert.throws(
		() => validateInventory(remoteModule),
		/supportingModules\[\] must be alongside its model module/u,
	);
});

test('changed-path selection is model-scoped and shared infrastructure selects every model', () => {
	const inventory = validateInventory(fixtureInventory());
	assert.deepEqual(
		selectModelsForChangedPaths(inventory, ['packages/example/src/example.ts']).map(({ id }) => id),
		['example'],
	);
	assert.deepEqual(
		selectModelsForChangedPaths(inventory, ['packages/unrelated/src/example.ts']),
		[],
	);
	assert.deepEqual(
		selectModelsForChangedPaths(inventory, ['scripts/tla/run-models.mjs']).map(({ id }) => id),
		['example'],
	);
	assert.equal(globMatches('packages/*/README.md', 'packages/example/README.md'), true);

	const inventoryWithSupport = fixtureInventory();
	inventoryWithSupport.models[0].supportingModules = [
		'packages/example/formal/tla/ExampleAuthority.tla',
	];
	assert.deepEqual(
		selectModelsForChangedPaths(validateInventory(inventoryWithSupport), [
			'packages/example/formal/tla/ExampleAuthority.tla',
		]).map(({ id }) => id),
		['example'],
	);
});

test('configuration variants replace one declared constant and reject ambiguous input', () => {
	assert.equal(
		variantConfigFileName('formal/tla/ActionExecution.cfg', 'reject-duplicate'),
		'ActionExecution-reject-duplicate.cfg',
	);
	assert.equal(
		applyConfigConstants(
			'SPECIFICATION Spec\nCONSTANT Scenario = "first"\nCONSTANT ExecutionCount = 2\n',
			{ ExecutionCount: 3, Scenario: 'second' },
		),
		'SPECIFICATION Spec\nCONSTANT Scenario = "second"\nCONSTANT ExecutionCount = 3\n',
	);
	assert.throws(
		() => applyConfigConstants('SPECIFICATION Spec\n', { Scenario: 'missing' }),
		/exactly one assignment/u,
	);
});

test('coverage accepts generated successors even when every state was already discovered', () => {
	assert.doesNotThrow(() =>
		assertRequiredCoverage(
			'example',
			['<RestoreServerConnection line 711>: 0:141,180'],
			['RestoreServerConnection'],
		),
	);
	assert.doesNotThrow(() =>
		assertRequiredCoverage('example', ['<Advance line 1>: 1'], ['Advance']),
	);
});

test('coverage enforcement rejects missing and zero-generated semantic actions', () => {
	for (const counts of ['0:0', '0', '1:0']) {
		assert.throws(
			() => assertRequiredCoverage('example', [`<Advance line 1>: ${counts}`], ['Advance']),
			/unreached markers/u,
		);
	}
	assert.throws(
		() =>
			assertRequiredCoverage(
				'example',
				['line 1, col 1 to line 2, col 1 of module Example: 100:200'],
				['Advance'],
			),
		/missing markers/u,
	);
	assert.throws(
		() => assertRequiredCoverage('example', ['<Different line 1>: 1:1'], ['Advance']),
		/missing markers/u,
	);
});

test('state reports retain the final TLC search summary', () => {
	assert.deepEqual(
		parseStateReport(
			'1 states generated, 1 distinct states found, 1 states left on queue.\n' +
				'10 states generated, 7 distinct states found, 0 states left on queue.\n' +
				'The depth of the complete state graph search is 4.',
		),
		{ depth: 4, distinctStates: 7, generatedStates: 10, statesLeft: 0 },
	);
});

test('TLC exit classification preserves an elapsed timeout when Java exits with status 143', () => {
	assert.match(
		tlcExitError('example', 120_000, { code: 143, signal: null, timedOut: true }).message,
		/exceeded its 120000ms timeout/u,
	);
	assert.match(
		tlcExitError('example', 120_000, { code: 143, signal: null, timedOut: false }).message,
		/exited with status 143/u,
	);
	assert.equal(
		tlcExitError('example', 120_000, { code: 0, signal: null, timedOut: false }),
		undefined,
	);
});

test('tooling and trigger changes recheck the compact full baseline without a workflow metamodel', () => {
	for (const file of [
		'scripts/tla/run-models.mjs',
		'scripts/tla/new-helper.mjs',
		'docs/VERIFICATION.md',
		'docs/MODELING-AND-CORRECTNESS.md',
	])
		assert.deepEqual(
			selectModelsForChangedPaths(repositoryInventory, [file]),
			repositoryInventory.models,
		);
	const previous = selectionInventory();
	const current = structuredClone(previous);
	current.models[0].triggers = ['apps/replacement/src/**'];
	for (const file of [
		'scripts/tla/models.json',
		'apps/alpha/src/operation.rs',
		'apps/replacement/src/operation.rs',
	])
		assert.deepEqual(selectModelsForChangedPaths(current, [file], previous), current.models);
	assert.deepEqual(
		selectModelsForChangedPaths(repositoryInventory, ['package.json', 'pnpm-lock.yaml']),
		[],
	);
});

test('direct model inputs and claims retain each protocol owner', () => {
	for (const model of repositoryInventory.models) {
		for (const changed of [model.module, model.config, ...model.claims])
			assert.ok(
				selectModelsForChangedPaths(repositoryInventory, [changed]).some((m) => m.id === model.id),
				changed,
			);
	}
});

test('protocol owner paths select their core while UI copy and deterministic local flows use implementation tests', async () => {
	const cases = [
		[
			'apps/orchbee/crates/workbench-client/src/terminal_viewport/window.rs',
			'native-terminal-viewport',
		],
		[
			'apps/orchbee/crates/workbench-client/src/workbench_state/membership.rs',
			'native-client-pane-membership',
		],
		['apps/orchbee-server/src/local_terminal_service/configuration.rs', 'native-settings-save'],
		[
			'apps/orchbee-server/src/local_terminal_service/viewer_connection.rs',
			'connection-viewer-routing',
		],
		[
			'apps/orchbee-server/src/local_client_service/application_connection.rs',
			'connection-viewer-routing',
		],
		[
			'apps/orchbee-server/src/local_terminal_service/viewer_connection/routing.rs',
			'local-terminal-relay',
		],
		['apps/orchbee-server/src/terminal_coordinator.rs', 'local-terminal-coordinator'],
		['apps/orchbee-worker/src/terminal_send_operation.rs', 'worker-send-transaction'],
		['apps/terminal-host/src/service/projection.zig', 'terminal-host-delivery'],
		['apps/terminal-host/src/platform/process_posix.zig', 'terminal-host-process-identity'],
		['apps/orchbee/src/local_startup.rs', 'local-session-startup'],
		['crates/local-runtime/src/checkpoint_rotation.rs', 'windows-checkpoint-storage'],
		['crates/local-runtime/src/tests.rs', 'windows-checkpoint-storage'],
		['crates/local-runtime/src/tests/session_store.rs', 'windows-checkpoint-storage'],
		['crates/local-runtime/src/tests/durable_sessions.rs', 'windows-checkpoint-storage'],
		['crates/local-runtime/src/tests/named_catalog.rs', 'windows-checkpoint-storage'],
		['crates/local-runtime/src/process_registration.rs', 'session-crash-recovery'],
		[
			'apps/web/src/workbench-client/terminal-projection/terminal-projection.ts',
			'projected-terminal-ipc-reliability',
		],
	];
	for (const [file, id] of cases) {
		await readFile(`${repositoryRoot}/${file}`);
		assert.ok(
			selectModelsForChangedPaths(repositoryInventory, [file]).some((m) => m.id === id),
			file,
		);
	}
	for (const file of [
		'apps/orchbee/crates/workbench-client/src/localization.rs',
		'apps/orchbee/crates/workbench-client/src/localization/action_menu.rs',
		'apps/orchbee/crates/workbench-client/src/tui_render/action_menu.rs',
		'packages/terminal/src/search/projected-search-controller.ts',
		'apps/orchbee/src/cli/creation.rs',
	])
		assert.deepEqual(selectModelsForChangedPaths(repositoryInventory, [file]), [], file);
});

test('source exclusions cannot suppress direct inputs or the previous owner in the same diff', () => {
	const previous = fixtureInventory();
	const current = structuredClone(previous);
	const file = 'packages/example/src/presentation.ts';
	current.models[0].triggerExclusions = [file];
	validateInventory(current);
	assert.deepEqual(selectModelsForChangedPaths(current, [file]), []);
	assert.equal(selectModelsForChangedPaths(current, [file], previous).length, 1);
	for (const direct of [
		current.models[0].module,
		current.models[0].config,
		current.models[0].claims[0],
	]) {
		const invalid = structuredClone(current);
		invalid.models[0].triggerExclusions = [direct];
		assert.throws(() => validateInventory(invalid), /exclu/iu);
	}
});
