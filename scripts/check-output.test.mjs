import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { checkVerbose } from './check-output.mjs';
import { checkSteps } from './run-checks.mjs';

const runnerUrl = new URL('./check-output.mjs', import.meta.url).href;

function nodeStep(name, source) {
	return { name, command: process.execPath, args: ['--input-type=module', '-e', source] };
}

function withRun(steps, inspect, options = {}) {
	const logRoot = mkdtempSync(join(tmpdir(), 'orchbee-check-output-test-'));
	try {
		const settings = { name: 'fixture', steps, logRoot, ...options };
		const source = `import { runChecks } from ${JSON.stringify(runnerUrl)};
			process.exitCode = runChecks(${JSON.stringify(settings)});`;
		const env = { ...process.env };
		delete env.ORCHBEE_CHECK_CAPTURED;
		const result = spawnSync(process.execPath, ['--input-type=module', '-e', source], {
			env,
			encoding: 'utf8',
			timeout: 20_000,
		});
		assert.ifError(result.error);
		const directories = readdirSync(logRoot);
		const directory = directories[0] === undefined ? undefined : join(logRoot, directories[0]);
		const logs = directory === undefined ? [] : readdirSync(directory).toSorted();
		inspect({ ...result, directory, logs, logRoot });
	} finally {
		rmSync(logRoot, { recursive: true, force: true });
	}
}

test('successful checks retain both streams privately and summarize recognized totals', () => {
	withRun(
		[
			nodeStep(
				'one',
				"process.stdout.write('test result: ok. 4 passed; 0 failed; 1 ignored;\\n'); process.stderr.write('diagnostic\\n');",
			),
			nodeStep('two', "process.stdout.write('# pass 3\\n# fail 0\\n# skipped 0\\n');"),
		],
		({ status, stdout, directory, logs }) => {
			assert.equal(status, 0);
			assert.equal(logs.length, 2);
			assert.match(stdout, /PASS one.*4 passed, 0 failed, 1 skipped/u);
			assert.match(stdout, /PASS two.*3 passed, 0 failed, 0 skipped/u);
			assert.doesNotMatch(stdout, /diagnostic/u);
			assert.equal(
				readFileSync(join(directory, logs[0]), 'utf8'),
				'test result: ok. 4 passed; 0 failed; 1 ignored;\ndiagnostic\n',
			);
			if (process.platform !== 'win32') {
				assert.equal(statSync(directory).mode & 0o777, 0o700);
				assert.equal(statSync(join(directory, logs[0])).mode & 0o777, 0o600);
			}
		},
	);
});

test('failure preserves the exit code, shows diagnostics, and never starts the next check', () => {
	withRun(
		[
			nodeStep(
				'broken',
				"process.stderr.write('error: deliberate failure\\n'); process.exitCode = 23;",
			),
			nodeStep('unreachable', "process.stdout.write('must not run');"),
		],
		({ status, stdout, stderr, logs }) => {
			assert.equal(status, 23);
			assert.equal(logs.length, 1);
			assert.match(stdout, /FAIL broken.*exit 23/u);
			assert.match(stderr, /error: deliberate failure/u);
			assert.doesNotMatch(stdout, /unreachable/u);
		},
	);
});

test('verbose mode streams the same checks without capture and remains fail-fast', () => {
	withRun(
		[
			nodeStep(
				'broken',
				"process.stdout.write('raw stdout\\n'); process.stderr.write('raw stderr\\n'); process.exitCode = 17;",
			),
			nodeStep('unreachable', "process.stdout.write('must not run');"),
		],
		({ status, stdout, stderr, logs }) => {
			assert.equal(status, 17);
			assert.equal(stdout, 'raw stdout\n');
			assert.equal(stderr, 'raw stderr\n');
			assert.deepEqual(logs, []);
		},
		{ verbose: true },
	);
});

test('nested checks reuse the outer log while preserving nested failure', () => {
	const nested = `import { runChecks } from ${JSON.stringify(runnerUrl)};
		process.exitCode = runChecks({ name: 'inner', steps: [${JSON.stringify(nodeStep('leaf', "process.stdout.write('nested output\\n'); process.exitCode = 9;"))}] });`;
	withRun([nodeStep('outer', nested)], ({ status, stdout, directory, logs }) => {
		assert.equal(status, 9);
		assert.equal(logs.length, 1);
		assert.equal((stdout.match(/Logs:/gu) ?? []).length, 1);
		assert.equal(readFileSync(join(directory, logs[0]), 'utf8'), 'nested output\n');
	});
});

test('large output is lossless on disk and bounded in failure diagnostics, including long lines', () => {
	const size = 2_000_000;
	withRun(
		[
			nodeStep(
				'large',
				`import { writeSync } from 'node:fs'; writeSync(1, 'first diagnostic\\n' + 'x'.repeat(${size}) + '\\nlast diagnostic\\n'); process.exitCode = 4;`,
			),
		],
		({ status, stdout, stderr, directory, logs }) => {
			assert.equal(status, 4);
			assert.ok(stdout.length + stderr.length < 16_000);
			assert.match(stderr, /first diagnostic/u);
			assert.match(stderr, /last diagnostic/u);
			assert.match(stderr, /omitted/u);
			assert.equal(
				readFileSync(join(directory, logs[0]), 'utf8'),
				`first diagnostic\n${'x'.repeat(size)}\nlast diagnostic\n`,
			);
		},
	);
});

test('success still surfaces warnings and unsupported-platform skips with a notice budget', () => {
	withRun(
		[
			nodeStep(
				'notices',
				"process.stdout.write('Tests  5 passed | 2 skipped (7)\\nSkipping unsupported runner\\nwarning: first warning\\n' + 'warning: repeated\\n'.repeat(40));",
			),
		],
		({ status, stdout }) => {
			assert.equal(status, 0);
			assert.match(stdout, /5 passed, 0 failed, 2 skipped/u);
			assert.match(stdout, /Skipping unsupported runner/u);
			assert.match(stdout, /warning: first warning/u);
			assert.match(stdout, /more notices/u);
			assert.ok(stdout.split('\n').length < 20);
		},
	);
});

test('a missing executable is a failure, not a successful empty check', () => {
	withRun(
		[{ name: 'missing', command: 'orchbee-nonexistent-check-fixture', args: [] }],
		({ status, stderr }) => {
			assert.equal(status, 1);
			assert.match(stderr, /ENOENT/u);
		},
	);
});

test('mixed framework totals include Zig failures and keep unexecuted tests visible', () => {
	const output = [
		'test result: ok. 2 passed; 0 failed; 1 ignored;',
		'# pass 3',
		'# fail 0',
		'# skipped 1',
		'# cancelled 1',
		'# todo 2',
		'\u001b[32mpackage test: Tests 5 passed | 2 skipped (7)\u001b[0m',
		'package test: Build Summary: 3/4 steps succeeded (1 failed); 7/12 tests passed (3 skipped, 2 failed)',
		'[default] (warn): diagnostic',
		'(node:123) ExperimentalWarning: diagnostic',
	].join('\n');
	withRun(
		[
			nodeStep(
				'frameworks',
				`process.stdout.write(${JSON.stringify(output)}); process.exitCode = 1;`,
			),
		],
		({ status, stdout }) => {
			assert.equal(status, 1);
			assert.match(stdout, /reported tests: 17 passed, 2 failed, 7 skipped, 1 cancelled, 2 todo/u);
			assert.match(stdout, /\(warn\): diagnostic/u);
			assert.match(stdout, /ExperimentalWarning: diagnostic/u);
		},
	);
});

test('failure headlines in the middle of noisy output survive excerpt truncation', () => {
	withRun(
		[
			nodeStep(
				'middle',
				"import { writeSync } from 'node:fs'; writeSync(1, 'noise\\n'.repeat(3000) + 'error[E0001]: middle diagnostic\\n' + 'noise\\n'.repeat(3000)); process.exitCode = 7;",
			),
		],
		({ status, stderr }) => {
			assert.equal(status, 7);
			assert.match(stderr, /error\[E0001\]: middle diagnostic/u);
			assert.match(stderr, /omitted/u);
			assert.ok(stderr.split('\n').length < 60);
		},
	);
});

test('log text cannot turn a failed process into a pass or invent a failing exit', () => {
	for (const [code, output] of [
		[5, '# pass 100\n# fail 0\n'],
		[0, 'error: this is fixture data\n'],
	]) {
		withRun(
			[
				nodeStep(
					'status',
					`process.stdout.write(${JSON.stringify(output)}); process.exitCode = ${code};`,
				),
			],
			({ status }) => {
				assert.equal(status, code);
			},
		);
	}
});

test('a log creation failure prevents check execution', () => {
	withRun(
		[nodeStep('unreachable', "process.stdout.write('must not run');")],
		({ status, stdout, stderr }) => {
			assert.equal(status, 1);
			assert.doesNotMatch(stdout, /must not run/u);
			assert.match(stderr, /ENOENT/u);
		},
		{ logRoot: join(tmpdir(), 'orchbee-nonexistent-log-parent-fixture', 'absent') },
	);
});

test(
	'a signalled child is nonzero and does not advance',
	{ skip: process.platform === 'win32' },
	() => {
		withRun(
			[
				nodeStep('interrupted', "process.kill(process.pid, 'SIGTERM');"),
				nodeStep('unreachable', ''),
			],
			({ status, stdout, logs }) => {
				assert.equal(status, 143);
				assert.equal(logs.length, 1);
				assert.match(stdout, /FAIL interrupted.*SIGTERM/u);
				assert.doesNotMatch(stdout, /unreachable/u);
			},
		);
	},
);

test('the root gate preserves its ordered stages and every automatic test scope', () => {
	assert.deepEqual(
		checkSteps('verify').map((step) => step.name),
		['format:check', 'lint', 'test:tla:inventory', 'build', 'typecheck', 'test', 'test:exports'],
	);
	const steps = checkSteps('test');
	assert.deepEqual(
		steps.slice(0, 8).map((step) => step.name),
		[
			'test:check-output',
			'test:lint-rules',
			'test:documentation-pairing',
			'test:ghostty-source',
			'test:style-contract',
			'test:storage-contract',
			'test:experience-contract',
			'test:ssh-environment-release',
		],
	);
	assert.equal(steps.length, 12);
	assert.deepEqual(steps[8].args.slice(-2), ['-r', 'test']);
	assert.deepEqual(steps[9].args, ['scripts/run-rust-workspace.mjs', 'test']);
	assert.deepEqual(steps[10].args.slice(-3), [
		'--filter',
		'@orchbee/desktop',
		'test:system:ssh-project-enrollment',
	]);
	assert.deepEqual(steps[11].args.slice(-3), ['-r', '--if-present', 'test:cross']);
	assert.doesNotMatch(JSON.stringify(steps), /manual:native-window/u);
});

test('unknown tasks and options are rejected instead of silently changing scope', () => {
	assert.throws(() => checkSteps('not-a-task'), /Unknown check task/u);
	assert.throws(() => checkVerbose(['--quiet']), /Unknown check options/u);
	assert.equal(checkVerbose([]), false);
	assert.equal(checkVerbose(['--', '--verbose']), true);
});
