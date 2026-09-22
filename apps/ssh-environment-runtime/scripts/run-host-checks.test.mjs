import assert from 'node:assert/strict';
import { test } from 'node:test';

import { hostChecks } from './run-host-checks.mjs';

test('Windows keeps release checks and reports native execution as unavailable', () => {
	const build = hostChecks('build', 'win32', 'x64');
	assert.deepEqual(build.steps, []);
	assert.match(build.notice, /Skipping.*win32-x64/u);
	const tests = hostChecks('test', 'win32', 'x64');
	assert.equal(tests.steps.length, 1);
	assert.equal(tests.steps[0].command, process.execPath);
	assert.ok(tests.steps[0].args.includes('scripts/ssh-runtime-release.test.mjs'));
	assert.match(tests.notice, /test:cross remain required/u);
});

test('supported hosts build before unit tests and real process conformance', () => {
	for (const [platform, arch] of [
		['darwin', 'arm64'],
		['linux', 'x64'],
	]) {
		const build = hostChecks('build', platform, arch);
		assert.equal(build.notice, undefined);
		assert.deepEqual(build.steps[0].args, ['build', '-Doptimize=ReleaseSafe']);
		const tests = hostChecks('test', platform, arch);
		assert.equal(tests.notice, undefined);
		assert.deepEqual(
			tests.steps.map((step) => step.args),
			[
				['build'],
				['build', 'test'],
				['scripts/run-runtime-conformance.mjs'],
				['--test', 'scripts/ssh-runtime-release.test.mjs', 'scripts/run-host-checks.test.mjs'],
			],
		);
	}
});

test('unknown commands fail instead of silently skipping all checks', () => {
	assert.throws(() => hostChecks('typo', 'win32', 'x64'), /Unknown Runtime check/u);
	assert.match(hostChecks('test', 'darwin', 'x64').notice, /Skipping/u);
	assert.match(hostChecks('test', 'linux', 'arm64').notice, /Skipping/u);
});
