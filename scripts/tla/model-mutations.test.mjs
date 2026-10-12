import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test, { describe } from 'node:test';

import { inventoryPath, repositoryRoot } from './model-inventory.mjs';
import { provisionJar } from './toolchain.mjs';
import { runChild } from './test-support/child-process.mjs';

const inventory = JSON.parse(await readFile(inventoryPath, 'utf8'));
const jar =
	process.env.ORCHBEE_TLA2TOOLS_JAR ??
	path.join(repositoryRoot, '.tools', `tla2tools-${inventory.tool.version}.jar`);
await provisionJar(inventory.tool, jar, process.env.ORCHBEE_TLA2TOOLS_JAR === undefined);

// Each mutation drops a protocol guard, leaving the checked property and bounds intact.
// The full baseline gate supplies the positive controls from the exact same modules/configs.
const cases = [
	[
		'native-settings-save',
		'stale file overwrite',
		'/\\ s.phase[c] = "owned" /\\ s.owner = c /\\ Fresh(c)',
		'/\\ s.phase[c] = "owned" /\\ s.owner = c',
		/Invariant OtherSettingsPreserved is violated/u,
	],
	[
		'native-settings-save',
		'legacy import resurrects an explicit reset',
		'value == IF k \\in s.importClaimed THEN s.importValues[k] ELSE 1',
		'value == 1',
		/Invariant ResetSurvivesMigration is violated/u,
	],
	[
		'session-viewer-routing',
		'late cleanup releases successor',
		'after == IF s.grant[target] = v THEN 0 ELSE s.grant[target]',
		'after == 0',
		/Invariant StaleCleanupPreservesSuccessor is violated/u,
	],
	[
		'native-client-pane-membership',
		'stale completion affects replacement',
		'accepted == Current(s.flight)',
		'accepted == TRUE',
		/Invariant (?:FramesAreCurrent|EffectsUseCurrentIncarnation) is violated/u,
	],
	[
		'local-terminal-relay',
		'feedback acknowledges another flight',
		'/\\ identity = s.frameFlight[c]',
		'/\\ identity \\in 1..s.frameSource',
		/Action property FeedbackUsesExactFlight is violated/u,
	],
	[
		'native-terminal-viewport',
		'independent outcome producer',
		'/\\ s.outcome.kind = "window" /\\ s.outcome.sequence # 0\n  /\\ s\' = [s EXCEPT !.outcome.sequence = 0]',
		'/\\ s.outcome = None\n  /\\ s\' = [s EXCEPT !.outcome = Command("window", 0)]',
		/Action property OutcomeProductionOwned is violated/u,
	],
	[
		'desktop-workbench-document-store',
		'commit bypasses revision comparison',
		'ELSE IF storedRevision # expectedRevision[client]',
		'ELSE IF FALSE',
		/Action property (?:RevisionNeverDecreases|CommitAdvancesExactlyOne) is violated/u,
	],
];

describe('protocol mutation counterexamples', { concurrency: 3 }, () => {
	for (const [id, fault, before, after, counterexample] of cases) {
		test(`${id}: ${fault}`, async (t) => {
			const model = inventory.models.find((entry) => entry.id === id);
			const source = (await readFile(path.join(repositoryRoot, model.module), 'utf8')).replaceAll(
				'\r\n',
				'\n',
			);
			assert.equal(source.split(before).length, 2, `${id}: mutation must name one guard`);
			const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-mutation-'));
			t.after(() => rm(directory, { recursive: true, force: true }));
			const module = path.basename(model.module);
			const config = path.basename(model.config);
			await writeFile(path.join(directory, module), source.replace(before, after));
			await writeFile(
				path.join(directory, config),
				await readFile(path.join(repositoryRoot, model.config)),
			);
			const result = await runChild(
				process.env.ORCHBEE_TLA_JAVA ?? 'java',
				[
					'-Xmx512m',
					'-XX:+UseParallelGC',
					`-Djava.io.tmpdir=${directory}`,
					'-jar',
					jar,
					'-workers',
					'1',
					'-fp',
					'0',
					'-lncheck',
					'final',
					'-metadir',
					path.join(directory, 'states'),
					'-config',
					config,
					module,
				],
				{ cwd: directory, encoding: 'utf8', timeout: 15_000 },
			);
			assert.ifError(result.error);
			assert.notEqual(result.status, 0, 'The seeded protocol defect must be detected');
			assert.match(result.stdout + result.stderr, counterexample);
		});
	}
});
