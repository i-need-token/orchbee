import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repository = fileURLToPath(new URL('../../../', import.meta.url));
const modelDirectory = new URL('./tla/', import.meta.url);
const source = readFileSync(new URL('TerminalClientViewport.tla', modelDirectory), 'utf8');
const configuration = readFileSync(new URL('TerminalClientViewport.cfg', modelDirectory), 'utf8');
const inventory = JSON.parse(
	readFileSync(path.join(repository, 'scripts/tla/models.json'), 'utf8'),
);
const jar = process.env.ORCHBEE_TLA2TOOLS_JAR
	? path.resolve(process.env.ORCHBEE_TLA2TOOLS_JAR)
	: path.join(repository, '.tools', 'tla2tools-' + inventory.tool.version + '.jar');
assert.equal(createHash('sha256').update(readFileSync(jar)).digest('hex'), inventory.tool.sha256);
const variables = source
	.match(/vars == <<([\s\S]*?)>>/u)[1]
	.split(',')
	.map((name) => name.trim());
const fairness = source.match(
	/Spec == Init \/\\ \[\]\[Next\]_vars([\s\S]*?)\\\* ---------------- invariants/u,
)[1];
const fairActions = [...fairness.matchAll(/WF_vars\((\w+)\)/gu)].map((match) => match[1]);
const quiescent = '  /\\ ~ENABLED <<(' + fairActions.join(' \\/ ') + ')>>_vars';

// Reach Entering with four old Inputs ahead of Enter and a staged Recover
// behind the full queue. Every prefix edge is an actual production-model action.
const prefix = [
	'PressEnter',
	'KeyTyped',
	'KeyTyped',
	'KeyTyped',
	'(ExecuteEnter /\\ demand\' = "viewing")',
	'(ExecuteRecover /\\ mode\' = "recovering")',
	'ConsumeEnterApplied',
	'ConsumeRecovered',
	'KeyTyped',
	'FrameArrive',
	'PressEnter',
	'DeadlineTick',
	'DeadlineExpire',
];
const drain = [
	'HostFaultWindow',
	'AdmitStaged',
	'ExecuteInput',
	'ExecuteEnter',
	'ExecuteRecover',
	'ConsumeEnterApplied',
	'ConsumeEnterRejected',
	'ConsumeRecovered',
	'DropDeadOutcome',
	'DropOutcome',
	'StrayWindow',
	'FrameArrive',
	'DeadlineTick',
	'DeadlineExpire',
];
const driver = [
	'---------------- MODULE ViewportStarvationRegression ----------------',
	'EXTENDS TerminalClientViewport',
	'VARIABLE pc',
	'TraceInit == Init /\\ pc = 0',
	'TracePrefix ==',
	...prefix.map(
		(action, index) => '  \\/ /\\ pc = ' + index + " /\\ pc' = " + (index + 1) + ' /\\ ' + action,
	),
	'TraceDrain ==',
	'  /\\ pc = ' + prefix.length + ' /\\ UNCHANGED pc',
	'  /\\ (' + drain.join(' \\/ ') + ')',
	'TraceResolved ==',
	'  /\\ pc = ' + prefix.length + ' /\\ mode \\in {"live", "lost"} /\\ outcomes = <<>>',
	quiescent,
	"  /\\ pc' = " + (prefix.length + 1) + ' /\\ UNCHANGED vars',
	'TraceNext == TracePrefix \\/ TraceDrain \\/ TraceResolved',
	'TraceSpec == TraceInit /\\ [][TraceNext]_<<pc, vars>>',
	fairness,
	'=============================================================================',
].join('\n');

// Reintroduce exactly the former independent producer, without changing
// fairness, queue capacity, the prefix, or any original property.
const legacyProducer = [
	'HostFaultWindow ==',
	'  /\\ WorkerFree',
	'  /\\ outcomes\' = Append(outcomes, [type |-> "read", r |-> "window",',
	'       sq |-> 0, g |-> host_gen, a |-> "none", e |-> "none", v |-> "none"])',
	'  /\\ UNCHANGED <<' + variables.filter((name) => name !== 'outcomes').join(', ') + '>>',
	'',
].join('\n');
const legacySource = source.replace(/HostFaultWindow ==[\s\S]*?(?=\nNext ==)/u, legacyProducer);
assert.notEqual(legacySource, source);

function check(t, model, checkOwnership, trace = driver, properties = []) {
	const root = mkdtempSync(path.join(tmpdir(), 'orchbee-viewport-regression-'));
	try {
		writeFileSync(path.join(root, 'TerminalClientViewport.tla'), model);
		writeFileSync(path.join(root, 'ViewportStarvationRegression.tla'), trace);
		let config = configuration.replace('SPECIFICATION Spec', 'SPECIFICATION TraceSpec');
		if (!checkOwnership) config = config.replace(/^\s*OutcomeProductionOwned\s*$/mu, '');
		if (properties.length > 0)
			config = config.replace('PROPERTIES', 'PROPERTIES\n    ' + properties.join('\n    '));
		writeFileSync(path.join(root, 'ViewportStarvationRegression.cfg'), config);
		const result = spawnSync(
			process.env.ORCHBEE_TLA_JAVA ?? 'java',
			[
				'-Xmx512m',
				'-Djava.io.tmpdir=' + root,
				'-jar',
				jar,
				'-workers',
				'1',
				'-fp',
				'0',
				'-lncheck',
				'final',
				'-coverage',
				'1',
				'-metadir',
				path.join(root, 'states'),
				'-config',
				'ViewportStarvationRegression.cfg',
				'ViewportStarvationRegression.tla',
			],
			{ cwd: root, encoding: 'utf8', timeout: 45_000, maxBuffer: 4 * 1024 * 1024 },
		);
		assert.ifError(result.error);
		const output = result.stdout + result.stderr;
		for (const line of output.split('\n'))
			if (/^\d+ states generated,|^Finished in/u.test(line)) t.diagnostic(line.trim());
		return { status: result.status, output };
	} finally {
		assert.equal(path.dirname(root), path.resolve(tmpdir()));
		rmSync(root, { recursive: true, force: true });
	}
}

test('a full queue after re-entry drains under the original weak fairness', (t) => {
	const { status, output } = check(t, source, true);
	assert.equal(status, 0, output);
	assert.match(output, /Model checking completed\. No error has been found\./u);
	assert.match(output, /\d+ states generated, \d+ distinct states found, 0 states left on queue/u);
	// Every fair action is disabled at this witness, so its infinite stuttering
	// suffix satisfies the original fairness. Coverage proves it is reachable.
	assert.match(output, /<TraceResolved [^>]+>:\s*\d+:[1-9]\d*/u);
});

test('an independent window producer reproduces the original fair starvation cycle', (t) => {
	const { status, output } = check(t, legacySource, false);
	assert.equal(status, 13, output);
	assert.match(output, /Temporal properties were violated/u);
	assert.match(output, /Back to state \d+/u);
});

test('outcome ownership rejects the independent producer before liveness checking', (t) => {
	const { status, output } = check(t, legacySource, true);
	assert.equal(status, 13, output);
	assert.match(output, /Action property OutcomeProductionOwned is violated/u);
});

for (const slot of [1, 2]) {
	test(
		'a faulty window in FIFO slot ' + slot + ' preserves its served read until recovery',
		(t) => {
			const actions = [
				'PressEnter',
				'(ExecuteEnter /\\ demand\' = "viewing")',
				...(slot === 1 ? ['ConsumeEnterApplied'] : []),
				'(HostEmitEvent /\\ evt_anchor\' = "committed")',
				'FactsArrive',
				'EmitTail',
				'AdmitStaged',
				'(ExecuteRead /\\ outcomes\'[Len(outcomes\')].r = "window")',
				'HostFaultWindow',
				...(slot === 2 ? ['ConsumeEnterApplied'] : []),
				'StrayWindow',
				'AdmitStaged',
				'(ExecuteRecover /\\ mode\' = "recovering")',
				'ConsumeRecovered',
				'FrameArrive',
			];
			const trace = [
				'---------------- MODULE ViewportStarvationRegression ----------------',
				'EXTENDS TerminalClientViewport',
				'VARIABLE pc',
				'TraceInit == Init /\\ pc = 0',
				'TracePrefix ==',
				...actions.map(
					(action, index) =>
						'  \\/ /\\ pc = ' + index + " /\\ pc' = " + (index + 1) + ' /\\ ' + action,
				),
				'TraceResolved ==',
				'  /\\ pc = ' + actions.length + ' /\\ mode = "live"',
				quiescent,
				"  /\\ pc' = " + (actions.length + 1) + ' /\\ UNCHANGED vars',
				'TraceNext == TracePrefix \\/ TraceResolved',
				'TraceSpec == TraceInit /\\ [][TraceNext]_<<pc, vars>>',
				fairness,
				'FaultKeepsServedRead == [][HostFaultWindow =>',
				'  /\\ Len(outcomes) = ' + slot + " /\\ Len(outcomes') = Len(outcomes)",
				'  /\\ host_pending = 1 /\\ UNCHANGED <<host_pending, cmd_q>>',
				"  /\\ outcomes'[" + slot + '].sq = 0]_vars',
				'StrayQueuesRecovery == [][StrayWindow =>',
				"  /\\ staged' = <<RecoverStep>> /\\ win_seq' = 0",
				"  /\\ read_flight' = 1 /\\ host_pending' = 1]_vars",
				'=============================================================================',
			].join('\n');
			const { status, output } = check(t, source, true, trace, [
				'FaultKeepsServedRead',
				'StrayQueuesRecovery',
			]);
			assert.equal(status, 0, output);
			assert.match(output, /Model checking completed\. No error has been found\./u);
			assert.match(output, /<TraceResolved [^>]+>:\s*\d+:[1-9]\d*/u);
		},
	);
}
