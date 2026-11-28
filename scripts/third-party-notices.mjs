/**
 * The third-party notices every distribution carries: a cohort's native programs, or the Desktop
 * application around them. Each `THIRD_PARTY_NOTICES.txt` starts with Orchbee's license and
 * reproduces the notices and license texts of what that distribution contains, for its one
 * platform.
 *
 * What a distribution contains comes from where it is decided: the crates Cargo resolves for the
 * programs and their target, the npm packages pnpm installs as the window's production
 * dependencies, the toolchain runtimes the programs link, the NOTICE files of the workspace
 * sources they are built from, and the sections of THIRD_PARTY_NOTICES.md whose `Carried by` names
 * the distribution. A component without a license text stops packaging rather than being left
 * out; scripts/third-party-license-texts.txt supplies the text a package does not carry itself.
 */
import { execFileSync } from 'node:child_process';
import {
	constants,
	copyFileSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	statSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/** The notices file at the root of every cohort and of every Desktop application. */
export const NOTICES_FILE = 'THIRD_PARTY_NOTICES.txt';
/** The Rust toolchain's notices of its standard library, beside a cohort's notices file. */
export const RUST_LIBRARY_NOTICES = 'licenses/rust-standard-library.html';

const INVENTORY = 'THIRD_PARTY_NOTICES.md';
const LICENSE_TEXTS = 'scripts/third-party-license-texts.txt';
const PLATFORMS = new Map([
	['Windows', 'windows'],
	['macOS', 'macos'],
	['Linux', 'linux'],
]);
const PLATFORM_NAMES = new Map([...PLATFORMS].map(([name, token]) => [token, name]));
// One `Carried by` value: a distribution, optionally narrowed to one platform and to the
// development and test builds, which alone carry the File Bar's process and rg.
const CARRIER =
	/^(native programs|Desktop application|website)(?: on (Windows|macOS|Linux))?( in development and test builds)?$/u;
// The file names a package keeps its license, notice and copyright texts under.
const LICENSE_FILE =
	/^(?:licen[cs]e|copying|copyright|notice|unlicense)|third[-_.]?party[-_.]?notices/iu;
const WORKSPACE_NOTICE = /^NOTICE(?:\.[A-Za-z0-9]+)?$/u;
const SKIPPED_DIRECTORIES = new Set(['node_modules', 'target', 'dist', 'out', 'zig-out']);
const WIDTH = 100;
const HEAVY = '='.repeat(80);
const LIGHT = '-'.repeat(80);

/** One error naming every missing text, so a single run shows all that packaging needs. */
function incomplete(problems) {
	return new Error(
		'The third-party notices are incomplete, so packaging stops rather than leave them out:\n' +
			problems.map((problem) => `  - ${problem}`).join('\n'),
	);
}

/** The text as notices reproduce it: LF line ends, no trailing spaces, no blank edges. */
function normalized(text) {
	const lines = text
		.replace(/^﻿/u, '')
		.replaceAll('\r\n', '\n')
		.replaceAll('\r', '\n')
		.split('\n')
		.map((line) => line.trimEnd());
	while (lines[0] === '') lines.shift();
	while (lines.at(-1) === '') lines.pop();
	return lines.join('\n');
}

/** Markdown inline syntax as plain text; a link outside the repository keeps its address. */
function plain(text) {
	return text
		.replaceAll(/\[([^\]]*)\]\((https?:[^)]+)\)/gu, '$1 ($2)')
		.replaceAll(/\[([^\]]*)\]\([^)]+\)/gu, '$1')
		.replaceAll(/<(https?:[^>]+)>/gu, '$1')
		.replaceAll('`', '');
}

/** Wraps one logical line at the notices' width, indenting its continuation lines. */
function wrap(text, indent = '  ') {
	const lines = [];
	let line = '';
	for (const word of text.split(/\s+/u).filter(Boolean)) {
		if (line !== '' && line.length + 1 + word.length > WIDTH) {
			lines.push(line);
			line = indent + word;
		} else line = line === '' ? word : `${line} ${word}`;
	}
	if (line !== '') lines.push(line);
	return lines;
}

function compare(left, right) {
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

/**
 * The sections of THIRD_PARTY_NOTICES.md: each one's name, the fields and paragraphs that describe
 * it, the distributions that carry it, and its license texts, quoted or linked. Throws, naming
 * every problem, when a distributed section lacks either.
 */
export function readInventory(root = repositoryRoot) {
	const sections = [];
	let section;
	const source = readFileSync(path.join(root, INVENTORY), 'utf8').replaceAll('\r\n', '\n');
	for (const line of source.split('\n')) {
		if (line.startsWith('## ')) {
			section = { name: line.slice(3).trim(), lines: [] };
			sections.push(section);
		} else section?.lines.push(line);
	}
	const problems = [];
	const parsed = sections.map((entry) => parseSection(root, entry, problems));
	if (problems.length > 0) throw incomplete(problems);
	return parsed;
}

function parseSection(root, { name, lines }, problems) {
	const fields = [];
	const quotes = [];
	const body = [];
	let previous = 'blank';
	for (const line of lines) {
		if (line.startsWith('>')) {
			if (previous !== 'quote') quotes.push([]);
			quotes.at(-1).push(line.replace(/^> ?/u, ''));
			previous = 'quote';
			continue;
		}
		const field = /^- ([^:]+): (.*)$/u.exec(line);
		if (field !== null) {
			fields.push({ key: field[1], value: field[2] });
			previous = 'field';
		} else if (previous === 'field' && /^ {2}\S/u.test(line)) {
			fields.at(-1).value += ` ${line.trim()}`;
		} else if (line.trim() === '') {
			if (previous !== 'blank' && previous !== 'quote') body.push('');
			previous = 'blank';
		} else {
			body.push(line);
			previous = 'body';
		}
	}
	const where = `${INVENTORY} section "${name}"`;
	const carriedBy = fields.find((field) => field.key === 'Carried by');
	const carriers = [];
	if (carriedBy === undefined) problems.push(`${where} has no \`Carried by\` item`);
	for (const value of carriedBy?.value.split(',') ?? []) {
		const match = CARRIER.exec(value.trim());
		if (match === null) {
			problems.push(
				`${where} is carried by "${value.trim()}"; write \`native programs\`, \`Desktop ` +
					'application` or `website`, optionally followed by ` on Windows`, ` on macOS` or ` on ' +
					'Linux` and by ` in development and test builds`',
			);
			continue;
		}
		carriers.push({
			distribution: { 'native programs': 'programs', 'Desktop application': 'window' }[match[1]],
			platform: PLATFORMS.get(match[2]),
			development: match[3] !== undefined,
		});
	}
	const licenseText = fields.find((field) => field.key.startsWith('License text'));
	const files = [];
	let none;
	// What only the website serves travels in no distribution, so packaging reads no text for it.
	if (carriers.some((carrier) => carrier.distribution !== undefined)) {
		if (licenseText?.value.startsWith('none; ')) none = licenseText.value.slice('none; '.length);
		else {
			for (const [, target] of licenseText?.value.matchAll(/\]\(([^)]+)\)/gu) ?? []) {
				const file = path.join(root, ...target.split('/'));
				if (/^[a-z]+:/u.test(target) || !existsSync(file) || !statSync(file).isFile())
					problems.push(`${where} links ${target} as a license text, which is no repository file`);
				else files.push(file);
			}
			if (licenseText !== undefined && files.length === 0)
				problems.push(
					`${where} links no file under \`${licenseText.key}\` and does not read \`none; <reason>\``,
				);
			else if (files.length === 0 && quotes.length === 0)
				problems.push(
					`${where} has no license text: quote it, link its file under \`License text\`, or write ` +
						'`License text: none; <reason>`',
				);
		}
	}
	while (body.at(-1) === '') body.pop();
	return {
		name,
		fields: fields.filter((field) => field !== carriedBy && field !== licenseText),
		body,
		carriers,
		none,
		texts: [
			...quotes.map((quote) => normalized(quote.join('\n'))),
			...files.map((file) => normalized(readFileSync(file, 'utf8'))),
		],
		files,
	};
}

function carries(section, distribution, platform, development) {
	return section.carriers.some(
		(carrier) =>
			carrier.distribution === distribution &&
			(carrier.platform === undefined || carrier.platform === platform) &&
			(!carrier.development || development),
	);
}

/**
 * The license texts of scripts/third-party-license-texts.txt, by `crate:<name>` or `npm:<name>`,
 * each with the license its package must declare for the text to apply.
 */
export function readLicenseTexts(root = repositoryRoot) {
	const texts = new Map();
	let current;
	const source = readFileSync(path.join(root, LICENSE_TEXTS), 'utf8').replaceAll('\r\n', '\n');
	for (const line of source.split('\n')) {
		const header = /^=== (crate|npm) (.+?) \| (.+)$/u.exec(line);
		if (header !== null) {
			current = { license: header[3].trim(), lines: [] };
			for (const name of header[2].split(',')) texts.set(`${header[1]}:${name.trim()}`, current);
		} else if (line.startsWith('===')) {
			throw new Error(`${LICENSE_TEXTS}: "${line}" is not "=== <crate|npm> <names> | <license>"`);
		} else current?.lines.push(line);
	}
	for (const [key, entry] of texts) {
		entry.text = normalized(entry.lines.join('\n'));
		if (entry.text === '') throw new Error(`${LICENSE_TEXTS}: ${key} has no text`);
	}
	return texts;
}

/** The license, notice and copyright files a package keeps at its root, in name order. */
function licenseFiles(directory, declared) {
	const found = [];
	for (const entry of readdirSync(directory, { withFileTypes: true }).toSorted((left, right) =>
		compare(left.name, right.name),
	)) {
		if (!LICENSE_FILE.test(entry.name)) continue;
		const file = path.join(directory, entry.name);
		if (entry.isFile()) found.push(file);
		else if (entry.isDirectory())
			for (const inner of readdirSync(file).toSorted(compare)) {
				const innerFile = path.join(file, inner);
				if (statSync(innerFile).isFile()) found.push(innerFile);
			}
	}
	if (declared !== undefined) {
		const file = path.resolve(directory, declared);
		if (existsSync(file) && !found.includes(file)) found.push(file);
	}
	return found;
}

/**
 * Each package with its license texts: its own files, else the text the repository supplies for a
 * package of its name that declares the same license. A package with neither is a problem.
 */
function packageTexts(packages, ecosystem, supplied, problems) {
	return packages.map((entry) => {
		const files = licenseFiles(entry.directory, entry.licenseFile);
		if (files.length > 0)
			return {
				...entry,
				texts: files.map((file) => ({
					origin: path.relative(entry.directory, file).split(path.sep).join('/'),
					text: normalized(readFileSync(file, 'utf8')),
				})),
			};
		const text = supplied.get(`${ecosystem}:${entry.name}`);
		if (text === undefined || text.license !== entry.license) {
			problems.push(
				`${ecosystem} ${entry.name} ${entry.version} (${entry.license ?? 'no declared license'}) ` +
					`carries no license text: add its upstream text to ${LICENSE_TEXTS} under ` +
					`"=== ${ecosystem} ${entry.name} | ${entry.license ?? '<license>'}"` +
					(text === undefined ? '' : `, since the entry there names ${text.license}`),
			);
			return { ...entry, texts: [] };
		}
		return { ...entry, texts: [{ origin: `text supplied in ${LICENSE_TEXTS}`, text: text.text }] };
	});
}

/**
 * The third-party crates in the given workspace packages for the target: every normal dependency,
 * followed through the workspace's own crates. A build dependency, a build script's dependencies
 * and a procedural macro run only on the building host and leave none of their code in a program,
 * so they and what they alone depend on stay out.
 */
function cargoPackages(root, target, roots) {
	const metadata = JSON.parse(
		execFileSync(
			'cargo',
			['metadata', '--format-version', '1', '--locked', '--filter-platform', target],
			{
				cwd: root,
				encoding: 'utf8',
				maxBuffer: 1 << 29,
				stdio: ['ignore', 'pipe', 'inherit'],
				windowsHide: true,
			},
		),
	);
	const packages = new Map(metadata.packages.map((entry) => [entry.id, entry]));
	const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
	const members = new Set(metadata.workspace_members);
	const pending = [];
	for (const name of roots) {
		const member = metadata.packages.find((entry) => members.has(entry.id) && entry.name === name);
		if (member === undefined) throw new Error(`No workspace package is named ${name}`);
		pending.push(member.id);
	}
	const reached = new Set();
	while (pending.length > 0) {
		const id = pending.pop();
		if (reached.has(id)) continue;
		reached.add(id);
		for (const dependency of nodes.get(id)?.deps ?? []) {
			if (!dependency.dep_kinds.some((kind) => kind.kind === null)) continue;
			const entry = packages.get(dependency.pkg);
			if (entry.targets.some((crate) => crate.kind.includes('proc-macro'))) continue;
			pending.push(entry.id);
		}
	}
	const third = [];
	const workspace = [];
	for (const id of reached) {
		const entry = packages.get(id);
		const directory = path.dirname(entry.manifest_path);
		if (members.has(id)) {
			workspace.push(directory);
			continue;
		}
		third.push({
			name: entry.name,
			version: entry.version,
			license: entry.license ?? undefined,
			licenseFile: entry.license_file ?? undefined,
			source:
				sourceUrl(entry.repository ?? entry.homepage ?? `https://crates.io/crates/${entry.name}`) +
				// A crate the workspace patches with its own copy names that copy.
				(entry.source === null
					? `, as modified in ${path.relative(root, directory).split(path.sep).join('/')}`
					: ''),
			directory,
		});
	}
	return { packages: third.toSorted(byPackage), workspace };
}

function byPackage(left, right) {
	return compare(left.name, right.name) || compare(left.version, right.version);
}

/** Runs pnpm through the entry `pnpm run` supplies, else the `pnpm` on PATH. */
function pnpm(root, args) {
	const cli = process.env.npm_execpath;
	const viaNode = cli !== undefined && /^pnpm\.(?:c?js|mjs)$/u.test(path.basename(cli));
	return execFileSync(viaNode ? process.execPath : 'pnpm', viaNode ? [cli, ...args] : args, {
		cwd: root,
		encoding: 'utf8',
		maxBuffer: 1 << 28,
		// pnpm is a command script on Windows, which only a shell starts; no argument needs quoting.
		shell: !viaNode && process.platform === 'win32',
		stdio: ['ignore', 'pipe', 'inherit'],
		windowsHide: true,
	});
}

/** A declared repository as a web address: Git's transports, GitHub shorthand and `.git` go. */
function sourceUrl(value) {
	return value
		.trim()
		.replace(/^git\+/u, '')
		.replace(/^(?:git|ssh):\/\/(?:git@)?/u, 'https://')
		.replace(/^git@([^:/]+):/u, 'https://$1/')
		.replace(/^github:/u, 'https://github.com/')
		.replace(/^([\w.-]+\/[\w.-]+)$/u, 'https://github.com/$1')
		.replace(/#.*$/u, '')
		.replace(/\/+$/u, '')
		.replace(/\.git$/u, '');
}

function repositoryUrl(manifest) {
	const value =
		typeof manifest.repository === 'string' ? manifest.repository : manifest.repository?.url;
	if (typeof value === 'string') return sourceUrl(value);
	return typeof manifest.homepage === 'string'
		? manifest.homepage
		: `https://www.npmjs.com/package/${manifest.name}`;
}

function declaredLicense(manifest) {
	if (typeof manifest.license === 'string') return manifest.license;
	if (typeof manifest.license?.type === 'string') return manifest.license.type;
	if (Array.isArray(manifest.licenses))
		return manifest.licenses.map((entry) => entry?.type ?? entry).join(' OR ');
	return undefined;
}

/**
 * The npm packages of the workspace packages' production dependencies, which hold everything the
 * window's bundles contain and more: a superset, because a notice missing from a bundle could not
 * be told apart from an unused package. The `electron` package's own dependencies only download
 * Electron while installing, so they stay out; the packaged Electron carries its `LICENSE` and
 * `LICENSES.chromium.html`.
 */
function npmPackages(root, filters) {
	const projects = JSON.parse(
		pnpm(root, [
			'list',
			'--prod',
			'--json',
			'--depth',
			'Infinity',
			...filters.flatMap((filter) => ['--filter', filter]),
		]),
	);
	// pnpm lists a package's dependencies once and marks its later appearances deduplicated, so
	// the graph joins every appearance by path.
	const children = new Map();
	const versions = new Map();
	const record = (node) => {
		versions.set(node.path, node.version);
		const dependencies = Object.values(node.dependencies ?? {});
		if (dependencies.length === 0) return;
		const known = children.get(node.path) ?? new Set();
		for (const dependency of dependencies) {
			known.add(dependency.path);
			record(dependency);
		}
		children.set(node.path, known);
	};
	for (const project of projects) record(project);
	const reached = new Set();
	const pending = projects.map((project) => project.path);
	const third = [];
	while (pending.length > 0) {
		const directory = pending.pop();
		if (reached.has(directory)) continue;
		reached.add(directory);
		const manifest = JSON.parse(readFileSync(path.join(directory, 'package.json'), 'utf8'));
		const workspace = projects.some((project) => project.path === directory);
		if (!workspace && versions.get(directory)?.startsWith('link:'))
			throw new Error(`${manifest.name} links outside the listed workspace packages`);
		if (!workspace)
			third.push({
				name: manifest.name,
				version: manifest.version,
				license: declaredLicense(manifest),
				source: repositoryUrl(manifest),
				directory,
			});
		if (manifest.name === 'electron') continue;
		pending.push(...(children.get(directory) ?? []));
	}
	return {
		packages: third.toSorted(byPackage),
		workspace: projects.map((project) => project.path),
	};
}

/** The NOTICE files of the workspace sources a distribution is built from, in path order. */
function workspaceNotices(root, directories) {
	const found = new Set();
	const visit = (directory) => {
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const file = path.join(directory, entry.name);
			if (entry.isDirectory()) {
				if (!entry.name.startsWith('.') && !SKIPPED_DIRECTORIES.has(entry.name)) visit(file);
			} else if (entry.isFile() && WORKSPACE_NOTICE.test(entry.name)) found.add(file);
		}
	};
	for (const directory of new Set(directories)) visit(directory);
	return [...found]
		.map((file) => ({ file, relative: path.relative(root, file).split(path.sep).join('/') }))
		.toSorted((left, right) => compare(left.relative, right.relative));
}

/** The Rust target of the host, which a build without `--target` produces. */
function hostTarget() {
	const host = /^host: (\S+)/mu.exec(capture('rustc', ['-vV']))?.[1];
	if (host === undefined) throw new Error('rustc -vV names no host');
	return host;
}

function capture(command, args) {
	return execFileSync(command, args, {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'inherit'],
		windowsHide: true,
	});
}

/**
 * The runtimes the programs' toolchains build into them: the Rust standard library, whose own
 * notices file travels beside the notices, and the Zig standard library and compiler runtime of
 * the Terminal Host, with the MinGW-w64 runtime it links on Windows and the musl C library the
 * Linux programs link. LLVM's runtime libraries carry the LLVM exception and need no notice.
 */
function toolchainRuntimes(platform, problems) {
	const rustLibrary = path.join(
		capture('rustc', ['--print', 'sysroot']).trim(),
		'share',
		'doc',
		'rust',
		'COPYRIGHT-library.html',
	);
	if (!existsSync(rustLibrary)) problems.push(`the Rust toolchain has no ${rustLibrary}`);
	const zig = capture('zig', ['env']);
	const zigValue = (key) => {
		const raw = new RegExp(`\\.${key} = "((?:[^"\\\\]|\\\\.)*)"`, 'u').exec(zig)?.[1];
		if (raw === undefined) throw new Error(`zig env names no ${key}`);
		return JSON.parse(`"${raw}"`);
	};
	const zigLibrary = zigValue('lib_dir');
	// Zig's archives keep its license beside `lib`; a package manager may install `lib/zig` instead.
	const zigLicense =
		[path.join(zigLibrary, '..', 'LICENSE'), path.join(zigLibrary, '..', '..', 'LICENSE')].find(
			(file) => existsSync(file),
		) ?? path.join(zigLibrary, '..', 'LICENSE');
	const runtimes = [
		{
			name: 'Zig standard library and compiler runtime',
			fields: [
				`Version: Zig ${zigValue('version')}`,
				'License: MIT',
				'Use in Orchbee: the Terminal Host is written in Zig and built with its standard library',
			],
			file: zigLicense,
		},
		...(platform === 'windows'
			? [
					{
						name: 'MinGW-w64 runtime',
						fields: [
							`Source: the copy in Zig ${zigValue('version')}`,
							'License: Zope Public License 2.1, with parts in the public domain',
							'Use in Orchbee: the C runtime start-up and support code the Terminal Host links',
						],
						file: path.join(zigLibrary, 'libc', 'mingw', 'COPYING'),
					},
				]
			: []),
		...(platform === 'linux'
			? [
					{
						name: 'musl',
						fields: [
							`Source: the copy in Zig ${zigValue('version')}; Rust's musl targets carry the same library`,
							'License: MIT',
							'Use in Orchbee: the C library that the Terminal Host, and every program of a ' +
								'Linux release, link statically',
						],
						file: path.join(zigLibrary, 'libc', 'musl', 'COPYRIGHT'),
					},
				]
			: []),
	];
	for (const runtime of runtimes)
		if (!existsSync(runtime.file)) problems.push(`the Zig toolchain has no ${runtime.file}`);
	return {
		rustLibrary,
		rustVersion: capture('rustc', ['--version']).trim(),
		runtimes,
	};
}

function heading(title) {
	return [HEAVY, title, HEAVY, ''];
}

function block(title, fields, texts) {
	const lines = [LIGHT, ...wrap(title), ...fields.flatMap((field) => wrap(field)), LIGHT, ''];
	for (const text of texts) lines.push(text, '');
	return lines;
}

function renderSection(section) {
	const fields = section.fields.map(({ key, value }) => `${key}: ${plain(value)}`);
	const paragraphs = [];
	let paragraph = [];
	for (const line of [...section.body, '']) {
		if (line === '') {
			if (paragraph.length > 0) paragraphs.push(paragraph.join(' '));
			paragraph = [];
		} else if (line.startsWith('|')) {
			// A table row reads as its cells; the rule under the header row says nothing.
			if (!/^\|[\s:|-]+\|$/u.test(line))
				paragraphs.push(
					line
						.slice(1, -1)
						.split('|')
						.map((cell) => plain(cell.trim()))
						.join(': '),
				);
		} else paragraph.push(plain(line.trim()));
	}
	if (section.none !== undefined) fields.push(`License text: none; ${plain(section.none)}`);
	const description = paragraphs.flatMap((text) => wrap(text, '').concat(''));
	const lines = block(section.name, fields, []);
	return [...lines, ...description, ...section.texts.flatMap((text) => [text, ''])];
}

function renderPackages(title, introduction, packages) {
	const lines = [...heading(title), ...wrap(introduction, ''), ''];
	for (const item of packages)
		lines.push(
			`${item.name} ${item.version} - ${item.license ?? 'see its license text'} - ${item.source}`,
		);
	lines.push('');
	// One copy of each distinct text, after the packages whose files hold it.
	const groups = new Map();
	for (const item of packages)
		for (const { origin, text } of item.texts) {
			const group = groups.get(text) ?? [];
			group.push(`${item.name} ${item.version}: ${origin}`);
			groups.set(text, group);
		}
	for (const [text, users] of groups) lines.push(LIGHT, ...users, LIGHT, '', text, '');
	return lines;
}

function document(title, scope, parts) {
	const lines = [
		title,
		'='.repeat(title.length),
		'',
		...scope.flatMap((text) => wrap(text, '').concat('')),
	];
	for (const part of parts) lines.push(...part);
	return `${lines.join('\n').trimEnd()}\n`;
}

function orchbeeLicense(root) {
	return [
		...heading('Orchbee'),
		...block(
			'Orchbee',
			['License: MIT'],
			[normalized(readFileSync(path.join(root, 'LICENSE'), 'utf8'))],
		),
	];
}

function componentParts(sections, notices) {
	const linked = new Set(sections.flatMap((section) => section.files));
	const unlinked = notices.filter((notice) => !linked.has(notice.file));
	return [
		...(sections.length === 0
			? []
			: [[...heading('Components'), ...sections.flatMap(renderSection)]]),
		...(unlinked.length === 0
			? []
			: [
					[
						...heading('Notices of adapted sources'),
						...unlinked.flatMap((notice) =>
							block(notice.relative, [], [normalized(readFileSync(notice.file, 'utf8'))]),
						),
					],
				]),
	];
}

/**
 * Writes `THIRD_PARTY_NOTICES.txt` and the Rust standard library's notices into the cohort
 * `destination`, for the workspace packages `packages` built for the Rust `target`, by default the
 * host's, on `platform` (`windows`, `macos` or `linux`). `development` adds what only development
 * and test builds carry. Returns the paths it wrote, relative to `destination`.
 */
export function writeProgramNotices({
	root = repositoryRoot,
	destination,
	target = hostTarget(),
	packages,
	platform,
	development,
}) {
	if (!PLATFORM_NAMES.has(platform)) throw new Error(`Unknown notices platform ${platform}`);
	const sections = readInventory(root).filter((section) =>
		carries(section, 'programs', platform, development),
	);
	const crates = cargoPackages(root, target, packages);
	const problems = [];
	const described = packageTexts(crates.packages, 'crate', readLicenseTexts(root), problems);
	const toolchain = toolchainRuntimes(platform, problems);
	const notices = workspaceNotices(root, [
		...crates.workspace,
		path.join(root, 'apps', 'terminal-host'),
	]);
	if (problems.length > 0) throw incomplete(problems);
	const text = document(
		'Third-party notices: Orchbee native programs',
		[
			`Platform: ${PLATFORM_NAMES.get(platform)} (${target})`,
			'This file accompanies the native programs of Orchbee: the orchbee command and its ' +
				'Server, Worker and Terminal Host, their launchers' +
				(development ? ", and the File Bar's process and rg of a development or test build" : '') +
				". It reproduces Orchbee's license and the notices and license texts of the third-party " +
				'software these programs contain. licenses/rust-standard-library.html, beside it, holds ' +
				'the notices of the Rust standard library.',
		],
		[
			orchbeeLicense(root),
			...componentParts(sections, notices),
			[
				...heading('Toolchain runtimes'),
				...block(
					'Rust standard library',
					[
						`Version: ${toolchain.rustVersion}`,
						'License: MIT OR Apache-2.0',
						'Notices: licenses/rust-standard-library.html holds the notices of the standard ' +
							'library and of the crates it is built from',
					],
					[],
				),
				...toolchain.runtimes.flatMap((runtime) =>
					block(runtime.name, runtime.fields, [normalized(readFileSync(runtime.file, 'utf8'))]),
				),
			],
			renderPackages(
				'Crates',
				'The programs contain these crates. Each license text below follows the crates whose ' +
					'files hold it.',
				described,
			),
		],
	);
	mkdirSync(path.join(destination, path.dirname(RUST_LIBRARY_NOTICES)), { recursive: true });
	copyFileSync(
		toolchain.rustLibrary,
		path.join(destination, ...RUST_LIBRARY_NOTICES.split('/')),
		constants.COPYFILE_EXCL,
	);
	writeFileSync(path.join(destination, NOTICES_FILE), text, { flag: 'wx', mode: 0o600 });
	return [NOTICES_FILE, RUST_LIBRARY_NOTICES];
}

/**
 * Writes the Desktop application's `THIRD_PARTY_NOTICES.txt` into `destination`, the folder that
 * holds Electron's `LICENSES.chromium.html` in the packaged application of `platform`.
 */
function writeWindowNotices({ root = repositoryRoot, destination, platform }) {
	if (!PLATFORM_NAMES.has(platform)) throw new Error(`Unknown notices platform ${platform}`);
	const sections = readInventory(root).filter((section) =>
		carries(section, 'window', platform, false),
	);
	const npm = npmPackages(root, ['@orchbee/web...', '@orchbee/desktop...']);
	const problems = [];
	const described = packageTexts(npm.packages, 'npm', readLicenseTexts(root), problems);
	if (problems.length > 0) throw incomplete(problems);
	const text = document(
		'Third-party notices: Orchbee Desktop',
		[
			`Platform: ${PLATFORM_NAMES.get(platform)}`,
			'This file accompanies the Orchbee Desktop application: its window, the Electron runtime ' +
				"it runs in and its installer. It reproduces Orchbee's license and the notices and " +
				'license texts of the third-party software the application contains. ' +
				'LICENSES.chromium.html, beside it, holds the notices of Chromium, Node.js and the other ' +
				"components of Electron. The native programs in the application's cohort folder carry " +
				'their own THIRD_PARTY_NOTICES.txt.',
		],
		[
			orchbeeLicense(root),
			...componentParts(sections, workspaceNotices(root, npm.workspace)),
			renderPackages(
				'npm packages',
				'The application contains code from these npm packages, the production dependencies of ' +
					'its window. Each license text below follows the packages whose files hold it.',
				described,
			),
		],
	);
	writeFileSync(path.join(destination, NOTICES_FILE), text, { flag: 'wx' });
}

const usage = 'Usage: node scripts/third-party-notices.mjs window --output <existing-directory>';

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const [command, flag, output, ...rest] = process.argv.slice(2);
	if (command !== 'window' || flag !== '--output' || output === undefined || rest.length > 0)
		throw new Error(usage);
	if (!path.isAbsolute(output) || !statSync(output).isDirectory())
		throw new Error('--output must name an existing absolute directory');
	const platform = { win32: 'windows', darwin: 'macos', linux: 'linux' }[process.platform];
	if (platform === undefined)
		throw new Error(`The Desktop application is not built on ${process.platform}`);
	writeWindowNotices({ destination: output, platform });
}
