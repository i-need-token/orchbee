import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
	applyConfigConstants,
	assertRequiredCoverage,
	globMatches,
	inventoryPath,
	parseStateReport,
	selectModelsForChangedPaths,
	tlcExitError,
	validateInventory,
	variantConfigFileName,
} from './model-inventory.mjs';

const repositoryInventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));

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
});

test('projected terminal selection, Find, projection, and recovery owners select the IPC model', () => {
	for (const changedPath of [
		'apps/web/src/workbench-client/projected-terminal-actions/registerProjectedTerminalActions.ts',
		'apps/web/src/workbench-client/projected-terminal-surface/model/ProjectedTerminalRecoveryController.ts',
		'apps/web/src/workbench-client/terminal-find-surface/model/TerminalFindController.ts',
		'apps/web/src/workbench-client/terminal-projection/terminal-projection.ts',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'projected-terminal-ipc-reliability',
			),
			true,
			changedPath,
		);
	}
});

test('Worker terminal authorization paths select both refinement models', () => {
	for (const changedPath of [
		'apps/orchbee-worker/src/terminal_authorization.rs',
		'apps/orchbee-worker/src/terminal_authorization/tests.rs',
		'apps/orchbee-worker/README.zh-CN.md',
		'crates/capability-node-protocol/src/codec.rs',
	]) {
		const selected = selectModelsForChangedPaths(repositoryInventory, [changedPath]);
		for (const modelId of ['terminal-launch-authorization', 'worker-terminal-binding-capacity']) {
			assert.equal(
				selected.some(({ id }) => id === modelId),
				true,
				`${changedPath}: ${modelId}`,
			);
		}
	}
});

test('Worker Host-command adapter surfaces select the local Host broker model', () => {
	for (const changedPath of [
		'apps/orchbee-server/formal/tla/TerminalLaunchAuthorization.tla',
		'apps/orchbee-worker/README.zh-CN.md',
		'apps/orchbee-worker/src/host_connection.rs',
		'apps/orchbee-worker/tests/process.rs',
		'apps/terminal-host/src/service/terminal_requests/creation.zig',
		'crates/capability-node-protocol/src/terminal_launch.rs',
		'crates/local-site-protocol/src/codec.rs',
		'crates/terminal-protocol/src/creation.rs',
		'packages/terminal-protocol/src/message.ts',
		'proposals/platform/native-runtime-topology.zh-CN.md',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'local-host-broker',
			),
			true,
			changedPath,
		);
	}
});

test('Client Session and OBCS refinement paths select the OBCS Session model', () => {
	for (const changedPath of [
		'apps/orchbee-server/Cargo.toml',
		'apps/orchbee-server/README.zh-CN.md',
		'apps/orchbee-server/formal/tla/LocalClientObcsSessions.cfg',
		'apps/orchbee-server/formal/tla/LocalClientObcsSessions.tla',
		'apps/orchbee-server/formal/tla/LocalClientSessions.cfg',
		'apps/orchbee-server/formal/tla/LocalClientSessions.tla',
		'apps/orchbee-server/project.json',
		'apps/orchbee-server/src/local_client_service.rs',
		'apps/orchbee-server/tests/client_process.rs',
		'crates/client-server-protocol/Cargo.toml',
		'crates/client-server-protocol/README.zh-CN.md',
		'crates/client-server-protocol/project.json',
		'crates/client-server-protocol/src/codec.rs',
		'crates/client-server-protocol/tests/conformance.rs',
		'schemas/client-server/v1/client-server.schema.json',
		'schemas/client-server/v1/client-server-conformance-v1.json',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'local-client-sessions-obcs',
			),
			true,
			changedPath,
		);
	}
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

test('coverage enforcement rejects missing and zero-count semantic actions', () => {
	assert.doesNotThrow(() =>
		assertRequiredCoverage('example', ['<Advance line 1>: 0:1'], ['Advance']),
	);
	assert.throws(
		() => assertRequiredCoverage('example', ['<Advance line 1>: 0:0'], ['Advance']),
		/unreached markers/u,
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
