// Drives installed Agent CLIs in a scratch Orchbee session and records the reported Panel state.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { platform } from 'node:os';
import path from 'node:path';
import { parseArgs } from 'node:util';

const OK_PROMPT = 'Reply with the single word OK. Do not use tools.';
const SHELL_TURN = '!ping -n 5 127.0.0.1';
const QUESTION_PROMPT =
	'Use your question tool to ask me whether I prefer red or blue. Do nothing else.';
// Launch commands for the supported CLIs this check exercises; a turn costs one trivial request.
// Exit keys go in one burst because some CLIs quit only on a quick double press.
const AGENTS = {
	claude: { launch: 'claude', turn: OK_PROMPT, exit: ['escape', 'ctrl+c', 'ctrl+c'] },
	codex: { launch: 'codex', turn: OK_PROMPT, exit: ['ctrl+c', 'ctrl+c'] },
	devin: { launch: 'devin', turn: OK_PROMPT, exit: ['ctrl+c', 'ctrl+c'] },
	grok: { launch: 'grok', turn: OK_PROMPT, exit: ['ctrl+q', 'ctrl+q'] },
	omp: { launch: 'omp --no-session', turn: SHELL_TURN, exit: ['ctrl+d', 'ctrl+d'] },
	opencode: { launch: 'opencode', turn: OK_PROMPT, question: QUESTION_PROMPT, exit: ['ctrl+c'] },
	pi: { launch: 'pi --no-session', turn: SHELL_TURN, exit: ['ctrl+d'] },
};
const POLL_MS = 500;

const { values } = parseArgs({
	options: {
		orchbee: { type: 'string' },
		scratch: { type: 'string' },
		agents: { type: 'string', default: Object.keys(AGENTS).join(',') },
		'path-prepend': { type: 'string', multiple: true, default: [] },
	},
});
if (platform() !== 'win32') throw new Error('The managed Server self-test runs on Windows only');
if (values.orchbee === undefined || values.scratch === undefined) {
	throw new Error('Usage: --orchbee <staged orchbee.exe> --scratch <new directory> [--agents a,b]');
}
const scratch = path.resolve(values.scratch);
if (existsSync(scratch))
	throw new Error(`Refusing to reuse an existing scratch directory: ${scratch}`);
const captures = path.join(scratch, 'captures');
mkdirSync(captures, { recursive: true });
const env = {
	...process.env,
	APPDATA: path.join(scratch, 'appdata'),
	LOCALAPPDATA: path.join(scratch, 'localappdata'),
	ORCHBEE_RUNTIME_BASE: path.join(scratch, 'rt'),
	ORCHBEE_SESSION: `agent-self-test-${process.pid}`,
};
for (const name of ['APPDATA', 'LOCALAPPDATA', 'ORCHBEE_RUNTIME_BASE']) mkdirSync(env[name]);
const orchbee = (args, timeout = 15_000) =>
	execFileSync(values.orchbee, args, { encoding: 'utf8', env, timeout, windowsHide: true });
// The steps are strictly sequential, so a blocking sleep keeps the driver synchronous.
const sleep = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);

function sample(panel) {
	let state = 'none';
	let title = '';
	try {
		const { panel: found } = JSON.parse(
			orchbee(['panel', 'get', panel, '--format', 'json']),
		).result;
		const view = found.reportedAgent?.view;
		state = view?.agent === null || view === undefined ? 'none' : view.state;
		title = found.terminalObservation?.frame?.title ?? '';
	} catch {
		state = 'read-failed';
	}
	let screen = '';
	try {
		screen = orchbee(['terminal', 'read', panel, '--source', 'detection', '--plain']);
	} catch {
		screen = '';
	}
	return { state, title, screen };
}
const send = (panel, args) => orchbee(['terminal', 'send', panel, ...args]);
// Enter follows separately: some composers treat text and Enter in one burst as a paste.
function submit(panel, text) {
	send(panel, ['--text', text]);
	sleep(1000);
	send(panel, ['--key', 'enter']);
}

// Polls until `done` accepts the history or the deadline passes; returns the state transitions.
function watch(panel, seconds, done) {
	const seen = [];
	let last;
	const end = Date.now() + seconds * 1000;
	while (Date.now() < end) {
		last = sample(panel);
		seen.push(last.state);
		if (done(seen)) break;
		sleep(POLL_MS);
	}
	return { last, seen: seen.filter((state, index) => state !== seen[index - 1]) };
}
const settled = (count) => (seen) =>
	seen.length >= count && seen.slice(-count).every((state) => state === seen.at(-1));

const results = [];
function record(agent, step, expected, observed, pass) {
	const file = path.join(captures, `${agent}-${step}.txt`);
	writeFileSync(
		file,
		`state: ${observed.last.state}\ntitle: ${observed.last.title}\n\n${observed.last.screen}`,
	);
	results.push({ agent, step, expected, reported: observed.seen.join('>'), pass, capture: file });
	process.stdout.write(
		`${agent.padEnd(9)} ${step.padEnd(9)} ${expected.padEnd(14)} ${observed.seen.join('>').padEnd(28)} ${pass ? 'PASS' : 'FAIL'}\n`,
	);
}

function drive(agent, spec) {
	const cwd = path.join(scratch, 'ws', agent);
	mkdirSync(cwd, { recursive: true });
	const created = JSON.parse(
		orchbee(
			[
				'workspace',
				'create',
				'--label',
				agent,
				'--cwd',
				cwd,
				'--exec',
				path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'cmd.exe'),
				'--format',
				'json',
			],
			30_000,
		),
	);
	const panel = `@panel:0x${BigInt(`0x${created.workspace.result.panelId}`).toString(16)}`;
	watch(panel, 15, () => /\S>\s*$/u.test(sample(panel).screen));
	if (values['path-prepend'].length > 0) {
		send(panel, ['--text', `set "PATH=${values['path-prepend'].join(';')};%PATH%"`, '--enter']);
	}
	send(panel, ['--text', `cls& ${spec.launch}`, '--enter']);
	const launch = watch(
		panel,
		60,
		(seen) => seen.length > 6 && !['none', 'unknown'].includes(seen.at(-1)) && settled(8)(seen),
	);
	const ready = launch.last.state === 'idle';
	record(agent, 'launch', 'idle|blocked', launch, ready || launch.last.state === 'blocked');
	if (ready) {
		submit(panel, spec.turn);
		const turn = watch(
			panel,
			120,
			(seen) => seen.includes('working') && settled(6)(seen) && seen.at(-1) !== 'working',
		);
		record(
			agent,
			'turn',
			'working>idle',
			turn,
			turn.seen.includes('working') && turn.last.state === 'idle',
		);
		if (spec.question !== undefined && turn.last.state === 'idle') {
			submit(panel, spec.question);
			const asked = watch(panel, 120, (seen) => seen.at(-1) === 'blocked' && settled(4)(seen));
			record(agent, 'question', 'blocked', asked, asked.last.state === 'blocked');
			send(panel, ['--key', 'escape']);
			const dismissed = watch(panel, 60, (seen) => seen.at(-1) === 'idle' && settled(6)(seen));
			record(agent, 'dismiss', 'idle', dismissed, dismissed.last.state === 'idle');
		}
	} else {
		// Never type into an unconfirmed screen: Enter could accept a trust or login prompt.
		results.push({
			agent,
			step: 'turn',
			expected: 'working>idle',
			reported: 'skipped',
			pass: null,
		});
		process.stdout.write(`${agent.padEnd(9)} turn      skipped: launch did not report idle\n`);
	}
	send(
		panel,
		spec.exit.flatMap((key) => ['--key', key]),
	);
	const exited = watch(panel, 45, (seen) => seen.at(-1) === 'none' && settled(3)(seen));
	record(agent, 'exit', 'none', exited, exited.last.state === 'none');
}

orchbee(['server', 'start'], 60_000);
try {
	for (const agent of values.agents.split(',')) {
		const spec = AGENTS[agent];
		if (spec === undefined) throw new Error(`No self-test entry for ${agent}`);
		drive(agent, spec);
	}
} finally {
	orchbee(['server', 'stop', '--confirm'], 60_000);
	writeFileSync(path.join(scratch, 'report.json'), `${JSON.stringify(results, null, 2)}\n`);
	process.stdout.write(`Report and captures: ${scratch}. Delete it when you are done.\n`);
}
