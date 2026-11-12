// Keeps the Orchbee product version in one place per ecosystem and checks every recorded version
// against a release tag.
//
//   node scripts/release-version.mjs set <version>   <version>: 0.1.0 or 0.1.0-beta.1
//   node scripts/release-version.mjs check <tag>     <tag>: v<version>
//
// The root Cargo.toml records the version under [workspace.package], and the Rust programs inherit
// it with `version.workspace = true`. The Desktop package, the Terminal Host default version and the
// Rust entries in Cargo.lock repeat the value, so `set` writes those places, and `check` also confirms
// that each Rust program still inherits the workspace version.
//
// Exit codes: 0 written, or every place agrees; 1 a place disagrees or cannot be found; 2 malformed
// arguments.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPOSITORY_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

// SemVer 2.0.0 without build metadata.
const SEMVER =
	/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:0|[1-9]\d*|\d*[a-zA-Z-][\da-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][\da-zA-Z-]*))*)?$/u;

const CARGO_MANIFEST = 'Cargo.toml';
const CARGO_LOCK = 'Cargo.lock';
const DESKTOP_PACKAGE = 'apps/desktop/package.json';
const HOST_BUILD = 'apps/terminal-host/build.zig';

// The Rust programs that inherit the product version, with the name each has in Cargo.lock.
const RUST_PROGRAMS = [
	{ manifest: 'apps/orchbee/Cargo.toml', name: 'orchbee' },
	{ manifest: 'apps/orchbee-server/Cargo.toml', name: 'orchbee-server' },
	{ manifest: 'apps/orchbee-worker/Cargo.toml', name: 'orchbee-worker' },
	{ manifest: 'crates/local-runtime/Cargo.toml', name: 'orchbee-local-runtime' },
	{ manifest: 'apps/orchbee/crates/workbench-client/Cargo.toml', name: 'orchbee-workbench-client' },
];

// Every place that records the version. `find` returns a span for the recorded value, or no span.
const RECORDED_VERSIONS = [
	{ path: CARGO_MANIFEST, label: '', find: (text) => findTomlVersion(text, 'workspace.package') },
	{ path: DESKTOP_PACKAGE, label: '', find: findJsonVersion },
	{ path: HOST_BUILD, label: '', find: findZigHostVersion },
	...RUST_PROGRAMS.map(({ name }) => ({
		path: CARGO_LOCK,
		label: `${name} `,
		find: (text) => findLockVersion(text, name),
	})),
];

const SET_FORM =
	'expected: node scripts/release-version.mjs set <version>, for example 0.1.0 or 0.1.0-beta.1';
const CHECK_FORM =
	'expected: node scripts/release-version.mjs check v<version>, for example v0.1.0';
const USAGE = 'expected: node scripts/release-version.mjs set <version> or check v<version>';

// Writes `version` into every recorded place. Nothing is written unless every place is found.
export function setVersion(root, version) {
	const sources = new Map();
	const spansByPath = new Map();
	for (const { path, label, find } of RECORDED_VERSIONS) {
		const spans = find(readSource(root, path, sources));
		if (spans.length === 0) return { status: 1, lines: [`${path}: ${label}not found`] };
		spansByPath.set(path, [...(spansByPath.get(path) ?? []), ...spans]);
	}
	const written = [];
	for (const [path, spans] of spansByPath) {
		const original = sources.get(path);
		let text = original;
		// From the end of the file backwards, so that the earlier spans keep their offsets.
		for (const { start, end } of spans.toSorted((a, b) => b.start - a.start)) {
			text = `${text.slice(0, start)}${version}${text.slice(end)}`;
		}
		if (text !== original) {
			writeFileSync(join(root, path), text);
			written.push(path);
		}
	}
	return {
		status: 0,
		lines: [`Set the product version to ${version} in ${written.length} file(s).`],
	};
}

// One line for each recorded place that does not hold `version`, and for each Rust program that does
// not inherit the workspace version. No lines means every place agrees.
export function checkVersion(root, version) {
	const sources = new Map();
	const lines = [];
	for (const { path, label, find } of RECORDED_VERSIONS) {
		const [found] = find(readSource(root, path, sources));
		if (found === undefined) lines.push(`${path}: ${label}not found (expected ${version})`);
		else if (found.value !== version)
			lines.push(`${path}: ${label}${found.value} (expected ${version})`);
	}
	for (const { manifest } of RUST_PROGRAMS) {
		const text = readSource(root, manifest, sources);
		if (!inheritsWorkspaceVersion(text)) {
			const [own] = findTomlVersion(text, 'package');
			const found = own === undefined ? 'no version' : `own version "${own.value}"`;
			lines.push(`${manifest}: ${found} (expected version.workspace = true)`);
		}
	}
	return { status: lines.length === 0 ? 0 : 1, lines };
}

// Runs one command line. Returns the exit status and the lines to print.
export function runReleaseVersion(argv, root = REPOSITORY_ROOT) {
	const [command, argument] = argv;
	if (command === 'set' && argv.length === 2) {
		return SEMVER.test(argument) ? setVersion(root, argument) : { status: 2, lines: [SET_FORM] };
	}
	if (command === 'check' && argv.length === 2) {
		const tag = /^v(.*)$/u.exec(argument);
		return tag !== null && SEMVER.test(tag[1])
			? checkVersion(root, tag[1])
			: { status: 2, lines: [CHECK_FORM] };
	}
	let form = USAGE;
	if (command === 'set') form = SET_FORM;
	if (command === 'check') form = CHECK_FORM;
	return { status: 2, lines: [form] };
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	const { status, lines } = runReleaseVersion(process.argv.slice(2));
	const stream = status === 0 ? process.stdout : process.stderr;
	for (const line of lines) stream.write(`${line}\n`);
	process.exitCode = status;
}

// Reads a repository file once per run; `sources` caches the text by path.
function readSource(root, path, sources) {
	if (!sources.has(path)) sources.set(path, readFileSync(join(root, path), 'utf8'));
	return sources.get(path);
}

// The lines of `text`, each with the offset where it starts. A CRLF line keeps its carriage return.
function lineRows(text) {
	const rows = [];
	let start = 0;
	for (const line of text.split('\n')) {
		rows.push({ text: line, start });
		start += line.length + 1;
	}
	return rows;
}

// The lines under the `[table]` header, up to the next table header.
function tableRows(text, table) {
	const rows = lineRows(text);
	const header = rows.findIndex((row) => row.text.trim() === `[${table}]`);
	if (header === -1) return [];
	const body = [];
	for (const row of rows.slice(header + 1)) {
		if (row.text.trimStart().startsWith('[')) break;
		body.push(row);
	}
	return body;
}

function valueSpan(start, value) {
	return { start, end: start + value.length, value };
}

// The quoted `version` of a TOML table.
function findTomlVersion(text, table) {
	for (const row of tableRows(text, table)) {
		const match = /^(\s*version\s*=\s*")([^"]*)"/u.exec(row.text);
		if (match !== null) return [valueSpan(row.start + match[1].length, match[2])];
	}
	return [];
}

function inheritsWorkspaceVersion(text) {
	return tableRows(text, 'package').some((row) =>
		/^\s*version\.workspace\s*=\s*true\s*(?:#.*)?$/u.test(row.text),
	);
}

// The quoted `version` of the Cargo.lock entry for the package `name`.
function findLockVersion(text, name) {
	const entries = [];
	for (const row of lineRows(text)) {
		if (row.text.trim() === '[[package]]') entries.push([]);
		else if (entries.length > 0) entries.at(-1).push(row);
	}
	const entry = entries.find((rows) => rows.some((row) => row.text.trim() === `name = "${name}"`));
	if (entry === undefined) return [];
	for (const row of entry) {
		const match = /^(version = ")([^"]*)"/u.exec(row.text);
		if (match !== null) return [valueSpan(row.start + match[1].length, match[2])];
	}
	return [];
}

// The quoted value of `const default_host_version = "...";` in build.zig.
function findZigHostVersion(text) {
	const match = /^(const default_host_version = ")([^"\r\n]*)"/mu.exec(text);
	return match === null ? [] : [valueSpan(match.index + match[1].length, match[2])];
}

// The string value of the top-level `"version"` member of a JSON object.
function findJsonVersion(text) {
	let depth = 0;
	let index = 0;
	while (index < text.length) {
		const character = text[index];
		if (character === '"') {
			const end = jsonStringEnd(text, index);
			const colon = /\s*:\s*/uy;
			colon.lastIndex = end;
			if (
				depth === 1 &&
				text.slice(index, end) === '"version"' &&
				colon.test(text) &&
				text[colon.lastIndex] === '"'
			) {
				const valueStart = colon.lastIndex + 1;
				const valueEnd = jsonStringEnd(text, colon.lastIndex);
				return [valueSpan(valueStart, text.slice(valueStart, valueEnd - 1))];
			}
			index = end;
		} else {
			if (character === '{' || character === '[') depth += 1;
			if (character === '}' || character === ']') depth -= 1;
			index += 1;
		}
	}
	return [];
}

// The index just past the closing quote of the JSON string that opens at `start`.
function jsonStringEnd(text, start) {
	let index = start + 1;
	while (index < text.length && text[index] !== '"') index += text[index] === '\\' ? 2 : 1;
	if (index >= text.length) throw new Error('unterminated JSON string');
	return index + 1;
}
