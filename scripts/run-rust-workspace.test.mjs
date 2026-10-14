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

function admitWindowsConsoleException(root) {
	const rootManifest = join(root, 'Cargo.toml');
	writeFileSync(
		rootManifest,
		readFileSync(rootManifest, 'utf8').replace(
			'members = ["fixture"]',
			'members = ["fixture", "crates/windows-console-host"]',
		),
	);
	mkdirSync(join(root, 'crates/windows-console-host/src/sys'), { recursive: true });
	writeFileSync(
		join(root, 'crates/windows-console-host/Cargo.toml'),
		`[package]
name = "orchbee-windows-console-host"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
publish = false

[lints.rust]
unsafe_code = "deny"

[lints.clippy]
all = { level = "deny", priority = -1 }
wildcard_imports = "deny"
`,
	);
	writeFileSync(
		join(root, 'crates/windows-console-host/src/lib.rs'),
		`#![deny(unsafe_code)]

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "reviewed Win32 calls and tagged INPUT_RECORD union reads are isolated here"
)]
mod sys;

#[cfg(windows)]
pub fn copied(value: &u8) -> u8 { sys::copy_byte(value) }

#[cfg(not(windows))]
pub fn copied(value: &u8) -> u8 { *value }
`,
	);
	writeFileSync(
		join(root, 'crates/windows-console-host/src/sys.rs'),
		`pub(super) fn copy_byte(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
`,
	);
	writeFileSync(
		join(root, 'crates/windows-console-host/src/sys/tests.rs'),
		`//! Safe boundary fixture.

#[test]
fn copies_a_byte() { assert_eq!(super::copy_byte(&7), 7); }
`,
	);
	const lock = run(root, 'cargo', ['generate-lockfile', '--offline']);
	assert.equal(lock.status, 0, lock.output);
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
		assert.notEqual(result.status, 0, result.output);
		assert.match(result.output, /forbid/u);
		assert.match(result.output, /unsafe_code/u);
	});
});

test('cfg-disabled authored unsafe Rust cannot hide in an ordinary workspace member', () => {
	withFixture((root) => {
		writeFileSync(
			join(root, 'fixture/src/lib.rs'),
			`${validLibrary}
#[cfg(target_os = "none")]
pub fn copy_byte(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }
`,
		);
		const result = runTask(root, 'lint');
		assert.notEqual(result.status, 0, result.output);
		assert.match(result.output, /authored unsafe Rust/u);
		assert.match(result.output, /src\/sys\.rs exception/u);
	});
});

test('unsafe-like text in comments and raw strings does not invent authored Rust', () => {
	withFixture((root) => {
		writeFileSync(
			join(root, 'fixture/src/lib.rs'),
			`${validLibrary}
// #[allow(unsafe_code)] unsafe { hidden(); }
pub const POLICY_TEXT: &str = r##"#[cfg_attr(windows, allow(unsafe_code))] unsafe { hidden(); }"##;
`,
		);
		const result = runTask(root, 'lint');
		assert.equal(result.status, 0, result.output);
	});
});

test('the Rust lint gate admits only the canonical private Windows console unsafe boundary', () => {
	withFixture((root) => {
		admitWindowsConsoleException(root);
		const result = runTask(root, 'lint');
		assert.equal(result.status, 0, result.output);
	});
});

for (const [name, mutate, diagnostic] of [
	[
		'a public sys module',
		(root) => {
			const path = join(root, 'crates/windows-console-host/src/lib.rs');
			writeFileSync(path, readFileSync(path, 'utf8').replace('mod sys;', 'pub mod sys;'));
		},
		/private mod sys/u,
	],
	[
		'a commented declaration paired with a conditional escape',
		(root) => {
			const path = join(root, 'crates/windows-console-host/src/lib.rs');
			const declaration = `#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "reviewed Win32 calls and tagged INPUT_RECORD union reads are isolated here"
)]
mod sys;`;
			writeFileSync(
				path,
				readFileSync(path, 'utf8').replace(
					declaration,
					`/* ${declaration} */
#[cfg(windows)]
#[cfg_attr(windows, allow(unsafe_code))]
mod sys;`,
				),
			);
		},
		/reasoned unsafe_code allow on private mod sys/u,
	],
	[
		'a missing crate-wide deny',
		(root) => {
			const path = join(root, 'crates/windows-console-host/src/lib.rs');
			writeFileSync(path, readFileSync(path, 'utf8').replace('#![deny(unsafe_code)]\n\n', ''));
		},
		/crate-wide deny\(unsafe_code\)/u,
	],
	[
		'an unsafe escape in a safe sibling',
		(root) => {
			const path = join(root, 'crates/windows-console-host/src/lib.rs');
			writeFileSync(path, `${readFileSync(path, 'utf8')}\nmod sibling;\n`);
			writeFileSync(
				join(root, 'crates/windows-console-host/src/sibling.rs'),
				'#![allow(unsafe_code)]\npub(super) fn copy(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }\n',
			);
		},
		/Only crates\/windows-console-host\/src\/lib\.rs/u,
	],
	[
		'an unsafe Rust 2024 attribute in a safe sibling',
		(root) => {
			const path = join(root, 'crates/windows-console-host/src/lib.rs');
			writeFileSync(path, `${readFileSync(path, 'utf8')}\nmod symbol;\n`);
			writeFileSync(
				join(root, 'crates/windows-console-host/src/symbol.rs'),
				'#[unsafe(no_mangle)]\npub extern "C" fn fixture_symbol() {}\n',
			);
		},
		/contains unsafe Rust outside crates\/windows-console-host\/src\/sys\.rs/u,
	],
	[
		'a missing deny on the safe ABI test child',
		(root) => {
			const path = join(root, 'crates/windows-console-host/src/sys.rs');
			writeFileSync(path, readFileSync(path, 'utf8').replace('#[deny(unsafe_code)]\n', ''));
		},
		/must attach its adjacent tests with deny\(unsafe_code\)/u,
	],
	[
		'a commented-out safe ABI test attachment',
		(root) => {
			const path = join(root, 'crates/windows-console-host/src/sys.rs');
			const declaration = '#[cfg(test)]\n#[deny(unsafe_code)]\nmod tests;';
			writeFileSync(path, readFileSync(path, 'utf8').replace(declaration, `/* ${declaration} */`));
		},
		/must attach its adjacent tests with deny\(unsafe_code\)/u,
	],
	[
		'a nested unsafe sys child',
		(root) => {
			writeFileSync(
				join(root, 'crates/windows-console-host/src/sys/hidden.rs'),
				'pub(super) fn copy(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }\n',
			);
		},
		/Only the denied safe test child may live below src\/sys/u,
	],
	[
		'a widened mirrored Clippy policy',
		(root) => {
			const path = join(root, 'crates/windows-console-host/Cargo.toml');
			writeFileSync(path, readFileSync(path, 'utf8').replace('wildcard_imports = "deny"', ''));
		},
		/mirrored Clippy policy/u,
	],
]) {
	test(`the Rust lint gate rejects ${name}`, () => {
		withFixture((root) => {
			admitWindowsConsoleException(root);
			mutate(root);
			const result = runTask(root, 'lint');
			assert.notEqual(result.status, 0, result.output);
			assert.match(result.output, diagnostic);
		});
	});
}

test('the Rust gate rejects production module globs and accepts explicit imports with parent test scope', () => {
	withFixture((root) => {
		const source = validLibrary.replace(
			'pub fn value() -> u8 { 7 }',
			'mod values { pub fn seven() -> u8 { 7 } }\nuse values::*;\npub fn value() -> u8 { seven() }',
		);
		writeFileSync(join(root, 'fixture/src/lib.rs'), source);
		const wildcard = runTask(root, 'lint');
		assert.equal(wildcard.status, 101, wildcard.output);
		assert.match(wildcard.output, /clippy::wildcard[-_]imports/u);

		writeFileSync(join(root, 'fixture/src/lib.rs'), source.replace('values::*', 'values::seven'));
		writeFileSync(
			join(root, 'fixture/src/tests.rs'),
			'use super::*;\n#[test] fn unit_contract() { assert_eq!(value(), 7); }\n',
		);
		const explicit = runTask(root, 'lint');
		assert.equal(explicit.status, 0, explicit.output);
	});
});

for (const [name, declaration] of [
	['active', 'mod values;\npub fn value() -> u8 { values::seven() }'],
	['inactive target', '#[cfg(target_os = "none")]\nmod values;\npub fn value() -> u8 { 7 }'],
]) {
	test(`the Rust lint gate rejects src/mod.rs for an ${name} module`, () => {
		withFixture((root) => {
			mkdirSync(join(root, 'fixture/src/values'));
			writeFileSync(join(root, 'fixture/src/values/mod.rs'), 'pub fn seven() -> u8 { 7 }\n');
			writeFileSync(
				join(root, 'fixture/src/lib.rs'),
				validLibrary.replace('pub fn value() -> u8 { 7 }', declaration),
			);
			const result = runTask(root, 'lint');
			assert.notEqual(result.status, 0, result.output);
			assert.match(result.output, /fixture\/src\/values\/mod\.rs/u);
			assert.match(result.output, /docs\/RUST\.md#modules-and-visibility/u);
		});
	});
}

test('the Rust lint gate admits adjacent source modules and integration support without policing nonmembers', () => {
	withFixture((root) => {
		for (const directory of ['fixture/src/values', 'fixture/tests/support', 'nonmember/src'])
			mkdirSync(join(root, directory), { recursive: true });
		writeFileSync(join(root, 'fixture/src/values/numbers.rs'), 'pub(super) const SEVEN: u8 = 7;\n');
		writeFileSync(
			join(root, 'fixture/src/values.rs'),
			'mod numbers;\npub fn seven() -> u8 { numbers::SEVEN }\n',
		);
		writeFileSync(
			join(root, 'fixture/src/lib.rs'),
			validLibrary.replace(
				'pub fn value() -> u8 { 7 }',
				'mod values;\npub fn value() -> u8 { values::seven() }',
			),
		);
		writeFileSync(join(root, 'fixture/tests/support/mod.rs'), 'pub fn expected() -> u8 { 7 }\n');
		writeFileSync(
			join(root, 'fixture/tests/contract.rs'),
			'mod support;\n#[test] fn integration_contract() { assert_eq!(runner_fixture::value(), support::expected()); }\n',
		);
		writeFileSync(join(root, 'nonmember/src/mod.rs'), 'pub fn foreign() {}\n');
		const result = runTask(root, 'lint');
		assert.equal(result.status, 0, result.output);
	});
});

test('explicit package selection retains unit, integration and doc tests and rejects unknown members', () => {
	withFixture((root) => {
		const selected = run(root, process.execPath, [
			'scripts/run-rust-workspace.mjs',
			'test',
			'--package',
			'runner-fixture',
			'--verbose',
		]);
		assert.equal(selected.status, 0, selected.output);
		for (const name of ['unit_contract', 'integration_contract', 'Doc-tests runner_fixture'])
			assert.ok(selected.output.includes(name));
		const unknown = run(root, process.execPath, [
			'scripts/run-rust-workspace.mjs',
			'test',
			'--package',
			'missing',
		]);
		assert.notEqual(unknown.status, 0);
		assert.match(unknown.output, /Unknown or ambiguous Cargo workspace package/u);
	});
});

test('affected Rust docs include only eligible library targets in mixed and binary-only selections', () => {
	withFixture((root) => {
		const manifest = join(root, 'Cargo.toml');
		writeFileSync(
			manifest,
			readFileSync(manifest, 'utf8').replace(
				'members = ["fixture"]',
				'members = ["fixture", "binary"]',
			),
		);
		mkdirSync(join(root, 'binary/src'), { recursive: true });
		writeFileSync(
			join(root, 'binary/Cargo.toml'),
			'[package]\nname = "runner-binary"\nversion = "0.0.0"\nedition = "2024"\n',
		);
		writeFileSync(
			join(root, 'binary/src/main.rs'),
			'fn main() {}\n#[test] fn binary_contract() {}\n',
		);
		const libraryManifest = join(root, 'fixture/Cargo.toml');
		writeFileSync(
			libraryManifest,
			readFileSync(libraryManifest, 'utf8') + '\n[lib]\nname = "renamed_library"\n',
		);
		for (const file of ['fixture/src/lib.rs', 'fixture/tests/contract.rs']) {
			const target = join(root, file);
			writeFileSync(
				target,
				readFileSync(target, 'utf8').replaceAll('runner_fixture', 'renamed_library'),
			);
		}
		assert.equal(run(root, 'cargo', ['generate-lockfile', '--offline']).status, 0);
		const args = [
			'scripts/run-rust-workspace.mjs',
			'test',
			'--verbose',
			'--package',
			'runner-binary',
		];
		const binary = run(root, process.execPath, args);
		assert.equal(binary.status, 0, binary.output);
		assert.match(binary.output, /binary_contract/u);
		assert.doesNotMatch(binary.output, /cargo documentation tests/u);
		const mixed = run(root, process.execPath, [...args, '--package', 'runner-fixture']);
		assert.equal(mixed.status, 0, mixed.output);
		assert.match(mixed.output, /Doc-tests renamed_library/u);
		assert.match(mixed.output, /binary_contract/u);
		writeFileSync(libraryManifest, readFileSync(libraryManifest, 'utf8') + 'doctest = false\n');
		const disabled = run(root, process.execPath, [...args, '--package', 'runner-fixture']);
		assert.equal(disabled.status, 0, disabled.output);
		assert.doesNotMatch(disabled.output, /cargo documentation tests/u);
	});
});
