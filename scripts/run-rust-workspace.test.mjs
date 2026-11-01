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

import { readCargoVerificationGraph, selectVerificationScope } from './verification-scope.mjs';

const repositoryRoot = new URL('../', import.meta.url);
const windowsBoundaries = [
	{
		name: 'orchbee-windows-console-host',
		directory: 'crates/windows-console-host',
		module: 'sys',
		reason: 'reviewed Win32 calls and tagged INPUT_RECORD union reads are isolated here',
	},
	{
		name: 'orchbee-local-ipc',
		directory: 'crates/local-ipc',
		module: 'windows_native',
		reason: 'reviewed Win32 name queries, pipe counters, read watches and waits are isolated here',
	},
];
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

function admitDependencyFixture(root) {
	const manifest = join(root, 'Cargo.toml');
	writeFileSync(
		manifest,
		readFileSync(manifest, 'utf8')
			.replace('members = ["fixture"]', 'members = ["fixture", "peer"]')
			.replace(
				'exclude = ["third-party/process-wrap"]',
				'exclude = ["third-party/process-wrap", "vendor/runner-shared"]',
			)
			.replace(
				'[patch.crates-io]',
				'[patch.crates-io]\nrunner-shared = { path = "vendor/runner-shared" }',
			)
			.replace(
				'[workspace.dependencies]',
				'[workspace.dependencies]\nrunner-shared = { version = "1.0.0", default-features = false }',
			),
	);
	for (const directory of ['peer/src', 'vendor/runner-shared/src'])
		mkdirSync(join(root, directory), { recursive: true });
	writeFileSync(
		join(root, 'vendor/runner-shared/Cargo.toml'),
		'[package]\nname = "runner-shared"\nversion = "1.0.0"\nedition = "2024"\n[features]\ndefault = []\nextra = []\n',
	);
	writeFileSync(join(root, 'vendor/runner-shared/src/lib.rs'), 'pub fn shared() {}\n');
	writeFileSync(join(root, 'peer/src/lib.rs'), 'pub fn peer() {}\n');
	writeFileSync(
		join(root, 'peer/Cargo.toml'),
		`[package]
name = "runner-peer"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
[lints]
workspace = true
[dependencies]
runner-shared.workspace = true
`,
	);
	const lock = run(root, 'cargo', ['generate-lockfile', '--offline']);
	assert.equal(lock.status, 0, lock.output);
}

function lintDependencyFixture(root) {
	// Each case authors a different graph; prepare its lock before the runner
	// exercises the same --locked boundary as the repository gate.
	const lock = run(root, 'cargo', ['generate-lockfile', '--offline']);
	assert.equal(lock.status, 0, lock.output);
	return runTask(root, 'lint');
}

test('application-internal Cargo boundaries allow their owner and siblings but reject foreign consumers and publishing', () => {
	withFixture((root) => {
		const rootManifest = join(root, 'Cargo.toml');
		writeFileSync(
			rootManifest,
			readFileSync(rootManifest, 'utf8').replace(
				'members = ["fixture"]',
				'members = ["fixture", "apps/demo", "apps/*/crates/*"]',
			),
		);
		const manifest = (name, dependencies = '') =>
			`[package]\nname = "${name}"\nversion = "0.0.0"\nedition.workspace = true\nrust-version.workspace = true\npublish = false\n[lints]\nworkspace = true\n${dependencies}`;
		for (const [directory, name, source, dependencies] of [
			[
				'apps/demo',
				'demo',
				'pub fn value() -> u8 { demo_view::value() }',
				'[dependencies]\ndemo-view = { path = "crates/view" }\n',
			],
			[
				'apps/demo/crates/view',
				'demo-view',
				'pub fn value() -> u8 { demo_io::value() }',
				'[dependencies]\ndemo-io = { path = "../io" }\n',
			],
			['apps/demo/crates/io', 'demo-io', 'pub fn value() -> u8 { 1 }', ''],
		]) {
			mkdirSync(join(root, directory, 'src'), { recursive: true });
			writeFileSync(join(root, directory, 'Cargo.toml'), manifest(name, dependencies));
			writeFileSync(join(root, directory, 'src/lib.rs'), source + '\n');
		}
		const admitted = lintDependencyFixture(root);
		assert.equal(admitted.status, 0, admitted.output);
		const privateManifest = join(root, 'apps/demo/crates/io/Cargo.toml');
		writeFileSync(privateManifest, manifest('demo-io').replace('publish = false\n', ''));
		const publishable = lintDependencyFixture(root);
		assert.notEqual(publishable.status, 0);
		assert.match(publishable.output, /internal crates require publish = false/u);
		writeFileSync(privateManifest, manifest('misnamed'));
		const sibling = join(root, 'apps/demo/crates/view/Cargo.toml');
		const siblingManifest = readFileSync(sibling, 'utf8');
		writeFileSync(
			sibling,
			siblingManifest.replace('demo-io = { path', 'demo-io = { package = "misnamed", path'),
		);
		const misnamed = lintDependencyFixture(root);
		assert.notEqual(misnamed.status, 0);
		assert.match(misnamed.output, /internal crate name must be demo-io/u);
		writeFileSync(privateManifest, manifest('demo-io'));
		writeFileSync(sibling, siblingManifest);
		const outsider = join(root, 'fixture/Cargo.toml');
		const outsiderManifest = readFileSync(outsider, 'utf8');
		writeFileSync(
			outsider,
			outsiderManifest + '\n[dependencies]\ndemo-io = { path = "../apps/demo/crates/io" }\n',
		);
		const foreign = lintDependencyFixture(root);
		assert.notEqual(foreign.status, 0);
		assert.match(foreign.output, /internal crate cannot be consumed by fixture/u);
		writeFileSync(outsider, outsiderManifest);
		writeFileSync(
			privateManifest,
			manifest('demo-io', '[dev-dependencies]\ndemo-view = { path = "../view" }\n'),
		);
		const cyclic = lintDependencyFixture(root);
		assert.notEqual(cyclic.status, 0);
		assert.match(cyclic.output, /cyclic application-internal dependency/u);
		writeFileSync(privateManifest, manifest('demo-io'));
		writeFileSync(
			rootManifest,
			readFileSync(rootManifest, 'utf8').replace(
				'[patch.crates-io]',
				'[patch.crates-io]\ndemo-io = { path = "apps/demo/crates/io" }',
			),
		);
		writeFileSync(
			sibling,
			siblingManifest.replace(
				'demo-io = { path = "../io" }',
				'demo-io = { version = "0.0.0", default-features = false }',
			),
		);
		const patchedOwner = lintDependencyFixture(root);
		assert.equal(patchedOwner.status, 0, patchedOwner.output);
		const patchedGraph = readCargoVerificationGraph(root).filter((entry) =>
			entry.root.startsWith('apps/'),
		);
		const selected = selectVerificationScope(patchedGraph, ['apps/demo/crates/io/src/lib.rs']);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		assert.deepEqual(selected.cargo, ['demo', 'demo-io', 'demo-view']);
		writeFileSync(
			outsider,
			outsiderManifest +
				'\n[dependencies]\ndemo-io = { version = "0.0.0", default-features = false }\n',
		);
		const patchedForeign = lintDependencyFixture(root);
		assert.notEqual(patchedForeign.status, 0);
		assert.match(patchedForeign.output, /internal crate cannot be consumed by fixture/u);
		writeFileSync(
			rootManifest,
			readFileSync(rootManifest, 'utf8')
				.replace(
					'exclude = ["third-party/process-wrap"]',
					'exclude = ["third-party/process-wrap", "vendor/carrier"]',
				)
				.replace(
					'[patch.crates-io]',
					'[patch.crates-io]\nrunner-carrier = { path = "vendor/carrier" }',
				),
		);
		mkdirSync(join(root, 'vendor/carrier/src'), { recursive: true });
		writeFileSync(
			join(root, 'vendor/carrier/Cargo.toml'),
			'[package]\nname = "runner-carrier"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\ndemo-io = { version = "0.0.0", default-features = false }\n',
		);
		writeFileSync(
			join(root, 'vendor/carrier/src/lib.rs'),
			'pub fn value() -> u8 { demo_io::value() }\n',
		);
		writeFileSync(
			outsider,
			outsiderManifest +
				'\n[dependencies]\nrunner-carrier = { version = "0.0.0", default-features = false }\n',
		);
		const nonmember = lintDependencyFixture(root);
		assert.notEqual(nonmember.status, 0);
		assert.match(nonmember.output, /internal crate cannot be consumed by vendor\/carrier/u);
		assert.ok(
			readCargoVerificationGraph(root)
				.find((entry) => entry.root === 'fixture')
				.dependencies.includes('apps/demo/crates/io'),
		);
	});
});

for (const [scope, declaration, inherited] of [
	[
		'normal',
		'[dependencies]\nalias = { package = "runner-shared", version = "1.0.0", default-features = false } # workspace = true',
		'[dependencies]\n"runner-shared" = { workspace = true, features = ["extra"] }',
	],
	[
		'development',
		'["dev-dependencies".alias]\npackage = "runner-shared"\nversion = "1.0.0"\ndefault-features = false',
		'[dev-dependencies.runner-shared]\nworkspace = true',
	],
	[
		'build',
		'[build-dependencies]\nalias.package = "runner-shared"\nalias.version = "1.0.0"\nalias.default-features = false',
		'[build-dependencies]\nrunner-shared.workspace = true',
	],
	[
		'inactive target',
		`[target.'cfg(target_os="none")'.dependencies]\nalias = { package = "runner-shared", version = "1.0.0", default-features = false }`,
		`[target.'cfg(target_os="none")'.dependencies]\nrunner-shared.workspace = true`,
	],
]) {
	test(`the Rust lint gate requires workspace inheritance for a shared ${scope} dependency`, () => {
		withFixture((root) => {
			admitDependencyFixture(root);
			const manifest = join(root, 'fixture/Cargo.toml');
			const source = `${readFileSync(manifest, 'utf8')}
[package.metadata.policy]
example = '''
[dependencies]
alias.workspace = true
'''
`;
			writeFileSync(manifest, `${source}\n${declaration}\n`);
			const rejected = lintDependencyFixture(root);
			assert.notEqual(rejected.status, 0, rejected.output);
			assert.match(rejected.output, /fixture\/Cargo\.toml.*alias/u);
			assert.match(rejected.output, /inherit with workspace = true/u);
			assert.match(rejected.output, /docs\/RUST\.md#manifests-dependencies-and-generated-state/u);
			writeFileSync(manifest, `${source}\n${inherited}\n`);
			const accepted = lintDependencyFixture(root);
			assert.equal(accepted.status, 0, accepted.output);
		});
	});
}

test('the Rust lint gate rejects enabled defaults in root and member dependency declarations', () => {
	withFixture((root) => {
		admitDependencyFixture(root);
		const rootManifest = join(root, 'Cargo.toml');
		const source = readFileSync(rootManifest, 'utf8');
		for (const defaults of ['', ', default-features = true']) {
			writeFileSync(
				rootManifest,
				source.replace(
					'runner-shared = { version = "1.0.0", default-features = false }',
					`runner-shared = { version = "1.0.0"${defaults} }`,
				),
			);
			const rejected = lintDependencyFixture(root);
			assert.notEqual(rejected.status, 0, rejected.output);
			assert.match(rejected.output, /Cargo\.toml \[workspace\.dependencies\] runner-shared/u);
			assert.match(rejected.output, /default-features = false/u);
		}
		writeFileSync(rootManifest, source);
		const memberManifest = join(root, 'fixture/Cargo.toml');
		const member = readFileSync(memberManifest, 'utf8');
		for (const declaration of [
			'runner-shared = { workspace = true, default-features = true }',
			'runner-shared = "1.0.0"',
		]) {
			writeFileSync(memberManifest, `${member}\n[dependencies]\n${declaration}\n`);
			const rejected = lintDependencyFixture(root);
			assert.notEqual(rejected.status, 0, rejected.output);
			assert.match(rejected.output, /fixture\/Cargo\.toml.*must disable default features/u);
		}
		writeFileSync(
			memberManifest,
			`${member}\n[dependencies]\nrunner-shared.workspace = true\nrunner-peer = { path = "../peer" }\n`,
		);
		const accepted = lintDependencyFixture(root);
		assert.equal(accepted.status, 0, accepted.output);
	});
});

test('the Rust lint gate counts distinct consumers and checks unused workspace defaults', () => {
	withFixture((root) => {
		admitDependencyFixture(root);
		const peer = join(root, 'peer/Cargo.toml');
		writeFileSync(peer, readFileSync(peer, 'utf8').replace('runner-shared.workspace = true', ''));
		const member = join(root, 'fixture/Cargo.toml');
		writeFileSync(
			member,
			`${readFileSync(member, 'utf8')}
[dependencies]
runner-shared = { version = "1.0.0", default-features = false }
[dev-dependencies]
runner-shared = { version = "1.0.0", default-features = false }
`,
		);
		const accepted = lintDependencyFixture(root);
		assert.equal(accepted.status, 0, accepted.output);
		const manifest = join(root, 'Cargo.toml');
		writeFileSync(
			manifest,
			readFileSync(manifest, 'utf8').replace(
				'[workspace.dependencies]',
				'[workspace.dependencies]\nunused = "1.0.0"',
			),
		);
		const rejected = lintDependencyFixture(root);
		assert.notEqual(rejected.status, 0, rejected.output);
		assert.match(
			rejected.output,
			/\[workspace\.dependencies\] unused must declare default-features = false/u,
		);
	});
});

function admitWindowsException(root, boundary) {
	const { name, directory, module, reason } = boundary;
	const rootManifest = join(root, 'Cargo.toml');
	writeFileSync(
		rootManifest,
		readFileSync(rootManifest, 'utf8').replace('members = [', `members = ["${directory}", `),
	);
	mkdirSync(join(root, directory, 'src', module), { recursive: true });
	writeFileSync(
		join(root, directory, 'Cargo.toml'),
		`[package]
name = "${name}"
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
		join(root, directory, 'src/lib.rs'),
		`#![deny(unsafe_code)]

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "${reason}"
)]
mod ${module};

#[cfg(windows)]
pub fn copied(value: &u8) -> u8 { ${module}::copy_byte(value) }

#[cfg(not(windows))]
pub fn copied(value: &u8) -> u8 { *value }
`,
	);
	writeFileSync(
		join(root, directory, `src/${module}.rs`),
		`pub(super) fn copy_byte(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
`,
	);
	writeFileSync(
		join(root, directory, 'src', module, 'tests.rs'),
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
		assert.match(result.output, /outside the accepted private Win32 boundaries/u);
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

test('the Rust lint gate admits both canonical private Win32 unsafe boundaries together', () => {
	withFixture((root) => {
		for (const boundary of windowsBoundaries) admitWindowsException(root, boundary);
		const result = runTask(root, 'lint');
		assert.equal(result.status, 0, result.output);
	});
});

for (const [name, mutate, diagnostic] of [
	[
		'a public adapter module',
		(root, boundary) => {
			const path = join(root, boundary.directory, 'src/lib.rs');
			writeFileSync(
				path,
				readFileSync(path, 'utf8').replace(
					`mod ${boundary.module};`,
					`pub mod ${boundary.module};`,
				),
			);
		},
		(boundary) => `private mod ${boundary.module}`,
	],
	[
		'a commented declaration paired with a conditional escape',
		(root, boundary) => {
			const path = join(root, boundary.directory, 'src/lib.rs');
			const declaration = `#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "${boundary.reason}"
)]
mod ${boundary.module};`;
			writeFileSync(
				path,
				readFileSync(path, 'utf8').replace(
					declaration,
					`/* ${declaration} */
#[cfg(windows)]
#[cfg_attr(windows, allow(unsafe_code))]
mod ${boundary.module};`,
				),
			);
		},
		(boundary) => `reasoned unsafe_code allow on private mod ${boundary.module}`,
	],
	[
		'a missing crate-wide deny',
		(root, boundary) => {
			const path = join(root, boundary.directory, 'src/lib.rs');
			writeFileSync(path, readFileSync(path, 'utf8').replace('#![deny(unsafe_code)]\n\n', ''));
		},
		/crate-wide deny\(unsafe_code\)/u,
	],
	[
		'an unsafe escape in a safe sibling',
		(root, boundary) => {
			const path = join(root, boundary.directory, 'src/lib.rs');
			writeFileSync(path, `${readFileSync(path, 'utf8')}\nmod sibling;\n`);
			writeFileSync(
				join(root, boundary.directory, 'src/sibling.rs'),
				'#![allow(unsafe_code)]\npub(super) fn copy(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }\n',
			);
		},
		(boundary) =>
			`Only ${boundary.directory}/src/lib.rs may contain the canonical unsafe_code allow`,
	],
	[
		'an unsafe Rust 2024 attribute in a safe sibling',
		(root, boundary) => {
			const path = join(root, boundary.directory, 'src/lib.rs');
			writeFileSync(path, `${readFileSync(path, 'utf8')}\nmod symbol;\n`);
			writeFileSync(
				join(root, boundary.directory, 'src/symbol.rs'),
				'#[unsafe(no_mangle)]\npub extern "C" fn fixture_symbol() {}\n',
			);
		},
		(boundary) => `contains unsafe Rust outside ${boundary.directory}/src/${boundary.module}.rs`,
	],
	[
		'a missing deny on the safe ABI test child',
		(root, boundary) => {
			const path = join(root, boundary.directory, `src/${boundary.module}.rs`);
			writeFileSync(path, readFileSync(path, 'utf8').replace('#[deny(unsafe_code)]\n', ''));
		},
		/must attach its adjacent tests with deny\(unsafe_code\)/u,
	],
	[
		'a commented-out safe ABI test attachment',
		(root, boundary) => {
			const path = join(root, boundary.directory, `src/${boundary.module}.rs`);
			const declaration = '#[cfg(test)]\n#[deny(unsafe_code)]\nmod tests;';
			writeFileSync(path, readFileSync(path, 'utf8').replace(declaration, `/* ${declaration} */`));
		},
		/must attach its adjacent tests with deny\(unsafe_code\)/u,
	],
	[
		'a nested unsafe adapter child',
		(root, boundary) => {
			writeFileSync(
				join(root, boundary.directory, 'src', boundary.module, 'hidden.rs'),
				'pub(super) fn copy(value: &u8) -> u8 { unsafe { std::ptr::read(value) } }\n',
			);
		},
		(boundary) =>
			`Only the denied safe test child may live below ${boundary.directory}/src/${boundary.module}`,
	],
	[
		'a widened mirrored Clippy policy',
		(root, boundary) => {
			const path = join(root, boundary.directory, 'Cargo.toml');
			writeFileSync(path, readFileSync(path, 'utf8').replace('wildcard_imports = "deny"', ''));
		},
		/mirrored Clippy policy/u,
	],
	[
		'a different package at the approved path',
		(root, boundary) => {
			const path = join(root, boundary.directory, 'Cargo.toml');
			writeFileSync(path, readFileSync(path, 'utf8').replace(boundary.name, 'unapproved-boundary'));
			const lock = run(root, 'cargo', ['generate-lockfile', '--offline']);
			assert.equal(lock.status, 0, lock.output);
		},
		/requires its exact package/u,
	],
]) {
	for (const boundary of windowsBoundaries) {
		test(`the Rust lint gate rejects ${name} in ${boundary.name}`, () => {
			withFixture((root) => {
				admitWindowsException(root, boundary);
				mutate(root, boundary);
				const result = runTask(root, 'lint');
				assert.notEqual(result.status, 0, result.output);
				if (typeof diagnostic === 'function')
					assert.ok(result.output.includes(diagnostic(boundary)), result.output);
				else assert.match(result.output, diagnostic);
				assert.ok(result.output.includes(boundary.directory), result.output);
			});
		});
	}
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

test('the Rust lint gate sees product-only warnings that a test-support dependency would hide', () => {
	withFixture((root) => {
		// A sibling's development dependency enables test-support, as an application's tests do;
		// feature unification under --all-targets would then mask the product build.
		const rootManifest = join(root, 'Cargo.toml');
		writeFileSync(
			rootManifest,
			readFileSync(rootManifest, 'utf8').replace(
				'members = ["fixture"]',
				'members = ["fixture", "consumer"]',
			),
		);
		const fixtureManifest = join(root, 'fixture/Cargo.toml');
		writeFileSync(
			fixtureManifest,
			`${readFileSync(fixtureManifest, 'utf8')}[features]\ntest-support = []\n`,
		);
		mkdirSync(join(root, 'consumer/src'), { recursive: true });
		writeFileSync(
			join(root, 'consumer/Cargo.toml'),
			'[package]\nname = "runner-consumer"\nversion = "0.0.0"\nedition.workspace = true\nrust-version.workspace = true\npublish = false\n[lints]\nworkspace = true\n[dev-dependencies]\nrunner-fixture = { path = "../fixture", features = ["test-support"] }\n',
		);
		writeFileSync(join(root, 'consumer/src/lib.rs'), 'pub fn consumer() {}\n');
		const source = validLibrary.replace(
			'pub fn value() -> u8 { 7 }',
			'mod values { pub(crate) const SEVEN: u8 = 7; }\nuse values::SEVEN;\npub fn value() -> u8 { 7 }\n#[cfg(feature = "test-support")]\npub fn support() -> u8 { SEVEN }',
		);
		writeFileSync(join(root, 'fixture/src/lib.rs'), source);
		const hidden = lintDependencyFixture(root);
		assert.equal(hidden.status, 101, hidden.output);
		assert.match(hidden.output, /unused import/u);

		writeFileSync(
			join(root, 'fixture/src/lib.rs'),
			source.replace('pub fn value() -> u8 { 7 }', 'pub fn value() -> u8 { SEVEN }'),
		);
		const used = runTask(root, 'lint');
		assert.equal(used.status, 0, used.output);
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
