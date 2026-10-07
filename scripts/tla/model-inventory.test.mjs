import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
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

test('future proposals remain bilingual broad dependencies without current claim authority', () => {
	for (const id of [
		'local-site-activation',
		'local-terminal-client-authority',
		'local-terminal-relay',
		'local-host-broker',
	]) {
		const model = repositoryInventory.models.find((entry) => entry.id === id);
		assert.ok(model.claims.every((claim) => !claim.startsWith('proposals/')));
		for (const trigger of model.triggers.filter(
			(entry) => entry.startsWith('proposals/') && !entry.endsWith('.zh-CN.md'),
		)) {
			const counterpart = trigger.replace(/\.md$/u, '.zh-CN.md');
			assert.ok(model.triggers.includes(counterpart), `${id}: ${counterpart}`);
			for (const changedPath of [trigger, counterpart])
				assert.ok(
					selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
						(entry) => entry.id === id,
					),
				);
		}
	}
});

test('viewer cleanup representatives cover all admission/reservation assignments under renaming', async () => {
	const model = repositoryInventory.models.find(({ id }) => id === 'session-viewer-routing');
	const cleanup = model.variants.filter(({ constants }) => constants.Scenario === 'cleanup');
	const classes = new Map();
	for (const firstSession of ['a', 'b']) {
		for (const firstTarget of ['x', 'y']) {
			for (const secondSession of ['a', 'b']) {
				for (const secondTarget of ['x', 'y']) {
					for (const firstGrantViewer of [1, 2]) {
						// Unique permutations sending the first Session/target to a/x.
						const session = secondSession === firstSession ? 'a' : 'b';
						const target = secondTarget === firstTarget ? 'x' : 'y';
						const name = `cleanup-a-x-${session}-${target}-g${firstGrantViewer}`;
						const members = classes.get(name) ?? [];
						members.push([
							firstSession,
							firstTarget,
							secondSession,
							secondTarget,
							firstGrantViewer,
						]);
						classes.set(name, members);
					}
				}
			}
		}
	}
	assert.equal(classes.size, 8);
	assert.deepEqual(cleanup.map(({ name }) => name).toSorted(), [...classes.keys()].toSorted());
	for (const { name, constants } of cleanup) {
		const members = classes.get(name);
		assert.equal(members.length, 4);
		const representative = members.find(([session, target]) => session === 'a' && target === 'x');
		const [FirstSession, FirstTarget, SecondSession, SecondTarget, FirstGrantViewer] =
			representative;
		assert.deepEqual(constants, {
			Scenario: 'cleanup',
			FirstSession,
			FirstTarget,
			SecondSession,
			SecondTarget,
			FirstGrantViewer,
		});
	}
	assert.deepEqual(
		model.variants.filter(({ constants }) => constants.Scenario !== 'cleanup'),
		[
			{ name: 'routing', constants: { Scenario: 'routing', MaxRoutingHistory: 1 } },
			{ name: 'control', constants: { Scenario: 'control' } },
		],
	);
	const config = await readFile(new URL(`../../${model.config}`, import.meta.url), 'utf8');
	for (const { constants } of model.variants) {
		const applied = applyConfigConstants(config, constants);
		assert.equal(
			applied.slice(applied.indexOf('CHECK_DEADLOCK')),
			config.slice(config.indexOf('CHECK_DEADLOCK')),
		);
	}
});

test('retired membership representatives preserve every ordinal and allocator seed class', () => {
	const model = repositoryInventory.models.find(
		({ id }) => id === 'native-client-retired-membership-recovery',
	);
	// This finite decision table classifies the reviewed model's input domain.
	// The graph-bijection argument belongs to native-tab-session.md, not this table.
	const baselines = [['A'], ['A', 'B', 'C'], ['B', 'A', 'C'], ['B', 'C', 'A']];
	const candidates = [[], ['B'], ['C'], ['B', 'C'], ['C', 'B']];
	const selections = baselines.map((baseline) =>
		candidates.map(
			(candidate) => candidate[Math.min(baseline.indexOf('A') + 1, candidate.length) - 1] ?? 'none',
		),
	);
	assert.deepEqual(selections[0], ['none', 'B', 'C', 'B', 'C']);
	assert.deepEqual(selections[1], selections[0]);
	assert.deepEqual(selections[2], ['none', 'B', 'C', 'C', 'B']);
	assert.deepEqual(selections[3], selections[2]);
	const seedClasses = ['normal', 'near-limit', 'exhausted'];
	const representatives = [
		{ prefix: 'singleton', ordinal: 0 },
		{ prefix: 'middle', ordinal: 2 },
	];
	assert.deepEqual(
		model.variants,
		representatives.flatMap(({ prefix, ordinal }) =>
			seedClasses.map((SeedClass) => ({
				name: `${prefix}-${SeedClass}`,
				constants: { InitialOrdinal: ordinal, SeedClass },
			})),
		),
	);
	for (const { constants } of model.variants) {
		const matchingOrdinals = selections.flatMap((selection, ordinal) =>
			selection.every((value, index) => value === selections[constants.InitialOrdinal][index])
				? [ordinal]
				: [],
		);
		assert.deepEqual(matchingOrdinals, constants.InitialOrdinal === 0 ? [0, 1] : [2, 3]);
	}
});

for (const [id, digest, authority] of [
	[
		'session-viewer-routing',
		'0e6f21c5803a2b5df539124b6c1f0f79a9f80548bd72f85cc9a066ab62924f0d',
		'apps/orchbee-server/README.md#accepted-session-viewer-routing-modeling-record',
	],
	[
		'native-client-retired-membership-recovery',
		'bedc926caa171bba886c1d79008a971cc576a4fa7308e741af5b276ba9d661c7',
		'apps/orchbee/docs/native-tab-session.md#retired-peer-membership-recovery',
	],
]) {
	test(`${id} configuration equivalence remains bound to its reviewed model and checks`, async () => {
		const model = repositoryInventory.models.find((entry) => entry.id === id);
		assert.deepEqual(model.supportingModules ?? [], []);
		const contents = await Promise.all(
			[model.module, model.config].map(async (file) =>
				(await readFile(new URL(`../../${file}`, import.meta.url), 'utf8')).replaceAll(
					'\r\n',
					'\n',
				),
			),
		);
		const inputs = [
			...contents,
			model.requiredCoverage,
			model.workers ?? 1,
			model.timeoutMilliseconds ?? repositoryInventory.defaultTimeoutMilliseconds,
		];
		// This tripwire requires semantic review; matching hashes do not prove equivalence.
		assert.equal(
			createHash('sha256').update(JSON.stringify(inputs)).digest('hex'),
			digest,
			`Recheck the complete equivalence argument in ${authority} before updating this digest, or restore the full configuration family.`,
		);
	});
}

test('native Tab barriers follow Client state, transport, presentation, and protocol owners', () => {
	const expected = [
		'native-client-close-control',
		'native-client-close-handoff',
		'native-client-create-barrier',
		'native-client-refresh-fence',
		'native-tab-switch',
	];
	for (const changedPath of [
		'apps/orchbee/docs/native-tab-session.md',
		'apps/orchbee/docs/native-tab-session.zh-CN.md',
		'apps/orchbee/src/terminal_session.rs',
		'apps/orchbee/src/terminal_session/workbench_projection.rs',
		'apps/orchbee/src/terminal_session/mailbox.rs',
		'apps/orchbee/src/terminal_session/state.rs',
		'apps/orchbee/src/terminal_session/connection.rs',
		'apps/orchbee/src/local_client/workbench.rs',
		'apps/orchbee/src/local_client.rs',
		'apps/orchbee/src/client_view.rs',
		'apps/orchbee/src/terminal_input.rs',
		'apps/orchbee/src/tui_shell.rs',
		'apps/orchbee/src/tui_layout.rs',
		'apps/orchbee/src/tui_render.rs',
		'apps/orchbee/src/presentation.rs',
		'apps/orchbee/src/tui_backend.rs',
		'apps/orchbee/src/localization.rs',
		'apps/orchbee/src/terminal_device.rs',
		'apps/orchbee/src/terminal_projection.rs',
		'apps/orchbee/src/terminal_lifecycle.rs',
		'apps/orchbee/src/termination_signal.rs',
		'apps/orchbee/src/local_startup.rs',
		'apps/orchbee/tests/local_startup_process.rs',
		'crates/workbench-protocol/src/command.rs',
		'crates/client-server-protocol/src/viewer.rs',
		'crates/terminal-protocol/src/text_frame/rows.rs',
		'schemas/workbench/v1/workbench-command.schema.json',
		'schemas/workbench/v1/workbench-command-conformance-v1.json',
		'schemas/client-server/v1/client-server.schema.json',
	]) {
		const selected = selectModelsForChangedPaths(repositoryInventory, [changedPath]);
		for (const id of expected) {
			assert.ok(
				selected.some((model) => model.id === id),
				`${changedPath} must select ${id}`,
			);
		}
	}
	for (const changedPath of [
		'apps/web/src/settings/AppearanceSettings.tsx',
		'packages/workbench/src/workbench/Workbench.module.css',
	]) {
		const selected = selectModelsForChangedPaths(repositoryInventory, [changedPath]);
		assert.ok(
			selected.every((model) => !expected.includes(model.id)),
			changedPath,
		);
	}
});

test('dependency bookkeeping uses the cheap inventory gate while TLC infrastructure selects all models', () => {
	assert.deepEqual(
		selectModelsForChangedPaths(repositoryInventory, ['package.json', 'pnpm-lock.yaml']),
		[],
	);
	for (const changedPath of [
		'scripts/tla/model-checks.mjs',
		'.github/workflows/verify.yml',
		'docs/VERIFICATION.md',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).length,
			repositoryInventory.models.length,
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

test('inventory entry changes select that entry and retain both generations of owner triggers', () => {
	const previous = selectionInventory();
	const current = structuredClone(previous);
	current.models[0].triggers = ['apps/replacement/src/**'];
	const identifiers = (paths) =>
		selectModelsForChangedPaths(current, paths, previous).map(({ id }) => id);
	assert.deepEqual(identifiers(['scripts/tla/models.json']), ['alpha']);
	assert.deepEqual(identifiers(['scripts/tla/models.json', 'apps/beta/src/operation.rs']), [
		'alpha',
		'beta',
	]);
	assert.deepEqual(identifiers(['apps/alpha/src/operation.rs']), ['alpha']);
	assert.deepEqual(identifiers(['apps/replacement/src/operation.rs']), ['alpha']);
	assert.deepEqual(
		identifiers(['scripts/tla/models.json', 'scripts/tla/model-inventory.test.mjs']),
		['alpha'],
	);
});

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

test('scoped lifecycle triggers retain semantic owners without replaying unrelated settings', () => {
	const cases = [
		[
			'desktop-lifecycle',
			'apps/desktop/src/main/application-lifetime/DesktopApplicationLifetimeBroker.ts',
		],
		[
			'desktop-terminal-archive',
			'apps/desktop/src/main/terminal-archive/TerminalArchiveCaptureService.ts',
		],
		[
			'desktop-terminal-lifecycle-adapter',
			'apps/desktop/src/main/terminal-document-control/DesktopTerminalLifecycleAdapter.ts',
		],
		['desktop-terminal-metadata-delivery', 'apps/desktop/src/main/host/HostConnection.ts'],
		[
			'desktop-workbench-document-store',
			'apps/desktop/src/main/workbench-document/decideWorkbenchDocumentCommit.ts',
		],
		[
			'desktop-workbench-write-ahead',
			'apps/desktop/src/main/terminal-document-control/DesktopTerminalIntentReconciler.ts',
		],
		['terminal-exit-recovery', 'apps/desktop/src/main/recovery/TerminalExitSpillStore.ts'],
		['terminal-recovery-binding', 'apps/desktop/src/main/recovery/TerminalRecoveryCoordinator.ts'],
		[
			'terminal-presentation-removal',
			'apps/desktop/src/main/terminal-lifetime/TerminalCloseOrchestrator.ts',
		],
		[
			'terminal-presentation-stop',
			'apps/desktop/src/main/terminal-document-control/DesktopTerminalLifecycleAdapter.ts',
		],
	];
	const unrelated = new Set(
		selectModelsForChangedPaths(repositoryInventory, [
			'apps/desktop/src/main/terminal-default-settings/DesktopTerminalDefaultSettingsController.ts',
		]).map(({ id }) => id),
	);
	for (const [id, changedPath] of cases) {
		assert.ok(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				(model) => model.id === id,
			),
			changedPath,
		);
		assert.equal(unrelated.has(id), false, id);
	}
});

test('terminal concurrency models follow input, renderer, row and wire owners', () => {
	const cases = [
		['terminal-input-repair', 'packages/terminal/src/input/terminal-input-state.ts'],
		['terminal-native-input-settlement', 'packages/terminal/src/projected/projected-terminal.ts'],
		['terminal-presentation-configuration', 'packages/terminal/src/rendering/renderer.ts'],
		['terminal-row-flight-coordination', 'packages/terminal/src/buffer-rows.ts'],
	];
	for (const [id, changedPath] of cases) {
		assert.ok(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				(model) => model.id === id,
			),
			changedPath,
		);
		assert.ok(
			selectModelsForChangedPaths(repositoryInventory, [
				'packages/terminal-protocol/src/index.ts',
			]).some((model) => model.id === id),
			id,
		);
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [
				'packages/terminal/src/ghostty-sprites.ts',
			]).some((model) => model.id === id),
			false,
			id,
		);
	}
	assert.ok(
		selectModelsForChangedPaths(repositoryInventory, [
			'apps/web/src/workbench-client/projected-terminal-renderer/createTerminalThemePresentationParticipant.ts',
		]).some(({ id }) => id === 'terminal-theme-revision'),
	);
	assert.equal(
		selectModelsForChangedPaths(repositoryInventory, [
			'apps/web/src/agent-integration-reminder/AgentIntegrationReminder.tsx',
		]).some(({ id }) => id === 'terminal-theme-revision'),
		false,
	);
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

test('Workbench presentation-only edits do not replay document and terminal models', () => {
	for (const changedPath of [
		'packages/workbench/src/workbench/Workbench.module.css',
		'packages/workbench/src/workbench/components/workbench-tab-bar/WorkbenchTabBar.module.css',
	]) {
		assert.deepEqual(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]),
			[],
			changedPath,
		);
	}
});

test('Desktop session and resize triggers retain their semantic owners and omit unrelated settings', () => {
	const cases = {
		'desktop-multi-client-session': [
			'apps/desktop/src/main/client-session/decideDesktopClientDocumentProposal.ts',
			'apps/desktop/src/main/workbench-document/FileWorkbenchDocumentStore.ts',
			'apps/desktop/src/main/client-session/DesktopTerminalIntentReconciler.ts',
			'packages/workbench/src/project-workspace/index.ts',
			'packages/workbench/src/terminal-presentation/terminal-presentation-contracts.ts',
			'packages/workbench/src/workbench/document/workbench-document.ts',
		],
		'desktop-terminal-resize-lease': [
			'apps/desktop/src/main/terminal-projection-control/DesktopTerminalProjectionService.ts',
			'apps/desktop/src/main/broker/ProjectedTerminalBroker.ts',
			'apps/desktop/src/main/host/HostConnection.ts',
			'apps/web/src/workbench-client/terminal-projection/terminal-projection.ts',
		],
	};
	for (const [id, paths] of Object.entries(cases)) {
		for (const changedPath of [
			...paths,
			'apps/desktop/src/main/composition/composeDesktopTerminalRuntime.ts',
			'packages/workbench-client-protocol/src/message.ts',
		]) {
			assert.ok(
				selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
					(model) => model.id === id,
				),
				`${id}: ${changedPath}`,
			);
		}
		assert.ok(
			!selectModelsForChangedPaths(repositoryInventory, [
				'apps/desktop/src/main/terminal-default-settings/DesktopTerminalDefaultSettingsController.ts',
			]).some((model) => model.id === id),
			id,
		);
	}
});

test('repaired Desktop boundaries select the coordinating lifecycle and session models', () => {
	const cases = [
		['apps/desktop/rsbuild.config.ts', ['desktop-lifecycle']],
		[
			'apps/web/src/desktop-client/project-folder-acquisition/DesktopProjectFolderAcquisitionClient.ts',
			['desktop-lifecycle'],
		],
		[
			'packages/desktop-client-protocol/src/desktop-application-lifetime-contract.ts',
			['desktop-lifecycle'],
		],
		[
			'apps/desktop/src/main/terminal-binding-registry/DesktopTerminalBindingRegistry.ts',
			['desktop-lifecycle'],
		],
		[
			'apps/desktop/src/main/project-worktree/DesktopProjectWorktreeRuntime.ts',
			['desktop-multi-client-session'],
		],

		[
			'apps/web/src/workbench-client/client-session/WorkbenchClientSessionClient.ts',
			['desktop-multi-client-session', 'desktop-workbench-document-proposal-acknowledgement'],
		],
		[
			'packages/workbench/src/terminal-presentation/terminal-presentation-contracts.ts',
			['desktop-terminal-intent-reconciliation'],
		],
		[
			'packages/workbench/src/workbench/model/WorkbenchProjectWorkspaceRuntime.ts',
			['desktop-workbench-document-proposal-acknowledgement'],
		],
		[
			'packages/workbench-client-protocol/src/workbench-client-session-contract.ts',
			['desktop-multi-client-session', 'desktop-workbench-document-proposal-acknowledgement'],
		],
	];
	for (const [changedPath, expectedIds] of cases) {
		const selectedIds = new Set(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).map(({ id }) => id),
		);
		for (const expectedId of expectedIds) {
			assert.equal(selectedIds.has(expectedId), true, `${changedPath}: ${expectedId}`);
		}
	}
});

test('Host and projection resize owners select the terminal lifecycle adapter', () => {
	for (const changedPath of [
		'apps/desktop/src/main/host/HostConnection.ts',
		'apps/desktop/src/main/terminal-projection-control/DesktopTerminalProjectionService.ts',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'desktop-terminal-lifecycle-adapter',
			),
			true,
			changedPath,
		);
	}
});

test('Workbench code edits keep the conservative coverage of the narrowed source globs', () => {
	for (const changedPath of [
		'packages/workbench/src/index.ts',
		'packages/workbench/src/workbench/Workbench.tsx',
		'packages/workbench/src/workbench/hooks/useWorkbench.ts',
	]) {
		const selected = selectModelsForChangedPaths(repositoryInventory, [changedPath]);
		for (const id of [
			'desktop-terminal-archive',
			'desktop-workbench-document-store',
			'terminal-presentation-stop',
		]) {
			assert.ok(
				selected.some((model) => model.id === id),
				`${id}: ${changedPath}`,
			);
		}
	}
});

test('Workbench tab-close owners select the panel split model', () => {
	for (const changedPath of [
		'apps/orchbee-server/src/workbench_authority/terminal_tab_close.rs',
		'apps/orchbee-server/src/workbench_authority/terminal_tab_close/admission.rs',
		'apps/orchbee-server/src/workbench_authority/terminal_tab_close/publication.rs',
		'apps/orchbee-server/src/workbench_authority/terminal_tab_close/tests.rs',
		'apps/orchbee-server/src/workbench_authority/terminal_panel_placement.rs',
		'apps/orchbee-server/src/workbench_authority/terminal_panel_placement/tests.rs',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'workbench-panel-split',
			),
			true,
			changedPath,
		);
	}
	for (const changedPath of [
		'apps/orchbee-server/src/workbench_authority/terminal_tab_create.rs',
		'apps/orchbee-server/src/workbench_authority/terminal_tab_move.rs',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'workbench-panel-split',
			),
			false,
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
		'apps/orchbee-worker/formal/tla/WorkerTerminalBindingCapacity.cfg',
		'apps/orchbee-worker/formal/tla/WorkerTerminalBindingCapacity.tla',
		'apps/orchbee-worker/src/host_connection.rs',
		'apps/orchbee-worker/src/terminal_authorization.rs',
		'apps/orchbee-worker/tests/process.rs',
		'apps/terminal-host/docs/runtime-model.md',
		'apps/terminal-host/docs/runtime-model.zh-CN.md',
		'apps/terminal-host/formal/tla/TerminalHostRequestOutcome.tla',
		'apps/terminal-host/src/service/terminal_requests/creation.zig',
		'crates/capability-node-protocol/src/terminal_launch.rs',
		'crates/local-site-protocol/src/codec.rs',
		'crates/terminal-protocol/src/creation.rs',
		'packages/terminal-protocol/src/message.ts',
		'proposals/platform/native-runtime-topology.md',
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

test('local Client bootstrap owners select the Site activation model', () => {
	for (const changedPath of [
		'apps/orchbee/README.zh-CN.md',
		'apps/orchbee/project.json',
		'apps/orchbee/src/invocation.rs',
		'apps/orchbee/src/local_client.rs',
		'apps/orchbee/src/local_site_rendezvous.rs',
		'apps/orchbee/src/main.rs',
		'apps/orchbee/src/site_activation.rs',
		'apps/orchbee/tests/process.rs',
		'crates/local-client-protocol/src/client.rs',
		'proposals/cli/native-command-surface.zh-CN.md',
		'proposals/platform/native-runtime-topology.zh-CN.md',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'local-site-activation',
			),
			true,
			changedPath,
		);
	}
});

test('Server Worker OBCN carrier refinement paths select its carrier model', () => {
	for (const changedPath of [
		'apps/orchbee-server/Cargo.toml',
		'apps/orchbee-server/README.md',
		'apps/orchbee-server/README.zh-CN.md',
		'apps/orchbee-server/formal/tla/TerminalLaunchAuthorization.cfg',
		'apps/orchbee-server/formal/tla/TerminalLaunchAuthorization.tla',
		'apps/orchbee-server/project.json',
		'apps/orchbee-server/src/local_site_service.rs',
		'apps/orchbee-server/src/local_site_service/tests.rs',
		'apps/orchbee-server/src/main.rs',
		'apps/orchbee-server/src/worker_obcn_carrier.rs',
		'apps/orchbee-server/src/worker_obcn_carrier/tests.rs',
		'apps/orchbee-server/src/worker_registry.rs',
		'apps/orchbee-server/src/worker_registry/tests.rs',
		'apps/orchbee-server/tests/process.rs',
		'apps/orchbee-server/tests/process/worker_obcn_carrier.rs',
		'apps/orchbee-server/tests/worker_obcn_carrier.rs',
		'apps/orchbee-server/tests/worker_obcn_carrier/process.rs',
		'crates/capability-node-protocol/Cargo.toml',
		'crates/capability-node-protocol/README.md',
		'crates/capability-node-protocol/README.zh-CN.md',
		'crates/capability-node-protocol/project.json',
		'crates/capability-node-protocol/src/codec.rs',
		'crates/capability-node-protocol/tests/conformance.rs',
		'crates/local-site-protocol/Cargo.toml',
		'crates/local-site-protocol/README.md',
		'crates/local-site-protocol/README.zh-CN.md',
		'crates/local-site-protocol/project.json',
		'crates/local-site-protocol/src/codec.rs',
		'crates/local-site-protocol/tests/conformance.rs',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'local-worker-obcn-carrier',
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

test('viewer/controller protocol and routing owners select the terminal relay model', () => {
	for (const changedPath of [
		'apps/orchbee/src/terminal_projection.rs',
		'apps/orchbee/src/terminal_session/connection.rs',
		'apps/orchbee/src/tui_shell.rs',
		'apps/orchbee/src/tui_render/terminal_cells.rs',
		'crates/terminal-protocol/src/text_frame/rows.rs',
		'apps/orchbee-server/src/local_terminal_service/viewer_session.rs',
		'apps/orchbee-server/src/local_terminal_service/controller_session.rs',
		'apps/orchbee-worker/src/terminal_control_operation.rs',
		'crates/client-server-protocol/src/control.rs',
		'crates/capability-node-protocol/src/terminal_control.rs',
		'crates/capability-node-protocol/src/codec/control.rs',
		'apps/orchbee-worker/src/terminal_viewer_operation.rs',
		'crates/client-server-protocol/src/viewer.rs',
		'crates/client-server-protocol/src/viewer_record.rs',
		'crates/client-server-protocol/tests/viewer_record.rs',
		'crates/capability-node-protocol/src/codec/viewer.rs',
		'crates/terminal-protocol/src/frame_route.rs',
		'schemas/client-server/v1/client-server.schema.json',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'local-terminal-relay',
			),
			true,
			changedPath,
		);
	}
});

test('Panel swap workflow, carrier and canonical owners select the staged swap model', () => {
	for (const changedPath of [
		'apps/orchbee-server/src/workbench_authority/terminal_panel_placement.rs',
		'apps/orchbee-server/src/workbench_authority/terminal_panel_placement/tests/swap.rs',
		'apps/orchbee/src/local_client/workbench/panel_placement.rs',
		'apps/orchbee/src/cli/client_control/windows.rs',
		'apps/orchbee/src/cli/client_control/selection.rs',
		'apps/orchbee/src/cli/client_control/swap.rs',
		'apps/orchbee/src/cli/client_control/swap/receipt.rs',
		'apps/orchbee/tests/cli_windows_panel_swap.rs',
		'crates/workbench-protocol/src/command.rs',
		'packages/workbench-client-protocol/src/workbench-command-contract.ts',
		'schemas/workbench/v1/workbench-command.schema.json',
	]) {
		assert.equal(
			selectModelsForChangedPaths(repositoryInventory, [changedPath]).some(
				({ id }) => id === 'workbench-panel-swap',
			),
			true,
			changedPath,
		);
	}
});
