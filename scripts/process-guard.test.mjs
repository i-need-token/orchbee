import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
	decideTarget,
	findRawTermination,
	isUnderRoot,
	parseWorktreeList,
	processTree,
} from './process-guard.mjs';

const worktree = 'C:/Users/luna/code/orchbee-worktrees/orc-382-env-isolation';
const production = 'C:\\Users\\luna\\AppData\\Local\\Programs\\Orchbee';
const startedAt = '2026-11-07T01:02:03.456Z';

test('raw termination commands are refused', () => {
	for (const command of [
		'taskkill /F /PID 1234',
		'taskkill.exe /IM orchbee.exe',
		'Stop-Process -Name orchbee -Force',
		'Get-Process orchbee | Stop-Process',
		'spps -Id 42',
		'Invoke-CimMethod -ClassName Win32_Process -MethodName Terminate',
		'wmic process where name="orchbee.exe" call terminate',
		'[System.Diagnostics.Process]::GetProcessById(42).Kill()',
		'node -e "process.kill(42)"',
		'powershell -NoProfile -EncodedCommand ZQBjAGgAbwA=',
		'pwsh -enc ZQBjAGgAbwA=',
		'kill -9 1234',
		'kill 42',
		'pkill node',
		'killall node',
	]) {
		assert.notEqual(findRawTermination(command), undefined, command);
	}
});

test('ordinary commands, the guard, and its own check command are not refused', () => {
	for (const command of [
		'git status --short',
		'git commit -m "fix kill logic"',
		'node scripts/process-guard.mjs check --pid 1234 --started 2026-11-07T01:02:03.456Z',
		'node --test scripts/process-guard.test.mjs',
	]) {
		assert.equal(findRawTermination(command), undefined, command);
	}
});

test('a path is under a root only at a directory boundary', () => {
	assert.equal(isUnderRoot(`${worktree}/target/debug/orchbee.exe`, worktree), true);
	assert.equal(
		isUnderRoot('C:\\USERS\\luna\\code\\orchbee-worktrees\\ORC-382-env-isolation\\x.exe', worktree),
		true,
	);
	assert.equal(isUnderRoot(`${worktree}-copy/orchbee.exe`, worktree), false);
	assert.equal(isUnderRoot(`${production}\\orchbee.exe`, worktree), false);
});

test('a target needs the expected start time, a worktree path, and no production path', () => {
	const roots = [worktree];
	const options = { roots, productionRoots: [production], startedAt };
	const devProcess = {
		pid: 10,
		started: startedAt,
		name: 'orchbee.exe',
		path: `${worktree}/target/debug/orchbee.exe`,
	};

	assert.equal(decideTarget(devProcess, options).allowed, true);
	assert.equal(
		decideTarget(devProcess, { ...options, startedAt: '2026-11-07T01:02:03.457Z' }).allowed,
		false,
	);
	assert.equal(decideTarget(undefined, options).allowed, false);
	assert.equal(
		decideTarget({ ...devProcess, started: null }, { ...options, startedAt: null }).allowed,
		false,
	);
	assert.equal(decideTarget({ ...devProcess, path: null }, options).allowed, false);
	assert.equal(
		decideTarget({ ...devProcess, path: 'C:\\Windows\\System32\\PING.EXE' }, options).allowed,
		false,
	);
	assert.equal(
		decideTarget(
			{ ...devProcess, path: `${production}\\orchbee.exe` },
			{ ...options, roots: [worktree, production] },
		).reason,
		'production install directory',
	);
});

test('worktree listing keeps every listed path', () => {
	const porcelain = [
		'worktree C:/a/b',
		'HEAD 1111111',
		'branch refs/heads/main',
		'',
		'worktree C:/a/c',
		'HEAD 2222222',
		'',
	].join('\n');
	assert.deepEqual(parseWorktreeList(porcelain), ['C:/a/b', 'C:/a/c']);
});

test('a process tree lists descendants before their parent and survives a cycle', () => {
	const records = [
		{ pid: 1, parent: 0 },
		{ pid: 2, parent: 1 },
		{ pid: 3, parent: 2 },
		{ pid: 9, parent: 0 },
	];
	assert.deepEqual(
		processTree(records, 1).map((record) => record.pid),
		[3, 2, 1],
	);
	assert.deepEqual(
		processTree(
			[
				{ pid: 5, parent: 6 },
				{ pid: 6, parent: 5 },
			],
			5,
		).map((record) => record.pid),
		[6, 5],
	);
});
