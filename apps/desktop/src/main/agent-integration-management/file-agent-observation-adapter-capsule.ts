import { createHash } from 'node:crypto';
import { cp, lstat, mkdir, mkdtemp, open, readFile, readdir, rename, rm } from 'node:fs/promises';
import path from 'node:path';

export const AGENT_OBSERVATION_CAPSULE_VERSION = '0.4.1';

const CAPSULE_CONTRACT_VERSION = 3;
const MAX_CAPSULE_ENTRIES = 128;
const MAX_CAPSULE_DEPTH = 8;
const MAX_CAPSULE_FILE_BYTES = 512 * 1024;
const MAX_CAPSULE_TOTAL_BYTES = 2 * 1024 * 1024;
const MAX_MANIFEST_BYTES = 1024;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;

interface CapsulePayloadEntry {
	readonly kind: 'directory' | 'file';
	readonly path: string;
	readonly sha256?: string;
	readonly size?: number;
}

export interface AgentObservationAdapterCapsule {
	readonly rootPath: string;
	isReady(): Promise<boolean>;
	prepare(): Promise<boolean>;
}

export interface FileAgentObservationAdapterCapsuleOptions {
	readonly rootPath: string;
	readonly sourcePath: string;
}

/**
 * Materializes one immutable, versioned copy of the bundled adapters outside the
 * application bundle. Persisted provider entries can therefore survive an app
 * move or uninstall without retaining a path into `/Applications`.
 */
export class FileAgentObservationAdapterCapsule implements AgentObservationAdapterCapsule {
	readonly rootPath: string;
	readonly #sourcePath: string;

	static computePayloadDigest(rootPath: string): Promise<string | null> {
		return computeAgentObservationCapsulePayloadDigest(rootPath);
	}

	constructor(options: FileAgentObservationAdapterCapsuleOptions) {
		this.rootPath = path.resolve(options.rootPath);
		this.#sourcePath = path.resolve(options.sourcePath);
	}

	async isReady(): Promise<boolean> {
		const currentDigest = await readValidCapsulePayloadDigest(this.rootPath);
		if (currentDigest === null) return false;
		if (this.rootPath === this.#sourcePath) return true;
		try {
			await lstat(this.#sourcePath);
		} catch (error: unknown) {
			return isFileNotFoundError(error);
		}
		return (await readValidCapsulePayloadDigest(this.#sourcePath)) === currentDigest;
	}

	async prepare(): Promise<boolean> {
		if (await this.isReady()) return true;
		if (!(await isValidCapsule(this.#sourcePath))) return false;

		const parentPath = path.dirname(this.rootPath);
		await mkdir(parentPath, { mode: 0o700, recursive: true });
		const stagingDirectory = await mkdtemp(path.join(parentPath, '.orchbee-agent-capsule-'));
		const stagedRoot = path.join(stagingDirectory, 'payload');
		try {
			await cp(this.#sourcePath, stagedRoot, {
				errorOnExist: true,
				recursive: true,
				verbatimSymlinks: true,
			});
			if (!(await isValidCapsule(stagedRoot))) return false;
			try {
				await rename(stagedRoot, this.rootPath);
			} catch (error: unknown) {
				if (!isAlreadyExistsError(error) || !(await this.isReady())) {
					return false;
				}
			}
			return true;
		} catch {
			return false;
		} finally {
			await rm(stagingDirectory, { force: true, recursive: true });
		}
	}
}

async function isValidCapsule(rootPath: string): Promise<boolean> {
	return (await readValidCapsulePayloadDigest(rootPath)) !== null;
}

async function readValidCapsulePayloadDigest(rootPath: string): Promise<string | null> {
	try {
		if (!(await lstat(rootPath)).isDirectory()) return null;
	} catch {
		return null;
	}
	const manifest = await readManifest(path.join(rootPath, 'capsule.json'));
	const declaredDigest = manifest?.['payloadSha256'];
	if (
		manifest?.['contractVersion'] !== CAPSULE_CONTRACT_VERSION ||
		manifest['payloadVersion'] !== AGENT_OBSERVATION_CAPSULE_VERSION ||
		typeof declaredDigest !== 'string' ||
		!SHA256_PATTERN.test(declaredDigest) ||
		Object.keys(manifest).sort().join(',') !== 'contractVersion,payloadSha256,payloadVersion'
	) {
		return null;
	}
	for (const relativePath of [
		'bundle/package.json',
		'bundle/scripts/forward-hook.mjs',
		'bundle/scripts/hook-sanitizers.mjs',
		'bundle/scripts/observation-transport.mjs',
	]) {
		try {
			if (!(await lstat(path.join(rootPath, relativePath))).isFile()) return null;
		} catch {
			return null;
		}
	}
	const computedDigest = await computeAgentObservationCapsulePayloadDigest(rootPath);
	return computedDigest === declaredDigest ? declaredDigest : null;
}

async function computeAgentObservationCapsulePayloadDigest(
	rootPath: string,
): Promise<string | null> {
	try {
		if (!(await lstat(rootPath)).isDirectory()) return null;
	} catch {
		return null;
	}
	let entryCount = 0;
	let totalBytes = 0;
	const payloadEntries: CapsulePayloadEntry[] = [];
	const pending: Array<{ readonly depth: number; readonly directory: string }> = [
		{ depth: 0, directory: rootPath },
	];
	while (pending.length > 0) {
		const current = pending.pop();
		if (current === undefined || current.depth > MAX_CAPSULE_DEPTH) return null;
		let entries;
		try {
			entries = await readdir(current.directory, { withFileTypes: true });
		} catch {
			return null;
		}
		for (const entry of entries) {
			entryCount += 1;
			if (entryCount > MAX_CAPSULE_ENTRIES || entry.isSymbolicLink()) return null;
			const entryPath = path.join(current.directory, entry.name);
			const relativePath = path.relative(rootPath, entryPath).split(path.sep).join('/');
			if (entry.isDirectory()) {
				payloadEntries.push({ kind: 'directory', path: relativePath });
				pending.push({
					depth: current.depth + 1,
					directory: entryPath,
				});
				continue;
			}
			if (!entry.isFile()) return null;
			if (relativePath === 'capsule.json') continue;
			let bytes: Buffer;
			try {
				const metadata = await lstat(entryPath);
				if (!metadata.isFile() || metadata.size > MAX_CAPSULE_FILE_BYTES) return null;
				bytes = await readFile(entryPath);
			} catch {
				return null;
			}
			if (bytes.byteLength > MAX_CAPSULE_FILE_BYTES) return null;
			totalBytes += bytes.byteLength;
			if (totalBytes > MAX_CAPSULE_TOTAL_BYTES) return null;
			payloadEntries.push({
				kind: 'file',
				path: relativePath,
				sha256: createHash('sha256').update(bytes).digest('hex'),
				size: bytes.byteLength,
			});
		}
	}
	payloadEntries.sort(compareCapsulePayloadEntries);
	return createHash('sha256').update(JSON.stringify(payloadEntries)).digest('hex');
}

function compareCapsulePayloadEntries(
	left: CapsulePayloadEntry,
	right: CapsulePayloadEntry,
): number {
	if (left.path < right.path) return -1;
	return left.path > right.path ? 1 : 0;
}

async function readManifest(filePath: string): Promise<Record<string, unknown> | null> {
	let handle: Awaited<ReturnType<typeof open>> | undefined;
	try {
		handle = await open(filePath, 'r');
		const metadata = await handle.stat();
		if (!metadata.isFile() || metadata.size > MAX_MANIFEST_BYTES) return null;
		const bytes = Buffer.alloc(MAX_MANIFEST_BYTES + 1);
		const { bytesRead } = await handle.read(bytes, 0, bytes.byteLength, 0);
		if (bytesRead > MAX_MANIFEST_BYTES) return null;
		const value: unknown = JSON.parse(bytes.subarray(0, bytesRead).toString('utf8'));
		return isRecord(value) ? value : null;
	} catch {
		return null;
	} finally {
		await handle?.close();
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isFileNotFoundError(error: unknown): boolean {
	return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

function isAlreadyExistsError(error: unknown): boolean {
	return (
		error instanceof Error &&
		'code' in error &&
		(error.code === 'EEXIST' || error.code === 'ENOTEMPTY')
	);
}
