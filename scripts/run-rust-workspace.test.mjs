import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
	copyFileSync,
	cpSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

const repositoryRoot = new URL('../', import.meta.url);
const validLibrary = `/// Returns the fixture value.
/// \`\`\`
/// assert_eq!(runner_fixture::value(), 7);
/// \`\`\`
pub fn value() -> u8 { 7 }
#[cfg(test)]
mod tests;
`;

function run(root, command, args) {
	const result = spawnSync(command, args, {
		cwd: root,
		env: {
			...process.env,
			CARGO_NET_OFFLINE: 'true',
			CARGO_TARGET_DIR: join(root, 'target'),
		},
		encoding: 'utf8',
		timeout: 60_000,
	});
	assert.ifError(result.error);
	return { ...result, output: `${result.stdout}${result.stderr}` };
}

function withFixture(inspect) {
	const root = mkdtempSync(join(tmpdir(), 'orchbee-rust-runner-test-'));
	try {
		for (const directory of ['scripts', 'fixture/src', 'fixture/tests']) {
			mkdirSync(join(root, directory), { recursive: true });
		}
		for (const file of [
			'scripts/run-rust-workspace.mjs',
			'scripts/check-output.mjs',
			'rust-toolchain.toml',
		]) {
			copyFileSync(new URL(file, repositoryRoot), join(root, file));
		}
		// Exercise the actual workspace lint policy, with an isolated dependency-free member.
		// Cargo resolves local patch manifests even when this member does not consume them.
		cpSync(
			new URL('third-party/process-wrap', repositoryRoot),
			join(root, 'third-party/process-wrap'),
			{ recursive: true },
		);
		const manifest = readFileSync(new URL('Cargo.toml', repositoryRoot), 'utf8').replace(
			/^members = \[[\s\S]*?^\]/mu,
			'members = ["fixture"]',
		);
		writeFileSync(join(root, 'Cargo.toml'), manifest);
		writeFileSync(
			join(root, 'fixture/Cargo.toml'),
			`[package]
name = "runner-fixture"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
publish = false
[lints]
workspace = true
`,
		);
		writeFileSync(join(root, 'fixture/src/lib.rs'), validLibrary);
		writeFileSync(
			join(root, 'fixture/src/tests.rs'),
			'#[test] fn unit_contract() { assert_eq!(super::value(), 7); }\n',
		);
		writeFileSync(
			join(root, 'fixture/tests/contract.rs'),
			'#[test] fn integration_contract() { assert_eq!(runner_fixture::value(), 7); }\n',
		);
		const lock = run(root, 'cargo', ['generate-lockfile', '--offline']);
		assert.equal(lock.status, 0, lock.output);
		inspect(root);
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
}

function runTask(root, task) {
	return run(root, process.execPath, ['scripts/run-rust-workspace.mjs', task, '--verbose']);
}

test('the Rust gate runs unit, integration, and documentation behavior', () => {
	withFixture((root) => {
		const result = runTask(root, 'test');
		assert.equal(result.status, 0, result.output);
		assert.match(result.output, /test tests::unit_contract \.\.\. ok/u);
		assert.match(result.output, /test integration_contract \.\.\. ok/u);
		assert.match(result.output, /Doc-tests runner_fixture/u);
		assert.match(result.output, /lib\.rs - value.*ok/u);
	});
});

for (const [name, file, source] of [
	['unit', 'src/tests.rs', '#[test] fn unit_contract() { panic!("unit boundary failed"); }\n'],
	[
		'integration',
		'tests/contract.rs',
		'#[test] fn integration_contract() { panic!("integration boundary failed"); }\n',
	],
	[
		'documentation',
		'src/lib.rs',
		validLibrary.replace(
			'assert_eq!(runner_fixture::value(), 7);',
			'panic!("documentation boundary failed");',
		),
	],
]) {
	test(`a failing ${name} test fails the Rust gate`, () => {
		withFixture((root) => {
			writeFileSync(join(root, 'fixture', file), source);
			const result = runTask(root, 'test');
			assert.equal(result.status, 101, result.output);
			assert.ok(result.output.includes(`${name} boundary failed`), result.output);
		});
	});
}

test('a local allow cannot bypass the inherited authored unsafe-code policy', () => {
	withFixture((root) => {
		writeFileSync(
			join(root, 'fixture/src/lib.rs'),
			`${validLibrary}
#[allow(unsafe_code)]
pub fn copy_byte(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }
`,
		);
		const result = runTask(root, 'lint');
		assert.equal(result.status, 101, result.output);
		assert.match(result.output, /forbid/u);
		assert.match(result.output, /unsafe_code/u);
	});
});
