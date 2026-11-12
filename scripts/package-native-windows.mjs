// Produce the updater's version 2 release manifest from a completed staged Windows cohort. Signing
// is a separate step, `scripts/sign-release-manifest.mjs`, which writes `release.json.sig`.
// No upload, application launch, installation or PATH mutation occurs here. A release feed packages
// a cohort staged with `--artifact-stem orchbee`; a test cohort (`ob-test-<16 hex>`) packages the
// same way for rehearsals signed with the test key. Development cohorts are refused.
import { createHash } from 'node:crypto';
import {
	closeSync,
	fsyncSync,
	lstatSync,
	openSync,
	readFileSync,
	readdirSync,
	readSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifiedCapsuleDigest } from './stage-local-runtime.mjs';
import { verifyConptyBundle } from '../apps/terminal-host/scripts/windows-conpty.mjs';

// Each cohort role's key in `cohort.json` and the suffix its file adds to the environment stem.
const peers = [
	['orchbee', ''],
	['orchbee-server', '-server'],
	['orchbee-worker', '-worker'],
	['orchbee-terminal-host', '-terminal-host'],
];
const packagedStem = /^(?:orchbee|ob-test-[0-9a-f]{16})$/u;
const maximumFile = 512 * 1024 * 1024;
const maximumTotal = 2 * 1024 * 1024 * 1024;
// The updater reads at most 1 MiB of manifest; this bound never shrinks.
const maximumManifest = 1024 * 1024;
const frames = ['obkg', 'obpb', 'obrw', 'obtf'];
// Each component names its files and every generation registry contract they encode or decode.
const components = [
	{
		id: 'client',
		files: (file, stem) => file === `${stem}.exe`,
		contracts: ['client-server', 'machine-hub', 'machine-link', ...frames, 'machine-route'],
	},
	{
		id: 'server',
		files: (file, stem) => file === `${stem}-server.exe`,
		contracts: ['client-server', 'agent-hook'],
	},
	{
		id: 'worker',
		files: (file, stem) => file === `${stem}-worker.exe`,
		contracts: ['worker-host', ...frames],
	},
	{
		id: 'terminal-host',
		files: (file, stem) => file === `${stem}-terminal-host.exe` || file.startsWith('conpty/'),
		contracts: ['worker-host', ...frames],
	},
	{
		id: 'agent-observation-capsule',
		files: (file) => file.startsWith('agent-observation-adapters/'),
		contracts: ['agent-hook'],
	},
];
// SemVer 2.0.0 without build metadata.
const identifier = String.raw`(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)`;
const semanticVersion = new RegExp(
	String.raw`^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-${identifier}(?:\.${identifier})*)?$`,
	'u',
);

/** Every contract of the generation registry with its supported generations. */
function generationRanges() {
	const registry = JSON.parse(
		readFileSync(new URL('../schemas/connection/v1/connections.json', import.meta.url), 'utf8'),
	);
	const ranges = new Map();
	for (const group of ['negotiatedConnections', 'frameFormats', 'deliveries'])
		for (const [name, entry] of Object.entries(registry[group]))
			ranges.set(name, { lowest: entry.generations.lowest, highest: entry.generations.highest });
	return ranges;
}

/**
 * The manifest's component entries for a cohort whose executables carry `stem`; every registry
 * contract must belong to a component.
 */
export function componentEntries(files, version, stem = 'orchbee') {
	const ranges = generationRanges();
	const listed = new Set(components.flatMap((component) => component.contracts));
	for (const name of ranges.keys())
		if (!listed.has(name)) throw new Error(`No component lists the registry contract ${name}`);
	return components.map((component) => {
		const paths = files.map((file) => file.path).filter((file) => component.files(file, stem));
		if (paths.length === 0) throw new Error(`Component ${component.id} has no files`);
		const connections = Object.fromEntries(
			component.contracts.map((name) => {
				const generations = ranges.get(name);
				if (generations === undefined) throw new Error(`Unknown registry contract ${name}`);
				return [name, { generations }];
			}),
		);
		return { id: component.id, version, files: paths, connections };
	});
}

function digest(filename) {
	const fd = openSync(filename, 'r');
	try {
		const hash = createHash('sha256');
		const buffer = Buffer.alloc(64 * 1024);
		let size = 0;
		while (true) {
			const count = readSync(fd, buffer);
			if (count === 0) break;
			size += count;
			if (size > maximumFile) throw new Error('Release file exceeds 512 MiB');
			hash.update(buffer.subarray(0, count));
		}
		return { sha256: hash.digest('hex'), size };
	} finally {
		closeSync(fd);
	}
}
function validPath(value) {
	return (
		value.length <= 240 &&
		value.split('/').length <= 12 &&
		value
			.split('/')
			.every(
				(part) =>
					part.length > 0 &&
					part.length <= 120 &&
					/^[A-Za-z0-9._-]+$/u.test(part) &&
					!part.endsWith('.') &&
					!/^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)/iu.test(part),
			)
	);
}

export function packageWindows(cohort, channel, version, buildId) {
	if (
		!path.isAbsolute(cohort) ||
		!['stable', 'preview'].includes(channel) ||
		typeof version !== 'string' ||
		version.length > 64 ||
		!semanticVersion.test(version) ||
		typeof buildId !== 'string' ||
		!/^[A-Za-z0-9._+-]{1,96}$/u.test(buildId)
	)
		throw new Error(
			'Expected absolute cohort, stable|preview channel, SemVer version without build metadata, and build identity',
		);
	const destination = path.join(cohort, 'release.json');
	// Never overwrite a previously published manifest or its signature, or follow their symlinks.
	for (const existing of [destination, destination + '.sig']) {
		try {
			lstatSync(existing);
			throw new Error(`${path.basename(existing)} already exists`);
		} catch (error) {
			if (error.code !== 'ENOENT') throw error;
		}
	}
	const files = [];
	const seen = new Set();
	let total = 0;
	let entryCount = 0;
	function visit(directory, prefix = '') {
		const metadata = lstatSync(directory);
		if (!metadata.isDirectory() || metadata.isSymbolicLink())
			throw new Error('Unsafe release directory');
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const relative = prefix + entry.name;
			if (++entryCount > 8192 || !validPath(relative))
				throw new Error('Invalid or oversized release tree');
			const filename = path.join(directory, entry.name);
			const stat = lstatSync(filename);
			if (stat.isSymbolicLink()) throw new Error('Release links are not supported');
			const folded = relative.toLowerCase();
			if (seen.has(folded)) throw new Error('Case alias in release');
			seen.add(folded);
			if (stat.isDirectory()) {
				visit(filename, relative + '/');
				continue;
			}
			if (!stat.isFile() || stat.nlink !== 1) throw new Error('Unsafe release file');
			const content = digest(filename);
			total += content.size;
			if (files.length >= 4096 || total > maximumTotal)
				throw new Error('Release size bound exceeded');
			files.push({ path: relative, url: relative, ...content });
		}
	}
	visit(cohort);
	files.sort((left, right) => (left.path < right.path ? -1 : Number(left.path > right.path)));
	const manifest = JSON.parse(readFileSync(path.join(cohort, 'cohort.json'), 'utf8'));
	if (
		manifest.schemaVersion !== 1 ||
		Object.keys(manifest).length !== 2 ||
		Object.keys(manifest.artifacts ?? {}).length !== 5
	)
		throw new Error('Invalid native cohort manifest');
	const stems = files
		.map((entry) => /^([0-9a-z-]+)\.exe$/u.exec(entry.path)?.[1])
		.filter((stem) => stem !== undefined && packagedStem.test(stem));
	if (stems.length !== 1)
		throw new Error(
			'Expected one orchbee.exe or ob-test-<16 hex>.exe: a release feed packages a cohort staged with --artifact-stem orchbee',
		);
	const stem = stems[0];
	for (const [peer, suffix] of peers) {
		const file = files.find((entry) => entry.path === `${stem}${suffix}.exe`);
		if (!file) throw new Error(`Missing ${stem}${suffix}.exe`);
		if (file.sha256 !== manifest.artifacts[peer]) throw new Error('Cohort binary digest mismatch');
	}
	// The launcher `install` copies to `bin\<stem>.exe`; it belongs to no component.
	if (!files.some((entry) => entry.path === `${stem}-launcher.exe`))
		throw new Error(`Missing ${stem}-launcher.exe`);
	const capsule = verifiedCapsuleDigest(path.join(cohort, 'agent-observation-adapters'));
	if (capsule === null || capsule !== manifest.artifacts['agent-observation-adapters'])
		throw new Error('Cohort adapter capsule digest mismatch');
	verifyConptyBundle(path.join(cohort, 'conpty'));
	const bytes =
		JSON.stringify({
			schemaVersion: 2,
			product: 'orchbee',
			channel,
			platform: 'windows-x86_64',
			version,
			buildId,
			files,
			components: componentEntries(files, version, stem),
		}) + '\n';
	if (Buffer.byteLength(bytes) > maximumManifest) throw new Error('Release manifest exceeds 1 MiB');
	const fd = openSync(destination, 'wx');
	try {
		writeFileSync(fd, bytes);
		fsyncSync(fd);
	} catch (error) {
		closeSync(fd);
		unlinkSync(destination);
		throw error;
	}
	closeSync(fd);
	return destination;
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const [cohort, channel, version, buildId, extra] = process.argv.slice(2);
	if (!cohort || !channel || !version || !buildId || extra !== undefined)
		throw new Error(
			'Usage: node scripts/package-native-windows.mjs <absolute-cohort> <stable|preview> <version> <build-id>',
		);
	process.stdout.write(packageWindows(cohort, channel, version, buildId) + '\n');
}
