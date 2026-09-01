import { createHash } from 'node:crypto';
import {
	cp,
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	readdir,
	readlink,
	rename,
	rm,
	stat,
	unlink,
	writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

import type { AgentIntegrationCommandRunner } from './NodeAgentIntegrationCommandRunner';
import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';
import { classifyManagedResourceRoot } from './agent-integration-state-parsers';

const BOOTSTRAP_MARKER_PREFIX = '// orchbee-agent-observation-bootstrap-v3:';
const DIRECTORY_MARKER_NAME = '.orchbee-agent-observation.json';
const ENTRY_CONTRACT_VERSION = 3;
const MAX_ENTRY_DEPTH = 8;
const MAX_ENTRY_FILES = 64;
const MAX_ENTRY_FILE_BYTES = 128 * 1024;
const MAX_ENTRY_TOTAL_BYTES = 512 * 1024;
const MAX_MARKER_BYTES = 16 * 1024;
const REQUIRED_RUNTIME_FILES = [
	'forward-hook.mjs',
	'hook-sanitizers.mjs',
	'observation-transport.mjs',
] as const;

type GuardedLocalEntryKind = 'extension-directory' | 'omp-module' | 'opencode-module' | 'pi-module';

export interface GuardedLocalAgentIntegrationAdapterOptions {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly entryKind: GuardedLocalEntryKind;
	readonly executableName: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly legacyResourceRoots: readonly string[];
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly resourceRoot: string;
	readonly runtimeSourcePath?: string;
	readonly sourcePath: string;
	readonly targetPath: string;
}

type TargetState = 'conflict' | 'current' | 'legacy' | 'missing' | 'unknown' | 'update-required';

interface DirectoryMarker {
	readonly contractVersion: number;
	readonly files: readonly DirectoryMarkerFile[];
	readonly integrationId: DesktopAgentIntegrationId;
	readonly kind: 'guarded-extension';
}

interface DirectoryMarkerFile {
	readonly path: string;
	readonly sha256: string;
}

interface StagedEntry {
	readonly containerPath: string;
	readonly entryPath: string;
}

/**
 * Installs a provider-local entry whose loader remains silent when Orchbee or
 * its stable Capsule is gone. Legacy exact symlinks are migration candidates;
 * arbitrary or modified targets remain conflicts.
 */
export class GuardedLocalAgentIntegrationAdapter implements AgentIntegrationAdapter {
	readonly #commandRunner: AgentIntegrationCommandRunner;
	readonly #entryKind: GuardedLocalEntryKind;
	readonly #executableName: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly #legacyResourceRoots: readonly string[];
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly #resourceRoot: string;
	readonly #runtimeSourcePath: string | null;
	readonly #sourceRelativePath: string;
	readonly #sourcePath: string;
	readonly #targetPath: string;

	constructor(options: GuardedLocalAgentIntegrationAdapterOptions) {
		this.#commandRunner = options.commandRunner;
		this.#entryKind = options.entryKind;
		this.#executableName = options.executableName;
		this.integrationId = options.integrationId;
		this.#legacyResourceRoots = options.legacyResourceRoots.map((resourceRoot) =>
			path.resolve(resourceRoot),
		);
		this.managementMode = options.managementMode;
		this.#resourceRoot = path.resolve(options.resourceRoot);
		this.#runtimeSourcePath =
			options.runtimeSourcePath === undefined ? null : path.resolve(options.runtimeSourcePath);
		this.#sourcePath = path.resolve(options.sourcePath);
		this.#sourceRelativePath = path.relative(this.#resourceRoot, this.#sourcePath);
		this.#targetPath = path.resolve(options.targetPath);
		if (
			this.#sourceRelativePath === '' ||
			path.isAbsolute(this.#sourceRelativePath) ||
			this.#sourceRelativePath === '..' ||
			this.#sourceRelativePath.startsWith(`..${path.sep}`)
		) {
			throw new Error('A guarded integration source must belong to its managed resource root.');
		}
		if (this.#entryKind === 'extension-directory' && this.#runtimeSourcePath === null) {
			throw new Error('A guarded extension requires its shared runtime source.');
		}
	}

	async inspect(): Promise<DesktopAgentIntegrationSnapshotItem> {
		const [executablePath, sourceReady, targetState] = await Promise.all([
			this.#commandRunner.resolveExecutable(this.#executableName),
			this.#sourceReady(),
			this.#inspectTarget(),
		]);
		const platformIssue =
			this.managementMode === 'manual-only' ? ('unsupported-platform' as const) : null;
		let state: DesktopAgentIntegrationSnapshotItem['state'];
		if (targetState === 'missing') state = 'not-installed';
		else if (
			targetState === 'legacy' ||
			targetState === 'update-required' ||
			(targetState === 'current' && !sourceReady)
		) {
			state = 'update-required';
		} else if (targetState === 'current') state = 'enabled';
		else state = targetState;
		return {
			availability: executablePath === null ? 'not-found' : 'available',
			integrationId: this.integrationId,
			issue: platformIssue ?? (targetState === 'unknown' ? 'inspection-failed' : null),
			managementMode: this.managementMode,
			state,
		};
	}

	async enable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		if (!(await this.#sourceReady())) return { kind: 'failed' };
		const initialState = await this.#inspectTarget();
		if (initialState === 'current') return { kind: 'attempted' };
		if (
			initialState !== 'missing' &&
			initialState !== 'legacy' &&
			initialState !== 'update-required'
		) {
			return { kind: initialState === 'conflict' ? 'conflict' : 'failed' };
		}
		const staged = await this.#stageEntry();
		if (staged === null) return { kind: 'failed' };
		try {
			const currentState = await this.#inspectTarget();
			if (currentState !== initialState) {
				return { kind: currentState === 'conflict' ? 'conflict' : 'failed' };
			}
			if (currentState === 'legacy' || currentState === 'update-required') {
				if (!(await this.#removeOwnedTarget(currentState))) return { kind: 'failed' };
			}
			try {
				await rename(staged.entryPath, this.#targetPath);
				return { kind: 'attempted' };
			} catch (error: unknown) {
				return { kind: isFileExistsError(error) ? 'conflict' : 'failed' };
			}
		} finally {
			await rm(staged.containerPath, { force: true, recursive: true });
		}
	}

	async disable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		const state = await this.#inspectTarget();
		if (state === 'missing') return { kind: 'attempted' };
		if (state !== 'current' && state !== 'legacy' && state !== 'update-required') {
			return { kind: state === 'conflict' ? 'conflict' : 'failed' };
		}
		return (await this.#removeOwnedTarget(state)) ? { kind: 'attempted' } : { kind: 'failed' };
	}

	async #inspectTarget(): Promise<TargetState> {
		let metadata;
		try {
			metadata = await lstat(this.#targetPath);
		} catch (error: unknown) {
			return isFileNotFoundError(error) ? 'missing' : 'unknown';
		}
		if (metadata.isSymbolicLink()) {
			try {
				const linkedPath = path.resolve(
					path.dirname(this.#targetPath),
					await readlink(this.#targetPath),
				);
				const linkedResourceRoot = resourceRootForSource(linkedPath, this.#sourceRelativePath);
				if (linkedResourceRoot === null) return 'conflict';
				return classifyManagedResourceRoot(linkedResourceRoot, {
					legacyResourceRoots: this.#legacyResourceRoots,
					resourceRoot: this.#resourceRoot,
				}) === 'foreign'
					? 'conflict'
					: 'legacy';
			} catch {
				return 'unknown';
			}
		}
		if (this.#entryKind === 'extension-directory') {
			if (!metadata.isDirectory()) return 'conflict';
			return inspectOwnedDirectory(this.#targetPath, this.integrationId);
		}
		if (!metadata.isFile()) return 'conflict';
		const expected = moduleBootstrap(this.#entryKind, this.integrationId, this.#sourcePath);
		const bytes = await readBoundedFile(this.#targetPath, Buffer.byteLength(expected));
		return bytes?.toString('utf8') === expected ? 'current' : 'conflict';
	}

	async #removeOwnedTarget(
		state: Exclude<TargetState, 'conflict' | 'missing' | 'unknown'>,
	): Promise<boolean> {
		try {
			if (state === 'legacy' || this.#entryKind !== 'extension-directory') {
				await unlink(this.#targetPath);
			} else {
				await rm(this.#targetPath, { recursive: true });
			}
			return true;
		} catch {
			return false;
		}
	}

	async #sourceReady(): Promise<boolean> {
		try {
			const sourceMetadata = await stat(this.#sourcePath);
			if (this.#entryKind !== 'extension-directory') return sourceMetadata.isFile();
			if (!sourceMetadata.isDirectory() || this.#runtimeSourcePath === null) return false;
			for (const fileName of REQUIRED_RUNTIME_FILES) {
				if (!(await stat(path.join(this.#runtimeSourcePath, fileName))).isFile()) return false;
			}
			return true;
		} catch {
			return false;
		}
	}

	async #stageEntry(): Promise<StagedEntry | null> {
		const parentPath = path.dirname(this.#targetPath);
		try {
			await mkdir(parentPath, { mode: 0o700, recursive: true });
			const containerPath = await mkdtemp(path.join(parentPath, '.orchbee-agent-entry-'));
			const entryPath = path.join(containerPath, path.basename(this.#targetPath));
			try {
				if (this.#entryKind === 'extension-directory') {
					if (this.#runtimeSourcePath === null) return null;
					await cp(this.#sourcePath, entryPath, {
						errorOnExist: true,
						recursive: true,
						verbatimSymlinks: true,
					});
					await cp(this.#runtimeSourcePath, path.join(entryPath, 'scripts'), {
						errorOnExist: true,
						recursive: true,
						verbatimSymlinks: true,
					});
					const files = await fingerprintDirectory(entryPath);
					if (files === null) return null;
					const marker: DirectoryMarker = {
						contractVersion: ENTRY_CONTRACT_VERSION,
						files,
						integrationId: this.integrationId,
						kind: 'guarded-extension',
					};
					await writeFile(path.join(entryPath, DIRECTORY_MARKER_NAME), JSON.stringify(marker), {
						encoding: 'utf8',
						flag: 'wx',
						mode: 0o600,
					});
					if ((await inspectOwnedDirectory(entryPath, this.integrationId)) !== 'current') {
						return null;
					}
				} else {
					await writeFile(
						entryPath,
						moduleBootstrap(this.#entryKind, this.integrationId, this.#sourcePath),
						{ encoding: 'utf8', flag: 'wx', mode: 0o600 },
					);
				}
				return { containerPath, entryPath };
			} catch {
				await rm(containerPath, { force: true, recursive: true });
				return null;
			}
		} catch {
			return null;
		}
	}
}

function moduleBootstrap(
	entryKind: Exclude<GuardedLocalEntryKind, 'extension-directory'>,
	integrationId: DesktopAgentIntegrationId,
	sourcePath: string,
): string {
	const moduleUrl = JSON.stringify(pathToFileURL(sourcePath).href);
	const marker = `${BOOTSTRAP_MARKER_PREFIX}${integrationId}\n`;
	if (entryKind === 'opencode-module') {
		return `${marker}const orchbeeAgentObservationModuleUrl = ${moduleUrl};\nexport async function OrchbeeAgentObservation(...arguments_) {\n\ttry {\n\t\tconst loaded = await import(orchbeeAgentObservationModuleUrl);\n\t\treturn typeof loaded.OrchbeeAgentObservation === 'function'\n\t\t\t? await loaded.OrchbeeAgentObservation(...arguments_)\n\t\t\t: {};\n\t} catch {\n\t\treturn {};\n\t}\n}\n`;
	}
	return `${marker}const orchbeeAgentObservationModuleUrl = ${moduleUrl};\nmodule.exports = async function orchbeeAgentObservation(api) {\n\ttry {\n\t\tconst loaded = await import(orchbeeAgentObservationModuleUrl);\n\t\tif (typeof loaded.orchbeeAgentObservation === 'function') {\n\t\t\tawait loaded.orchbeeAgentObservation(api);\n\t\t}\n\t} catch {}\n};\n`;
}

async function inspectOwnedDirectory(
	rootPath: string,
	integrationId: DesktopAgentIntegrationId,
): Promise<TargetState> {
	const markerBytes = await readBoundedFile(
		path.join(rootPath, DIRECTORY_MARKER_NAME),
		MAX_MARKER_BYTES,
	);
	if (markerBytes === null) return 'conflict';
	let value: unknown;
	try {
		value = JSON.parse(markerBytes.toString('utf8'));
	} catch {
		return 'conflict';
	}
	const marker = parseDirectoryMarker(value, integrationId);
	if (marker === null) return 'conflict';
	const files = await fingerprintDirectory(rootPath);
	if (files === null || JSON.stringify(files) !== JSON.stringify(marker.files)) return 'conflict';
	return marker.contractVersion === ENTRY_CONTRACT_VERSION ? 'current' : 'update-required';
}

function parseDirectoryMarker(
	value: unknown,
	integrationId: DesktopAgentIntegrationId,
): DirectoryMarker | null {
	if (!isRecord(value) || value['kind'] !== 'guarded-extension') return null;
	if (value['integrationId'] !== integrationId || !Number.isSafeInteger(value['contractVersion'])) {
		return null;
	}
	const contractVersion = Number(value['contractVersion']);
	if (contractVersion < 1 || contractVersion > ENTRY_CONTRACT_VERSION) return null;
	const files = value['files'];
	if (!Array.isArray(files) || files.length > MAX_ENTRY_FILES) return null;
	const parsedFiles: DirectoryMarkerFile[] = [];
	for (const file of files) {
		if (!isRecord(file) || !validRelativePath(file['path']) || !validDigest(file['sha256'])) {
			return null;
		}
		parsedFiles.push({ path: file['path'], sha256: file['sha256'] });
	}
	for (let index = 1; index < parsedFiles.length; index += 1) {
		const previous = parsedFiles[index - 1];
		const current = parsedFiles[index];
		if (previous === undefined || current === undefined || previous.path >= current.path)
			return null;
	}
	return { contractVersion, files: parsedFiles, integrationId, kind: 'guarded-extension' };
}

async function fingerprintDirectory(
	rootPath: string,
): Promise<readonly DirectoryMarkerFile[] | null> {
	const pending: Array<{ readonly depth: number; readonly directory: string }> = [
		{ depth: 0, directory: rootPath },
	];
	const files: DirectoryMarkerFile[] = [];
	let totalBytes = 0;
	while (pending.length > 0) {
		const current = pending.pop();
		if (current === undefined || current.depth > MAX_ENTRY_DEPTH) return null;
		let entries;
		try {
			entries = await readdir(current.directory, { withFileTypes: true });
		} catch {
			return null;
		}
		for (const entry of entries) {
			if (entry.isSymbolicLink()) return null;
			const entryPath = path.join(current.directory, entry.name);
			const relativePath = path.relative(rootPath, entryPath).split(path.sep).join('/');
			if (relativePath === DIRECTORY_MARKER_NAME) continue;
			if (entry.isDirectory()) {
				pending.push({ depth: current.depth + 1, directory: entryPath });
				continue;
			}
			if (!entry.isFile() || files.length >= MAX_ENTRY_FILES) return null;
			const bytes = await readBoundedFile(entryPath, MAX_ENTRY_FILE_BYTES);
			if (bytes === null) return null;
			totalBytes += bytes.byteLength;
			if (totalBytes > MAX_ENTRY_TOTAL_BYTES) return null;
			files.push({
				path: relativePath,
				sha256: createHash('sha256').update(bytes).digest('hex'),
			});
		}
	}
	return files.sort((left, right) => left.path.localeCompare(right.path));
}

async function readBoundedFile(filePath: string, maximumBytes: number): Promise<Buffer | null> {
	try {
		const metadata = await lstat(filePath);
		if (!metadata.isFile() || metadata.size > maximumBytes) return null;
		const bytes = await readFile(filePath);
		return bytes.byteLength <= maximumBytes ? bytes : null;
	} catch {
		return null;
	}
}

function validRelativePath(value: unknown): value is string {
	return (
		typeof value === 'string' &&
		value.length > 0 &&
		value.length <= 256 &&
		!path.posix.isAbsolute(value) &&
		value.split('/').every((segment) => segment !== '' && segment !== '.' && segment !== '..')
	);
}

function validDigest(value: unknown): value is string {
	return typeof value === 'string' && /^[0-9a-f]{64}$/u.test(value);
}

function resourceRootForSource(sourcePath: string, relativeSourcePath: string): string | null {
	let candidateRoot = sourcePath;
	for (const _segment of relativeSourcePath.split(path.sep)) {
		candidateRoot = path.dirname(candidateRoot);
	}
	return path.resolve(candidateRoot, relativeSourcePath) === sourcePath ? candidateRoot : null;
}

function isFileExistsError(error: unknown): boolean {
	return isNodeError(error) && (error.code === 'EEXIST' || error.code === 'ENOTEMPTY');
}

function isFileNotFoundError(error: unknown): boolean {
	return isNodeError(error) && error.code === 'ENOENT';
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
