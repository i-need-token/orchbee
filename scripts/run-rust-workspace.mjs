import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';

import { checkVerbose, runChecks } from './check-output.mjs';

const repositoryRoot = resolve(import.meta.dirname, '..');
const windowsConsoleHost = Object.freeze({
	name: 'orchbee-windows-console-host',
	manifest: 'crates/windows-console-host/Cargo.toml',
	library: 'crates/windows-console-host/src/lib.rs',
	system: 'crates/windows-console-host/src/sys.rs',
	systemTests: 'crates/windows-console-host/src/sys/tests.rs',
});
const windowsConsoleUnsafeReason =
	'reviewed Win32 calls and tagged INPUT_RECORD union reads are isolated here';
const task = process.argv[2];
const selectedPackages = [];
const presentationArguments = [];
const taskArguments = process.argv.slice(3).filter((argument) => argument !== '--');
for (let index = 0; index < taskArguments.length; index += 1) {
	if (taskArguments[index] === '--package') {
		const name = taskArguments[++index];
		if (task !== 'test' || name === undefined || !/^[a-z\d][a-z\d_-]*$/u.test(name))
			throw new Error('--package requires a Cargo package name and the test task');
		selectedPackages.push(name);
	} else presentationArguments.push(taskArguments[index]);
}
const verbose = checkVerbose(presentationArguments);
const commands = new Map([
	['build', ['build', '--workspace', '--all-targets', '--all-features', '--locked']],
	['format:check', ['fmt', '--all', '--', '--check']],
	['format', ['fmt', '--all']],
	[
		'lint',
		[
			'clippy',
			'--workspace',
			'--all-targets',
			'--all-features',
			'--locked',
			'--',
			'-D',
			'warnings',
		],
	],
	['test', ['test', '--workspace', '--all-targets', '--all-features', '--locked']],
]);
const command = commands.get(task);

if (command === undefined) {
	throw new Error(`Unknown Rust workspace task: ${task ?? '<missing>'}`);
}

const toolchain = readFileSync(resolve(repositoryRoot, 'rust-toolchain.toml'), 'utf8');
const pinnedChannel = toolchain.match(/^channel = "([^"]+)"$/mu)?.[1];
if (pinnedChannel === undefined) {
	throw new Error('rust-toolchain.toml must declare one exact channel');
}

const rustcVersion = execFileSync('rustc', ['--version'], {
	cwd: repositoryRoot,
	encoding: 'utf8',
}).trim();
if (!rustcVersion.startsWith(`rustc ${pinnedChannel} `)) {
	throw new Error(`Expected rustc ${pinnedChannel}, received ${rustcVersion}`);
}

const metadata = JSON.parse(
	execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps', '--locked', '--quiet'], {
		cwd: repositoryRoot,
		encoding: 'utf8',
	}),
);

if (!Array.isArray(metadata.packages)) {
	throw new TypeError('Cargo metadata did not contain a packages array');
}

if (task === 'lint') {
	const workspacePackages = metadata.packages.filter((entry) =>
		metadata.workspace_members.includes(entry.id),
	);
	enforceDependencyPolicy(workspacePackages);
	enforceAuthoredUnsafePolicy(workspacePackages);
	const legacyModules = workspacePackages
		.flatMap((entry) => legacySourceModules(join(dirname(entry.manifest_path), 'src')))
		.map((path) => relative(repositoryRoot, path).replaceAll('\\', '/'))
		.toSorted();
	if (legacyModules.length > 0) {
		throw new Error(
			`Rust source modules use name.rs beside name/; replace these mod.rs files:\n${legacyModules.join('\n')}\nSee docs/RUST.md#modules-and-visibility`,
		);
	}
}

for (const name of selectedPackages) {
	if (
		metadata.packages.filter(
			(entry) => entry.name === name && metadata.workspace_members.includes(entry.id),
		).length !== 1
	)
		throw new Error(`Unknown or ambiguous Cargo workspace package: ${name}`);
}
const workspaceArguments = selectedPackages.length
	? [...new Set(selectedPackages)].flatMap((name) => ['--package', name])
	: ['--workspace'];
const scopedCommand = command.flatMap((argument) =>
	argument === '--workspace' ? workspaceArguments : [argument],
);
const documentationArguments = selectedPackages.length
	? metadata.packages
			.filter(
				(entry) =>
					selectedPackages.includes(entry.name) &&
					entry.targets.some(
						(target) =>
							target.doctest &&
							target.kind.some((kind) =>
								['lib', 'rlib', 'dylib', 'staticlib', 'cdylib', 'proc-macro'].includes(kind),
							),
					),
			)
			.flatMap((entry) => ['--package', entry.name])
	: ['--workspace'];

if (metadata.packages.length === 0) {
	process.stdout.write(`Verified empty Rust workspace for ${task}\n`);
} else {
	process.exitCode = await runChecks({
		name: `rust:${task}`,
		steps: [
			{ name: `cargo ${task}`, command: 'cargo', args: scopedCommand },
			...(task === 'test' && documentationArguments.length
				? [
						{
							name: 'cargo documentation tests',
							command: 'cargo',
							args: ['test', ...documentationArguments, '--doc', '--all-features', '--locked'],
						},
					]
				: []),
		],
		cwd: repositoryRoot,
		verbose,
	});
}

function legacySourceModules(directory) {
	if (!existsSync(directory)) return [];
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = join(directory, entry.name);
		if (entry.isDirectory()) return legacySourceModules(path);
		return entry.name === 'mod.rs' ? [path] : [];
	});
}

function enforceDependencyPolicy(workspacePackages) {
	const rootDeclarations = dependencyDeclarations(
		readFileSync(join(repositoryRoot, 'Cargo.toml'), 'utf8'),
		true,
	);
	for (const declaration of rootDeclarations.values()) {
		if (!declaration.fields.has('path') && declaration.fields.get('default-features') !== 'false') {
			throw new Error(
				`Cargo.toml [workspace.dependencies] ${declaration.name} must declare default-features = false and list required features; see docs/RUST.md#manifests-dependencies-and-generated-state`,
			);
		}
	}
	const consumers = new Map();
	for (const entry of workspacePackages) {
		for (const dependency of entry.dependencies) {
			if (dependency.source === null) continue;
			const members = consumers.get(dependency.name) ?? new Set();
			members.add(entry.id);
			consumers.set(dependency.name, members);
		}
	}
	for (const entry of workspacePackages) {
		const declarations = dependencyDeclarations(readFileSync(entry.manifest_path, 'utf8'), false);
		for (const dependency of entry.dependencies) {
			if (dependency.source === null) continue;
			const kind = dependency.kind === null ? 'dependencies' : `${dependency.kind}-dependencies`;
			const name = dependency.rename ?? dependency.name;
			const location = `${relativeManifest(entry.manifest_path)} [${kind}] ${name}${dependency.target === null ? '' : ` (${dependency.target})`}`;
			const declaration = declarations.get(dependencyDeclarationKey(kind, dependency.target, name));
			if (declaration === undefined) {
				throw new Error(
					`Cannot locate ${location}; use a Cargo dependency table, dotted key, or inline table; see docs/RUST.md#manifests-dependencies-and-generated-state`,
				);
			}
			if (dependency.uses_default_features !== false) {
				throw new Error(
					`${location} must disable default features with default-features = false, directly or through workspace inheritance; see docs/RUST.md#manifests-dependencies-and-generated-state`,
				);
			}
			if (
				consumers.get(dependency.name).size > 1 &&
				declaration.fields.get('workspace') !== 'true'
			) {
				throw new Error(
					`${location} shares third-party package ${dependency.name} across workspace members; declare it in [workspace.dependencies] and inherit with workspace = true; see docs/RUST.md#manifests-dependencies-and-generated-state`,
				);
			}
		}
	}
}

function dependencyDeclarationKey(kind, target, name) {
	return JSON.stringify([kind, tomlPolicyTokens(target ?? '').join(''), name]);
}

function dependencyDeclarations(manifest, workspace) {
	const declarations = new Map();
	const tokens = tomlPolicyTokens(manifest);
	let index = 0;
	let table = [];
	const key = () => {
		const parts = [tomlPolicyKey(tokens[index++])];
		while (tokens[index] === '.') {
			index += 1;
			parts.push(tomlPolicyKey(tokens[index++]));
		}
		return parts;
	};
	const record = (path, fieldValue) => {
		let start = 1;
		let target = null;
		if (workspace) {
			if (path[0] !== 'workspace' || path[1] !== 'dependencies') return;
			start = 2;
		} else if (path[0] === 'target') {
			target = path[1];
			start = 3;
		}
		const kind = path[start - 1];
		if (!['dependencies', 'dev-dependencies', 'build-dependencies'].includes(kind)) return;
		const name = path[start];
		if (name === undefined) return;
		const identity = dependencyDeclarationKey(kind, target, name);
		const declaration = declarations.get(identity) ?? { name, fields: new Map() };
		if (path.length === start + 2) declaration.fields.set(path[start + 1], fieldValue);
		declarations.set(identity, declaration);
	};
	const value = (path) => {
		record(path, undefined);
		if (tokens[index] === '{') {
			index += 1;
			while (index < tokens.length && tokens[index] !== '}') {
				if (tokens[index] === '\n' || tokens[index] === ',') {
					index += 1;
					continue;
				}
				const child = key();
				index += 1;
				value([...path, ...child]);
			}
			index += 1;
			return;
		}
		const begin = index;
		let depth = 0;
		while (index < tokens.length) {
			const token = tokens[index];
			if (depth === 0 && ['\n', ',', '}'].includes(token)) break;
			if (token === '[' || token === '{') depth += 1;
			if (token === ']' || token === '}') depth -= 1;
			index += 1;
		}
		record(path, index === begin + 1 ? tokens[begin] : undefined);
	};
	// Cargo has already validated TOML. Retain declaration keys and policy flags rather
	// than inferring inheritance from metadata, which flattens workspace references.
	while (index < tokens.length) {
		if (tokens[index] === '\n') index += 1;
		else if (tokens[index] === '[') {
			index += 1;
			const arrayTable = tokens[index] === '[';
			if (arrayTable) index += 1;
			table = key();
			index += arrayTable ? 2 : 1;
		} else {
			const path = [...table, ...key()];
			index += 1;
			value(path);
		}
	}
	return declarations;
}

function tomlPolicyKey(token) {
	if (token.startsWith("'")) return token.slice(1, -1);
	if (!token.startsWith('"')) return token;
	const escapes = { b: '\b', t: '\t', n: '\n', f: '\f', r: '\r', e: '\u001b' };
	return token.slice(1, -1).replace(/\\(u[\da-f]{4}|U[\da-f]{8}|.)/giu, (_, escaped) => {
		if (escaped.startsWith('u') || escaped.startsWith('U'))
			return String.fromCodePoint(Number.parseInt(escaped.slice(1), 16));
		return escapes[escaped] ?? escaped;
	});
}

function tomlPolicyTokens(source) {
	const tokens = [];
	let index = 0;
	while (index < source.length) {
		const character = source[index];
		if (character === '#') {
			const end = source.indexOf('\n', index);
			index = end === -1 ? source.length : end;
		} else if (character === '\n') {
			tokens.push(character);
			index += 1;
		} else if (/\s/u.test(character)) index += 1;
		else if (character === '"' || character === "'") {
			const begin = index;
			const multiline = source.startsWith(character.repeat(3), index);
			index += multiline ? 3 : 1;
			while (index < source.length) {
				if (character === '"' && source[index] === '\\') index += 2;
				else if (source.startsWith(character.repeat(multiline ? 3 : 1), index)) {
					index += multiline ? 3 : 1;
					if (multiline) while (source[index] === character) index += 1;
					break;
				} else index += 1;
			}
			tokens.push(source.slice(begin, index));
		} else if ('[].{}=,'.includes(character)) {
			tokens.push(character);
			index += 1;
		} else {
			const begin = index++;
			while (index < source.length && !/[\s#'"[\].{}=,]/u.test(source[index])) index += 1;
			tokens.push(source.slice(begin, index));
		}
	}
	return tokens;
}

function enforceAuthoredUnsafePolicy(workspacePackages) {
	assertTomlEntries(
		readFileSync(resolve(repositoryRoot, 'Cargo.toml'), 'utf8'),
		'workspace.lints.rust',
		['unsafe_code = "forbid"'],
		'root workspace Rust lint policy',
	);
	assertTomlEntries(
		readFileSync(resolve(repositoryRoot, 'Cargo.toml'), 'utf8'),
		'workspace.lints.clippy',
		['all = { level = "deny", priority = -1 }', 'wildcard_imports = "deny"'],
		'root workspace Clippy policy',
	);

	const canonicalManifest = resolve(repositoryRoot, windowsConsoleHost.manifest);
	const canonicalExists = existsSync(canonicalManifest);
	const namedExceptions = workspacePackages.filter(
		(entry) => entry.name === windowsConsoleHost.name,
	);
	const pathExceptions = workspacePackages.filter((entry) =>
		samePath(entry.manifest_path, canonicalManifest),
	);
	if (namedExceptions.length > 1 || pathExceptions.length > 1) {
		throw new Error('The Windows console unsafe-code exception must identify one exact package');
	}
	if (
		(namedExceptions.length === 1 &&
			!samePath(namedExceptions[0].manifest_path, canonicalManifest)) ||
		(pathExceptions.length === 1 && pathExceptions[0].name !== windowsConsoleHost.name) ||
		(canonicalExists && (namedExceptions.length !== 1 || pathExceptions.length !== 1))
	) {
		throw new Error(
			`The only unsafe-code exception is ${windowsConsoleHost.name} at ${windowsConsoleHost.manifest}`,
		);
	}

	for (const entry of workspacePackages) {
		if (
			entry.name === windowsConsoleHost.name &&
			samePath(entry.manifest_path, canonicalManifest)
		) {
			enforceWindowsConsoleHostPolicy(entry);
			continue;
		}
		const manifest = readFileSync(entry.manifest_path, 'utf8');
		assertTomlEntries(
			manifest,
			'lints',
			['workspace = true'],
			`${relativeManifest(entry.manifest_path)} workspace lint inheritance`,
		);
		for (const source of rustSourceFiles(dirname(entry.manifest_path))) {
			const text = readFileSync(source, 'utf8');
			const escapes = unsafeEscapeAttributes(text);
			if (escapes.length > 0) {
				throw new Error(
					`${relativeSource(source)} cannot weaken the workspace forbid for unsafe_code`,
				);
			}
			if (authoredUnsafePattern().test(rustPolicySource(text))) {
				throw new Error(
					`${relativeSource(source)} contains authored unsafe Rust outside the sole ${windowsConsoleHost.system} exception`,
				);
			}
		}
	}
}

function enforceWindowsConsoleHostPolicy(entry) {
	const manifest = readFileSync(entry.manifest_path, 'utf8');
	assertTomlEntries(
		manifest,
		'lints.rust',
		['unsafe_code = "deny"'],
		`${windowsConsoleHost.manifest} Rust lint policy`,
	);
	assertTomlEntries(
		manifest,
		'lints.clippy',
		['all = { level = "deny", priority = -1 }', 'wildcard_imports = "deny"'],
		`${windowsConsoleHost.manifest} mirrored Clippy policy`,
	);
	if (tomlEntries(manifest, 'lints').length > 0) {
		throw new Error(`${windowsConsoleHost.manifest} must not inherit the workspace unsafe forbid`);
	}

	const libraryPath = resolve(repositoryRoot, windowsConsoleHost.library);
	const systemPath = resolve(repositoryRoot, windowsConsoleHost.system);
	const testsPath = resolve(repositoryRoot, windowsConsoleHost.systemTests);
	for (const path of [libraryPath, systemPath, testsPath]) {
		if (!existsSync(path))
			throw new Error(`Missing canonical unsafe-boundary file: ${relativeSource(path)}`);
	}
	const library = readFileSync(libraryPath, 'utf8');
	const libraryPolicy = rustPolicySource(library);
	if (!/#!\[\s*deny\s*\(\s*unsafe_code\s*\)\s*\]/u.test(libraryPolicy)) {
		throw new Error(`${windowsConsoleHost.library} must keep crate-wide deny(unsafe_code)`);
	}
	const canonicalDeclaration = `#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "${windowsConsoleUnsafeReason}"
)]
mod sys;`;
	const canonicalDeclarationShape =
		/^[\t ]*#\[\s*cfg\s*\(\s*windows\s*\)\s*\][\t ]*\r?\n[\t ]*#\[\s*allow\s*\(\s*unsafe_code\s*,\s*reason\s*=\s*\)\s*\][\t ]*\r?\n[\t ]*mod[\t ]+sys[\t ]*;[\t ]*$/mu.exec(
			libraryPolicy,
		);
	const activeDeclaration =
		canonicalDeclarationShape === null
			? undefined
			: library.slice(
					canonicalDeclarationShape.index,
					canonicalDeclarationShape.index + canonicalDeclarationShape[0].length,
				);
	if (
		activeDeclaration === undefined ||
		normalizedWhitespace(activeDeclaration) !== normalizedWhitespace(canonicalDeclaration)
	) {
		throw new Error(
			`${windowsConsoleHost.library} must keep the reasoned unsafe_code allow on private mod sys`,
		);
	}
	const system = readFileSync(systemPath, 'utf8');
	const systemPolicy = rustPolicySource(system);
	const safeTestDeclaration = `#[cfg(test)]
#[deny(unsafe_code)]
mod tests;`;
	const safeTestDeclarationShape =
		/^[\t ]*#\[\s*cfg\s*\(\s*test\s*\)\s*\][\t ]*\r?\n[\t ]*#\[\s*deny\s*\(\s*unsafe_code\s*\)\s*\][\t ]*\r?\n[\t ]*mod[\t ]+tests[\t ]*;[\t ]*$/mu.exec(
			systemPolicy,
		);
	const activeTestDeclaration =
		safeTestDeclarationShape === null
			? undefined
			: system.slice(
					safeTestDeclarationShape.index,
					safeTestDeclarationShape.index + safeTestDeclarationShape[0].length,
				);
	if (
		activeTestDeclaration === undefined ||
		normalizedWhitespace(activeTestDeclaration) !== normalizedWhitespace(safeTestDeclaration)
	) {
		throw new Error(
			`${windowsConsoleHost.system} must attach its adjacent tests with deny(unsafe_code)`,
		);
	}
	const systemDirectory = dirname(testsPath);
	const unexpectedSystemChildren = readdirSync(systemDirectory, { withFileTypes: true })
		.filter((child) => !(child.isFile() && child.name === 'tests.rs'))
		.map((child) => `${windowsConsoleHost.system.replace(/\.rs$/u, '')}/${child.name}`);
	if (unexpectedSystemChildren.length > 0) {
		throw new Error(
			`Only the denied safe test child may live below src/sys:\n${unexpectedSystemChildren.join('\n')}`,
		);
	}

	const sources = rustSourceFiles(dirname(entry.manifest_path));
	const escapes = sources.flatMap((source) =>
		unsafeEscapeAttributes(readFileSync(source, 'utf8')).map(() => source),
	);
	if (escapes.length !== 1 || !samePath(escapes[0], libraryPath)) {
		throw new Error(
			`Only ${windowsConsoleHost.library} may contain the canonical unsafe_code allow`,
		);
	}
	for (const source of sources) {
		if (samePath(source, systemPath)) continue;
		const text = readFileSync(source, 'utf8');
		if (authoredUnsafePattern().test(rustPolicySource(text))) {
			throw new Error(
				`${relativeSource(source)} contains unsafe Rust outside ${windowsConsoleHost.system}`,
			);
		}
	}
}

function assertTomlEntries(manifest, section, expected, label) {
	const actual = tomlEntries(manifest, section);
	if (actual.length !== expected.length || actual.some((line, index) => line !== expected[index])) {
		throw new Error(`${label} must be exactly:\n${expected.join('\n')}`);
	}
}

function tomlEntries(manifest, section) {
	const escaped = section.replaceAll('.', '\\.');
	const match = new RegExp(`^\\[${escaped}\\]\\s*$`, 'mu').exec(manifest);
	if (match === null) return [];
	const rest = manifest.slice(match.index + match[0].length);
	const nextSection = /^\s*\[/mu.exec(rest);
	const body = nextSection === null ? rest : rest.slice(0, nextSection.index);
	return body
		.split(/\r?\n/u)
		.map((line) => line.replace(/\s+#.*$/u, '').trim())
		.filter((line) => line.length > 0);
}

function rustSourceFiles(directory) {
	if (!existsSync(directory)) return [];
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		if (entry.name === 'target' || entry.name === '.git') return [];
		const path = join(directory, entry.name);
		if (entry.isDirectory()) return rustSourceFiles(path);
		return entry.isFile() && entry.name.endsWith('.rs') ? [path] : [];
	});
}

function unsafeEscapeAttributes(source) {
	const policy = rustPolicySource(source);
	return [
		...policy.matchAll(
			/#\s*!?\s*\[(?=[^\]]*\b(?:allow|expect)\s*\()(?=[^\]]*\bunsafe_code\b)[^\]]*\]/gu,
		),
	];
}

function authoredUnsafePattern() {
	return /#\s*!?\s*\[\s*unsafe\s*\(|\bunsafe\s+(?:(?:async|const)\s+)*(?:extern|fn|impl|trait)\b|\bunsafe\s*\{/u;
}

function normalizedWhitespace(source) {
	return source.replace(/\s+/gu, ' ').trim();
}

function rustPolicySource(source) {
	const policy = source.split('');
	const mask = (start, end) => {
		for (let offset = start; offset < end; offset += 1) {
			if (policy[offset] !== '\n' && policy[offset] !== '\r') policy[offset] = ' ';
		}
	};
	let index = 0;
	while (index < source.length) {
		if (source.startsWith('//', index)) {
			const end = source.indexOf('\n', index + 2);
			const next = end === -1 ? source.length : end;
			mask(index, next);
			index = next;
			continue;
		}
		if (source.startsWith('/*', index)) {
			let depth = 1;
			let end = index + 2;
			while (end < source.length && depth > 0) {
				if (source.startsWith('/*', end)) {
					depth += 1;
					end += 2;
				} else if (source.startsWith('*/', end)) {
					depth -= 1;
					end += 2;
				} else end += 1;
			}
			mask(index, end);
			index = end;
			continue;
		}
		const rawString = /^(?:b|c)?r(#{0,255})"/u.exec(source.slice(index));
		if (rawString !== null) {
			const delimiter = `"${rawString[1]}`;
			const contentStart = index + rawString[0].length;
			const closing = source.indexOf(delimiter, contentStart);
			const end = closing === -1 ? source.length : closing + delimiter.length;
			mask(index, end);
			index = end;
			continue;
		}
		if (source[index] === '"') {
			let end = index + 1;
			while (end < source.length) {
				if (source[end] === '\\') end += 2;
				else if (source[end++] === '"') break;
			}
			mask(index, Math.min(end, source.length));
			index = end;
			continue;
		}
		const character = /^(?:b)?'(?:\\.|[^\\'\r\n])'/u.exec(source.slice(index));
		if (character !== null) {
			mask(index, index + character[0].length);
			index += character[0].length;
			continue;
		}
		index += 1;
	}
	return policy.join('');
}

function samePath(left, right) {
	const normalizedLeft = resolve(left).replaceAll('\\', '/');
	const normalizedRight = resolve(right).replaceAll('\\', '/');
	return process.platform === 'win32'
		? normalizedLeft.toLowerCase() === normalizedRight.toLowerCase()
		: normalizedLeft === normalizedRight;
}

function relativeSource(path) {
	return relative(repositoryRoot, path).replaceAll('\\', '/');
}

function relativeManifest(path) {
	return relativeSource(path);
}
