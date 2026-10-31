import { execFileSync, spawn } from 'node:child_process';
import path from 'node:path';

// Deadline basis and measurements: docs/VERIFICATION.md#required-tasks.
const unitTestTimeout = '120s';
const runDeadlineMilliseconds = 20 * 60_000;
const timeoutExitCode = 124;
const listedProcessLimit = 40;

// Absolute system tools: a PATH entry cannot substitute the process lister or terminator.
function windowsTool(...segments) {
	const systemRoot = process.env.SystemRoot;
	if (typeof systemRoot !== 'string' || !path.isAbsolute(systemRoot))
		throw new Error('Missing Windows system root');
	return path.join(systemRoot, 'System32', ...segments);
}

/** Lists live processes as { pid, parent, created, command }; created orders PID reuse on Windows. */
function processTable() {
	if (process.platform === 'win32') {
		const output = execFileSync(
			windowsTool('WindowsPowerShell', 'v1.0', 'powershell.exe'),
			[
				'-NoLogo',
				'-NoProfile',
				'-NonInteractive',
				'-Command',
				'@(Get-CimInstance Win32_Process | ForEach-Object { [pscustomobject]@{ pid = $_.ProcessId; parent = $_.ParentProcessId; created = $_.CreationDate.ToFileTimeUtc(); command = if ($_.CommandLine) { $_.CommandLine } else { $_.Name } } }) | ConvertTo-Json -Compress',
			],
			{ encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 30_000, windowsHide: true },
		);
		return JSON.parse(output);
	}
	return execFileSync('/bin/ps', ['-A', '-o', 'pid=', '-o', 'ppid=', '-o', 'args='], {
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024,
		timeout: 30_000,
	})
		.split('\n')
		.map((line) => line.match(/^\s*(\d+)\s+(\d+)\s(.*)$/u))
		.filter((match) => match !== null)
		.map(([, pid, parent, command]) => ({
			pid: Number(pid),
			parent: Number(parent),
			created: 0,
			command,
		}));
}

/**
 * Adds every process descended from a known member, including one whose parent already exited:
 * its recorded parent PID still names a member, and a later creation time rules out PID reuse.
 */
function collectDescendants(members) {
	const table = processTable();
	const added = [];
	let grew = true;
	while (grew) {
		grew = false;
		for (const entry of table) {
			const parent = members.get(entry.parent);
			if (members.has(entry.pid) || parent === undefined || entry.created < parent.created)
				continue;
			members.set(entry.pid, entry);
			added.push(entry);
			grew = true;
		}
	}
	return added;
}

function terminate(pids) {
	if (pids.length === 0) return;
	if (process.platform === 'win32') {
		try {
			execFileSync(
				windowsTool('taskkill.exe'),
				['/F', ...pids.flatMap((pid) => ['/PID', String(pid)])],
				{
					stdio: 'ignore',
					timeout: 30_000,
					windowsHide: true,
				},
			);
		} catch {
			// A member may exit between listing and termination; the next listing rechecks.
		}
		return;
	}
	for (const pid of pids) {
		try {
			process.kill(pid, 'SIGKILL');
		} catch (error) {
			if (error.code !== 'ESRCH') throw error;
		}
	}
}

/** Terminates only the given child and the processes it started, and describes what was running. */
function terminateTree(child, spawnedAt) {
	// Windows reports FILETIME creation; one second of slack absorbs clock granularity.
	const created =
		process.platform === 'win32' ? (spawnedAt - 1000 + 11_644_473_600_000) * 10_000 : 0;
	const root = { pid: child.pid, parent: process.pid, created, command: 'zig build test' };
	const members = new Map([[root.pid, root]]);
	const running = [root, ...collectDescendants(members)];
	if (process.platform !== 'win32') {
		// Stopped processes cannot start more children while the tree is being collected.
		for (const entry of running) {
			try {
				process.kill(entry.pid, 'SIGSTOP');
			} catch {
				// An exited member needs no stop; collection and termination recheck it.
			}
		}
		collectDescendants(members);
		terminate([...members.keys()]);
	} else {
		// Kill parents first so they stop spawning, then sweep any children started meanwhile.
		let pending = running.map((entry) => entry.pid);
		for (let round = 0; pending.length > 0 && round < 5; round += 1) {
			terminate(pending);
			pending = collectDescendants(members).map((entry) => entry.pid);
		}
	}
	return running;
}

function describe(running) {
	const children = Map.groupBy(running.slice(1), (entry) => entry.parent);
	const lines = [];
	const visit = (entry, depth) => {
		lines.push(`${'  '.repeat(depth + 1)}${entry.pid} ${entry.command}`);
		for (const next of children.get(entry.pid) ?? []) visit(next, depth + 1);
	};
	visit(running[0], 0);
	if (lines.length > listedProcessLimit)
		lines.splice(listedProcessLimit, Infinity, `  ... ${lines.length - listedProcessLimit} more`);
	// Zig starts every unit-test binary with --listen=-; other leaves are native witnesses.
	const testBinaries = running.filter((entry) => /\s--listen=-(?:\s|$)/u.test(entry.command));
	const suspects = testBinaries.length
		? testBinaries
		: running.filter((entry) => !children.has(entry.pid));
	return [
		`${testBinaries.length ? 'Zig test binaries' : 'Leaf processes'} still running:`,
		...suspects.map((entry) => `  ${entry.command}`),
		`Process tree started by this run:`,
		...lines,
	].join('\n');
}

const spawnedAt = Date.now();
const child = spawn(
	'zig',
	['build', 'test', '-j1', '--test-timeout', unitTestTimeout, ...process.argv.slice(2)],
	{ stdio: 'inherit' },
);
let timedOut = false;
const deadline = setTimeout(() => {
	timedOut = true;
	let detail;
	try {
		detail = describe(terminateTree(child, spawnedAt));
	} catch (error) {
		child.kill('SIGKILL');
		detail = `Process tree listing failed (${error.message}); terminated zig only.`;
	}
	process.stderr.write(
		`error: Zig tests exceeded the ${runDeadlineMilliseconds / 60_000}-minute run deadline and were terminated.\n${detail}\n`,
	);
}, runDeadlineMilliseconds);
child.once('error', (error) => {
	clearTimeout(deadline);
	process.stderr.write(`error: failed to start zig: ${error.message}\n`);
	process.exitCode = 1;
});
child.once('exit', (code, signal) => {
	clearTimeout(deadline);
	if (timedOut) process.exitCode = timeoutExitCode;
	else if (signal !== null) process.kill(process.pid, signal);
	else process.exitCode = code ?? 1;
});
