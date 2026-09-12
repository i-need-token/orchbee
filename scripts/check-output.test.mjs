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
			process.exitCode = await runChecks(${JSON.stringify(settings)});`;
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
		process.exitCode = await runChecks({ name: 'inner', steps: [${JSON.stringify(nodeStep('leaf', "process.stdout.write('nested output\\n'); process.exitCode = 9;"))}] });`;
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
		['format:check', 'test:tla:inventory', 'build', 'lint', 'typecheck', 'test', 'test:exports'],
	);
	const steps = checkSteps('test');
	assert.equal(steps.length, 4);
	assert.deepEqual(
		steps[0].concurrent.map((step) => step.name),
		[
			'test:check-output',
			'test:rust-runner',
			'test:lint-rules',
			'test:documentation-pairing',
			'test:ghostty-source',
			'test:style-contract',
			'test:storage-contract',
			'test:experience-contract',
			'test:ssh-environment-release',
		],
	);
	assert.equal(steps[1].name, 'workspace tests');
	assert.deepEqual(steps[1].args.slice(-2), ['-r', 'test']);
	assert.equal(steps[2].name, 'Rust workspace tests');
	assert.deepEqual(steps[2].args, ['scripts/run-rust-workspace.mjs', 'test']);
	assert.deepEqual(
		steps[3].concurrent.map((step) => step.name),
		['SSH Project enrollment system witness', 'native cross-compilation checks'],
	);
	assert.deepEqual(steps[3].concurrent[1].args.slice(-3), ['-r', '--if-present', 'test:cross']);
	assert.doesNotMatch(JSON.stringify(steps), /manual:native-window/u);
});

test('concurrent group runs its members in parallel, reports each, and blocks the next stage', () => {
	withRun(
		[
			{
				concurrent: [
					nodeStep(
						'slow-ok',
						"await new Promise((resolve) => setTimeout(resolve, 400)); process.stdout.write('# pass 2\\n# fail 0\\n# skipped 0\\n');",
					),
					nodeStep(
						'fast-broken',
						"process.stderr.write('error: deliberate concurrent failure\\n'); process.exitCode = 11;",
					),
				],
			},
			nodeStep('unreachable', "process.stdout.write('must not run');"),
		],
		({ status, stdout, stderr, logs }) => {
			assert.equal(status, 11);
			assert.equal(logs.length, 2);
			assert.match(stdout, /PASS slow-ok.*2 passed/u);
			assert.match(stdout, /FAIL fast-broken.*exit 11/u);
			assert.match(stderr, /error: deliberate concurrent failure/u);
			assert.doesNotMatch(stdout, /unreachable/u);
		},
	);
});

test('a concurrent group of successes reports every member and continues', () => {
	withRun(
		[
			{
				concurrent: [
					nodeStep('left', "process.stdout.write('# pass 1\\n# fail 0\\n# skipped 0\\n');"),
					nodeStep('right', "process.stdout.write('# pass 3\\n# fail 0\\n# skipped 0\\n');"),
				],
			},
			nodeStep('after', "process.stdout.write('reached\\n'); process.exitCode = 6;"),
		],
		({ status, stdout, directory, logs }) => {
			assert.equal(status, 6);
			assert.equal(logs.length, 3);
			assert.match(stdout, /PASS left.*1 passed/u);
			assert.match(stdout, /PASS right.*3 passed/u);
			assert.match(stdout, /FAIL after.*exit 6/u);
			assert.equal(readFileSync(join(directory, logs[2]), 'utf8'), 'reached\n');
		},
	);
});

test('unknown tasks and options are rejected instead of silently changing scope', () => {
	assert.throws(() => checkSteps('not-a-task'), /Unknown check task/u);
	assert.throws(() => checkVerbose(['--quiet']), /Unknown check options/u);
	assert.equal(checkVerbose([]), false);
	assert.equal(checkVerbose(['--', '--verbose']), true);
});

test('concurrent durations stop at each child close instead of the group barrier', () => {
	withRun(
		[
			{
				concurrent: [
					nodeStep('short', ''),
					nodeStep('long', 'await new Promise((resolve) => setTimeout(resolve, 1500));'),
				],
			},
		],
		({ status, stdout }) => {
			assert.equal(status, 0);
			const short = Number(stdout.match(/PASS short \(([\d.]+)s/u)?.[1]);
			const long = Number(stdout.match(/PASS long \(([\d.]+)s/u)?.[1]);
			assert.ok(long - short > 1, stdout);
		},
	);
});

test('nested concurrent long lines fit a small heap and retain both streams', () => {
	const size = 64 * 1024 * 1024;
	const leaf = nodeStep(
		'long',
		`import { once } from 'node:events';
		const block = 'x'.repeat(16384);
		for (let index = 0; index < ${size / 16384}; index += 1) {
			if (!process.stdout.write(block)) await once(process.stdout, 'drain');
		}
		process.stdout.write('終😀'); process.stderr.write('stderr tail');`,
	);
	const source = `import { runChecks } from ${JSON.stringify(runnerUrl)};
		process.exitCode = await runChecks({ name: 'inner', steps: [{ concurrent: [
			${JSON.stringify(leaf)}, ${JSON.stringify(nodeStep('peer', ''))}
		] }] });`;
	withRun(
		[
			{
				name: 'outer',
				command: process.execPath,
				args: ['--max-old-space-size=32', '--input-type=module', '-e', source],
			},
		],
		({ status, stderr, directory, logs }) => {
			assert.equal(status, 0, stderr);
			const output = readFileSync(join(directory, logs[0]), 'utf8');
			assert.equal(output.length - output.replaceAll('x', '').length, size);
			assert.match(output, /終😀/u);
			assert.match(output, /\[long\] stderr tail\n/u);
			assert.ok(output.split('\n').every((line) => line.length < 20_000));
		},
	);
});

test('concurrent forwarding waits for slow destination writes and preserves split UTF-8', () => {
	const leaf = nodeStep(
		'leaf',
		`import { once } from 'node:events';
		for (let index = 0; index < 128; index += 1) {
			if (!process.stdout.write('x'.repeat(8192))) await once(process.stdout, 'drain');
		}
		for (const byte of Buffer.from('😀終')) {
			process.stdout.write(Buffer.from([byte]));
			await new Promise((resolve) => setImmediate(resolve));
		}`,
	);
	const source = `import assert from 'node:assert/strict';
		import { Writable } from 'node:stream';
		import { runChecks } from ${JSON.stringify(runnerUrl)};
		let maxQueued = 0;
		let count = 0;
		let tail = '';
		const sink = new Writable({ highWaterMark: 1, write(chunk, encoding, callback) {
			const text = chunk.toString();
			count += text.split('x').length - 1;
			tail = (tail + text).slice(-100);
			setTimeout(() => { maxQueued = Math.max(maxQueued, sink.writableLength); callback(); }, 1);
		} });
		process.stdout.write = sink.write.bind(sink);
		const result = await runChecks({ name: 'inner', verbose: true, steps: [{ concurrent: [
			${JSON.stringify(leaf)}, ${JSON.stringify(nodeStep('peer', ''))}
		] }] });
		await new Promise((resolve) => sink.end(resolve));
		assert.equal(result, 0);
		assert.equal(count, 1024 * 1024);
		assert.ok(maxQueued < 128 * 1024, 'queued bytes: ' + maxQueued);
		assert.ok(tail.includes('😀終'), tail);`;
	withRun([nodeStep('slow-sink', source)], ({ status, stderr }) => assert.equal(status, 0, stderr));
});
