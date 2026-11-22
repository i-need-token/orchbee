import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test, { describe } from 'node:test';

import { applyConfigConstants, inventoryPath, repositoryRoot, tlcJar } from './run-models.mjs';
import { runChild } from './test-support/child-process.mjs';

const inventory = JSON.parse(await readFile(inventoryPath, 'utf8'));
const jar = await tlcJar(inventory);

// Each mutation drops a protocol guard, leaving the checked property and bounds intact.
// run-models.mjs runs the unmutated modules and configurations first as the positive controls.
const cases = [
	[
		'terminal-host-handover',
		'claim accepted without a mark for the claimant',
		'ClaimNeedsMark = TRUE',
		'ClaimNeedsMark = FALSE',
		/Invariant OneReader is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'resume succeeds after a claim',
		'ResumeRefusedAfterClaim = TRUE',
		'ResumeRefusedAfterClaim = FALSE',
		/Invariant OneReader is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'mark sent before the reader parks',
		'MarkAfterPark = TRUE',
		'MarkAfterPark = FALSE',
		/Invariant RecordsEndAtGround is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'new Host reads before it has seen claimed',
		'ReadAfterClaimed = TRUE',
		'ReadAfterClaimed = FALSE',
		/Invariant OnlyClaimCommits is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'old Host resumes at its deadline without the Keeper answer',
		'ResumeNeedsAnswer = TRUE',
		'ResumeNeedsAnswer = FALSE',
		/Invariant OneReader is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'claim accepted under an earlier mark',
		'ClaimNeedsCurrentMark = TRUE',
		'ClaimNeedsCurrentMark = FALSE',
		/Invariant NoLossOrRepeat is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'quiesced terminal held again in a second Keeper',
		'HoldOnlyRead = TRUE',
		'HoldOnlyRead = FALSE',
		/Invariant HoldingsNameReaders is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'reader parks inside a sequence',
		'ParkAtGround = TRUE',
		'ParkAtGround = FALSE',
		/Invariant RecordsEndAtGround is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'drop sent once the old Host has seen claimed but before the new Host has',
		'DropAfterNewClaimed = TRUE',
		'DropAfterNewClaimed = FALSE',
		/Invariant ReaderRemains is violated/u,
		'config',
	],
	[
		'terminal-host-handover',
		'an orphan assigned while its owner still reads',
		'AssignNeedsOrphan = TRUE',
		'AssignNeedsOrphan = FALSE',
		/Invariant OneReader is violated/u,
		'config',
	],
	[
		'release-store-fallback',
		'a recorder writes from the pointer it read before the lock',
		'LockedUpdate = TRUE',
		'LockedUpdate = FALSE',
		/(?:Invariant NoLostRecord|Action property MoveNeverReverted) is violated/u,
		'config',
	],
	[
		'release-store-fallback',
		'a failure counts toward a move it does not belong to',
		'BindRecord = TRUE',
		'BindRecord = FALSE',
		/Invariant RecordBound is violated/u,
		'config',
	],
	[
		'release-store-fallback',
		'the fallback holds the failed version in a later write',
		'HoldInSameWrite = TRUE',
		'HoldInSameWrite = FALSE',
		/Invariant NoReturn is violated/u,
		'config',
	],
	[
		'release-store-fallback',
		'a held version is offered and activated again',
		'SkipHeld = TRUE',
		'SkipHeld = FALSE',
		/Invariant NoReturn is violated/u,
		'config',
	],
	[
		'release-store-fallback',
		'a move from a version that does not record may fall back',
		'ProofRequired = TRUE',
		'ProofRequired = FALSE',
		/Invariant TargetProven is violated/u,
		'config',
	],
	[
		'release-store-fallback',
		'a move may fall back below a floor of the failed version',
		'FloorChecked = TRUE',
		'FloorChecked = FALSE',
		/Invariant FallbackAboveFloor is violated/u,
		'config',
	],
	[
		'session-crash-recovery',
		'a replacement Worker starts before the Worker it replaces is proven gone',
		'workers[generation][incarnation] \\in {"none", "dead"}',
		'TRUE',
		/Invariant (?:SingleOwner|OnlyCurrentWorkerRuns|IdentityBeforeExecution) is violated/u,
	],
	[
		'session-crash-recovery',
		'a successor commits without the successor entry naming it',
		'  /\\ successor = "reserved"\n',
		'',
		/Invariant (?:SingleOwner|ServingIsRecorded) is violated/u,
		'module',
		'handover',
	],
	[
		'session-crash-recovery',
		'a released Server takes the Session back after the record named its successor',
		'  /\\ oldGeneration = generation\n',
		'',
		/Invariant (?:SingleOwner|ServingIsRecorded) is violated/u,
		'module',
		'handover',
	],
	[
		'session-crash-recovery',
		'cold adoption starts beside a Server or Worker not proven gone',
		'  /\\ ServerWorkerGone\n',
		'',
		/Invariant (?:SingleOwner|OnlyCurrentWorkerRuns|IdentityBeforeExecution|ServingIsRecorded) is violated/u,
		'module',
		'handover',
	],
	[
		'session-restart-handover',
		'the old Server takes the Session back without the fence',
		'  /\\ IF record.server = "So"\n       THEN /\\ record\' = NoEntry',
		"  /\\ IF TRUE\n       THEN /\\ record' = NoEntry",
		/Invariant (?:AdmitsOnlyRecordedOwner|SingleOwner) is violated/u,
	],
	[
		'session-restart-handover',
		'the old Server takes the Host back before the new Worker is proven gone',
		'/\\ Ends({"Wn", "Sn"}) /\\ so\' = "retaking"',
		'/\\ Ends({"Sn"}) /\\ so\' = "retaking"',
		/Invariant SuccessorTracked is violated/u,
	],
	[
		'session-restart-handover',
		'the new Server commits before the Host is adopted',
		'  /\\ sn = "adopted" /\\ proc["Sn"] = "run"\n',
		'  /\\ sn \\in {"stored", "adopted"} /\\ proc["Sn"] = "run"\n',
		/Invariant (?:AdmitsOnlyRecordedOwner|EndsWell) is violated/u,
	],
	[
		'session-restart-handover',
		'the old Server exits on the commit message without reading the record',
		'  /\\ IF record.server = "Sn"\n',
		'  /\\ IF TRUE\n',
		/Invariant ExitOnlyAfterCommit is violated/u,
	],
	[
		'session-restart-handover',
		'the new Server serves before its commit publish',
		'  /\\ sn = "released" /\\ proc["Sn"] = "run" /\\ proc["So"] \\notin Alive\n',
		'  /\\ sn \\in {"adopted", "released"} /\\ proc["Sn"] = "run" /\\ proc["So"] \\notin Alive\n',
		/Invariant AdmitsOnlyRecordedOwner is violated/u,
	],
	[
		'session-restart-handover',
		'the next start adopts beside a live new Worker the successor entry names',
		'LET live == {p \\in Named: proc[p] \\in Alive}',
		'LET live == {p \\in Named \\ {"Wn"}: proc[p] \\in Alive}',
		/Invariant (?:SuccessorTracked|AdoptionAlone|EndsWell) is violated/u,
	],
	[
		'session-restart-handover',
		'a live process the successor entry names refuses the next start',
		'  /\\ proc[record.server] \\notin Alive /\\ proc[RecordedWorker] \\notin Alive\n',
		'  /\\ proc[record.server] \\notin Alive /\\ proc[RecordedWorker] \\notin Alive\n  /\\ \\A p \\in Named: proc[p] \\notin Alive\n',
		/Invariant EndsWell is violated/u,
	],
	[
		'session-restart-handover',
		'the old Server clears an abandoned successor entry and leaves its Worker running',
		'  /\\ Ends(Named)\n',
		'  /\\ Ends(Named \\ {"Wn"})\n',
		/Invariant SuccessorTracked is violated/u,
	],
	[
		'session-restart-handover',
		'the requester clears its ended successor entry and leaves the Worker running',
		"  /\\ record' = NoEntry /\\ Ends(Named)\n",
		'  /\\ record\' = NoEntry /\\ Ends(Named \\ {"Wn"})\n',
		/Invariant SuccessorTracked is violated/u,
	],
	[
		'local-session-startup',
		'a replacement Worker starts a Host for the Launch authorized to the Worker it replaced',
		'\n  /\\ dispatchWorker = incarnation\n',
		'\n',
		/Invariant HostAnswersItsOwnLaunch is violated/u,
	],
	[
		'local-terminal-coordinator',
		'a replacement imports a binding whose operation is still in flight',
		'/\\ phase \\in {"awaiting", "started"}',
		'/\\ TRUE',
		/Action property ExactCommitWasIssuedToCurrentWorker is violated/u,
	],
	[
		'local-terminal-coordinator',
		'an adopting Server imports a recorded binding the Host did not confirm',
		'  /\\ workerState[recorded] = "quarantined"\n',
		'',
		/Invariant (?:SettledTerminalIsLive|RetiredIdNeverReissued) is violated/u,
		'module',
		'cold-adoption',
	],
	[
		'windows-checkpoint-storage',
		'a writer that no longer holds the store lock publishes the binding file',
		'  /\\ storeHeld /\\ t = storeTake\n',
		'',
		/Invariant BindingsCurrent is violated/u,
	],
];

describe('protocol mutation counterexamples', { concurrency: 3 }, () => {
	for (const [id, fault, before, after, counterexample, target = 'module', variantName] of cases) {
		test(`${id}: ${fault}`, async (t) => {
			const model = inventory.models.find((entry) => entry.id === id);
			// A named variant runs the mutation on that gate configuration, which supplies its control.
			const variant = model.variants?.find((entry) => entry.name === variantName);
			assert.equal(variant === undefined, variantName === undefined, `${id}: unknown variant`);
			const source = (await readFile(path.join(repositoryRoot, model[target]), 'utf8')).replaceAll(
				'\r\n',
				'\n',
			);
			assert.equal(source.split(before).length, 2, `${id}: mutation must name one guard`);
			const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-mutation-'));
			t.after(() => rm(directory, { recursive: true, force: true }));
			const module = path.basename(model.module);
			const config = path.basename(model.config);
			await writeFile(
				path.join(directory, module),
				target === 'module'
					? source.replace(before, after)
					: await readFile(path.join(repositoryRoot, model.module)),
			);
			const configuration =
				target === 'config'
					? source.replace(before, after)
					: await readFile(path.join(repositoryRoot, model.config), 'utf8');
			await writeFile(
				path.join(directory, config),
				variant === undefined
					? configuration
					: applyConfigConstants(configuration, variant.constants),
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
			if (target === 'config') {
				t.diagnostic(
					(result.stdout.match(/Error: Invariant[^\n]+|\d+ states generated[^\n]+/gu) ?? []).join(
						'; ',
					),
				);
			}
		});
	}
});
