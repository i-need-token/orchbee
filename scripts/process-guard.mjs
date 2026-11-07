#!/usr/bin/env node
// Guards agent process termination during development.
//
// `hook` is a Claude Code PreToolUse hook: it refuses raw termination commands in Bash and PowerShell
// tool calls. `list`, `check`, and `stop` are the permitted path. A process is acted on only when its
// PID, UTC start time, and executable path under a repository worktree all match, and never when the
// executable is under the production install directory.
//
// This guards against mistakes; it is not a security boundary. It does not inspect scripts that end
// processes internally, and it does not detect deliberately obfuscated commands. A hook event it cannot
// parse is allowed, so a malformed event never blocks ordinary work.
//
// `list`, `check`, and `stop` need Windows with powershell.exe and git on PATH. `hook` needs Node.js only.

import { execFileSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const START_TIME_PATTERN = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u;
const USAGE = `Usage:
  node scripts/process-guard.mjs hook
  node scripts/process-guard.mjs list
  node scripts/process-guard.mjs check --pid <PID> --started <UTC-START> [--tree]
  node scripts/process-guard.mjs stop --pid <PID> --started <UTC-START> [--tree]`;

// Termination commands refused in a tool call. Kept narrow so ordinary commands pass.
const RAW_TERMINATION_PATTERNS = [
	{ label: 'taskkill', pattern: /\btaskkill(?:\.exe)?\b/iu },
	{ label: 'Stop-Process', pattern: /\b(?:Stop-Process|spps)\b/iu },
	{
		label: 'WMI or CIM terminate',
		pattern: /\b(?:Invoke-CimMethod|Invoke-WmiMethod|wmic)\b[\s\S]*\b(?:Terminate|delete)\b/iu,
	},
	{ label: 'Process.Kill', pattern: /\.Kill\s*\(/iu },
	{ label: 'encoded PowerShell', pattern: /\s-(?:EncodedCommand|enc)\b/iu },
	{ label: 'kill by PID', pattern: /(?:^|[\s;&|(])kill\s+(?:-\S+\s+)*(?:%\d+|\d+|\$)/u },
	{ label: 'pkill or killall', pattern: /(?:^|[\s;&|(])(?:pkill|killall)\b/iu },
];

// Every process, with start times in the same UTC millisecond form that `list` prints and `--started` takes.
const PROCESS_SNAPSHOT_SCRIPT = `
Get-CimInstance -ClassName Win32_Process | ForEach-Object {
	[pscustomobject]@{
		pid = [int]$_.ProcessId
		parent = [int]$_.ParentProcessId
		name = $_.Name
		path = $_.ExecutablePath
		started = if ($_.CreationDate) { $_.CreationDate.ToUniversalTime().ToString('yyyy-MM-dd\\THH:mm:ss.fff\\Z') } else { $null }
	}
} | ConvertTo-Json -Compress
`;

// The start time is checked again at the moment of the stop, so a reused PID is never terminated.
const STOP_PROCESS_SCRIPT = `
$ErrorActionPreference = 'Stop'
$target = Get-CimInstance -ClassName Win32_Process -Filter "ProcessId = $env:GUARD_PID"
if ($null -eq $target) { 'missing'; exit 0 }
$started = $target.CreationDate.ToUniversalTime().ToString('yyyy-MM-dd\\THH:mm:ss.fff\\Z')
if ($started -ne $env:GUARD_STARTED) { 'identity-changed'; exit 0 }
Stop-Process -Id ([int]$env:GUARD_PID) -Force
'stopped'
`;

class UsageError extends Error {}

export function findRawTermination(commandText) {
	return RAW_TERMINATION_PATTERNS.find(({ pattern }) => pattern.test(commandText))?.label;
}

export function parseWorktreeList(porcelain) {
	return porcelain
		.split(/\r?\n/u)
		.filter((line) => line.startsWith('worktree '))
		.map((line) => line.slice('worktree '.length));
}

export function isUnderRoot(candidatePath, root) {
	const child = normalizeWindowsPath(candidatePath);
	const parent = normalizeWindowsPath(root);
	return child === parent || child.startsWith(`${parent}\\`);
}

function normalizeWindowsPath(value) {
	return path.win32
		.normalize(value)
		.replace(/[\\/]+$/u, '')
		.toLowerCase();
}

// A process is a target only when it still has the expected start time, runs from a worktree, and is
// not in the production install directory. The production check comes first so its refusal says why.
export function decideTarget(record, { startedAt, roots, productionRoots }) {
	if (record === undefined) return refused('no such process');
	if (typeof record.started !== 'string' || record.started !== startedAt) {
		return refused('start time differs; the PID was reused or the identity is wrong');
	}
	if (typeof record.path !== 'string' || record.path === '') {
		return refused('executable path is unavailable');
	}
	if (productionRoots.some((root) => isUnderRoot(record.path, root))) {
		return refused('production install directory');
	}
	if (!roots.some((root) => isUnderRoot(record.path, root))) {
		return refused('executable is outside the repository worktrees');
	}
	return { allowed: true, reason: 'start time and worktree path match' };
}

function refused(reason) {
	return { allowed: false, reason };
}

// Descendants come before their parent, so stopping in this order stops children first.
export function processTree(records, rootPid) {
	const childrenByParent = new Map();
	for (const record of records) {
		const siblings = childrenByParent.get(record.parent) ?? [];
		siblings.push(record);
		childrenByParent.set(record.parent, siblings);
	}
	const ordered = [];
	const visited = new Set([rootPid]);
	const visit = (pid) => {
		for (const child of childrenByParent.get(pid) ?? []) {
			if (visited.has(child.pid)) continue;
			visited.add(child.pid);
			visit(child.pid);
			ordered.push(child);
		}
	};
	visit(rootPid);
	const root = records.find((record) => record.pid === rootPid);
	if (root !== undefined) ordered.push(root);
	return ordered;
}

// Progress is silenced so module-loading records do not leak into this process's stderr as CLIXML.
function runPowerShell(script, environment = {}) {
	const encoded = Buffer.from(
		`$ProgressPreference = 'SilentlyContinue'\n${script}`,
		'utf16le',
	).toString('base64');
	try {
		return execFileSync(
			'powershell.exe',
			['-NoProfile', '-NonInteractive', '-EncodedCommand', encoded],
			{
				encoding: 'utf8',
				env: { ...process.env, ...environment },
				maxBuffer: 64 * 1024 * 1024,
				stdio: ['ignore', 'pipe', 'pipe'],
				windowsHide: true,
			},
		);
	} catch (error) {
		const detail = error instanceof Error && 'stderr' in error ? String(error.stderr).trim() : '';
		throw new Error(`PowerShell query failed${detail === '' ? '' : `: ${detail}`}`, {
			cause: error,
		});
	}
}

function readProcesses() {
	const output = runPowerShell(PROCESS_SNAPSHOT_SCRIPT).trim();
	if (output === '') return [];
	const parsed = JSON.parse(output);
	return Array.isArray(parsed) ? parsed : [parsed];
}

function readWorktreeRoots() {
	const porcelain = execFileSync(
		'git',
		['-C', REPOSITORY_ROOT, 'worktree', 'list', '--porcelain'],
		{
			encoding: 'utf8',
		},
	);
	return parseWorktreeList(porcelain);
}

function productionInstallRoots() {
	const roots = [];
	if (process.env.LOCALAPPDATA !== undefined) {
		roots.push(path.win32.join(process.env.LOCALAPPDATA, 'Programs', 'Orchbee'));
	}
	if (process.env.ORCHBEE_INSTALL_DIR !== undefined && process.env.ORCHBEE_INSTALL_DIR !== '') {
		roots.push(process.env.ORCHBEE_INSTALL_DIR);
	}
	return roots;
}

// Every target is decided before anything stops. One refused target refuses the whole operation.
function planTargets(options) {
	const context = { roots: readWorktreeRoots(), productionRoots: productionInstallRoots() };
	const snapshot = readProcesses();
	const root = snapshot.find((record) => record.pid === options.pid);
	const rootDecision = decideTarget(root, { ...context, startedAt: options.started });
	if (!options.tree || root === undefined) {
		return [{ pid: options.pid, record: root, decision: rootDecision }];
	}
	return processTree(snapshot, options.pid).map((record) => ({
		pid: record.pid,
		record,
		decision:
			record.pid === options.pid
				? rootDecision
				: decideTarget(record, { ...context, startedAt: record.started }),
	}));
}

function stopRecord(record) {
	const output = runPowerShell(STOP_PROCESS_SCRIPT, {
		GUARD_PID: String(record.pid),
		GUARD_STARTED: record.started,
	});
	const outcome = output.trim().split(/\r?\n/u).at(-1);
	if (outcome === 'stopped' || outcome === 'missing' || outcome === 'identity-changed') {
		return outcome;
	}
	throw new Error(`unexpected stop result for PID ${record.pid}: ${output.trim()}`);
}

function printPlan(plan) {
	for (const { pid, record, decision } of plan) {
		const detail = record === undefined ? '' : ` started=${record.started} path=${record.path}`;
		const state = decision.allowed ? 'allowed' : 'refused';
		process.stdout.write(`${state} pid=${pid}${detail} reason=${decision.reason}\n`);
	}
}

function runList() {
	const context = { roots: readWorktreeRoots(), productionRoots: productionInstallRoots() };
	for (const record of readProcesses()) {
		if (decideTarget(record, { ...context, startedAt: record.started }).allowed) {
			process.stdout.write(
				`pid=${record.pid} started=${record.started} name=${record.name} path=${record.path}\n`,
			);
		}
	}
	return 0;
}

function runCheck(options) {
	const plan = planTargets(options);
	printPlan(plan);
	return plan.every(({ decision }) => decision.allowed) ? 0 : 1;
}

function runStop(options) {
	const plan = planTargets(options);
	printPlan(plan);
	if (!plan.every(({ decision }) => decision.allowed)) {
		process.stdout.write('stop refused; nothing was stopped\n');
		return 1;
	}
	let exitCode = 0;
	for (const { pid, record } of plan) {
		const outcome = stopRecord(record);
		process.stdout.write(`${outcome} pid=${pid}\n`);
		if (outcome === 'identity-changed') exitCode = 1;
	}
	return exitCode;
}

function parseOptions(args) {
	const options = { pid: Number.NaN, started: '', tree: false };
	for (let index = 0; index < args.length; index += 1) {
		const argument = args[index];
		if (argument === '--tree') {
			options.tree = true;
		} else if (argument === '--pid') {
			options.pid = Number(args[index + 1]);
			index += 1;
		} else if (argument === '--started') {
			options.started = args[index + 1] ?? '';
			index += 1;
		} else {
			throw new UsageError(`unknown argument: ${argument}\n${USAGE}`);
		}
	}
	if (!Number.isInteger(options.pid) || options.pid <= 0) {
		throw new UsageError(`--pid must be a positive integer\n${USAGE}`);
	}
	if (!START_TIME_PATTERN.test(options.started)) {
		throw new UsageError(
			`--started must be the UTC start time that list prints, such as 2026-11-07T01:02:03.456Z\n${USAGE}`,
		);
	}
	return options;
}

function blockMessage(label) {
	return `Blocked by the process guard: raw process termination (${label}) is not allowed for agents.
Stop only a process this task started, by PID and UTC start time, from the repository root:
  node scripts/process-guard.mjs list
  node scripts/process-guard.mjs check --pid <PID> --started <UTC-START> [--tree]
  node scripts/process-guard.mjs stop --pid <PID> --started <UTC-START> [--tree]
Never stop the production Orchbee; ask the human to do it.
`;
}

async function readStandardInput() {
	const chunks = [];
	for await (const chunk of process.stdin) chunks.push(chunk);
	return Buffer.concat(chunks).toString('utf8');
}

function parseHookEvent(text) {
	try {
		const parsed = JSON.parse(text);
		return typeof parsed === 'object' && parsed !== null ? parsed : undefined;
	} catch {
		return undefined;
	}
}

async function runHook() {
	const event = parseHookEvent(await readStandardInput());
	if (event === undefined) return 0;
	if (event.tool_name !== 'Bash' && event.tool_name !== 'PowerShell') return 0;
	const input = event.tool_input ?? {};
	const commandText = typeof input.command === 'string' ? input.command : JSON.stringify(input);
	const label = findRawTermination(commandText);
	if (label === undefined) return 0;
	process.stderr.write(blockMessage(label));
	return 2;
}

async function main(argv) {
	const [command, ...rest] = argv;
	if (command === 'hook') return runHook();
	if (command !== 'list' && command !== 'check' && command !== 'stop') {
		throw new UsageError(USAGE);
	}
	if (process.platform !== 'win32') throw new UsageError(`${command} requires Windows`);
	if (command === 'list') {
		if (rest.length > 0) throw new UsageError(USAGE);
		return runList();
	}
	const options = parseOptions(rest);
	return command === 'check' ? runCheck(options) : runStop(options);
}

const invokedDirectly =
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]).toLowerCase() === fileURLToPath(import.meta.url).toLowerCase();
if (invokedDirectly) {
	try {
		process.exitCode = await main(process.argv.slice(2));
	} catch (error) {
		process.stderr.write(
			`process-guard: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	}
}
