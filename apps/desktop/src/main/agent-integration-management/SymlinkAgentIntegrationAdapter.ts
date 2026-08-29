import { constants } from 'node:fs';
import { access, lstat, mkdir, readlink, symlink, unlink } from 'node:fs/promises';
import path from 'node:path';

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

export interface SymlinkAgentIntegrationAdapterOptions {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly executableName: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly sourcePath: string;
	readonly targetPath: string;
}

export class SymlinkAgentIntegrationAdapter implements AgentIntegrationAdapter {
	readonly #commandRunner: AgentIntegrationCommandRunner;
	readonly #executableName: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly #sourcePath: string;
	readonly #targetPath: string;

	constructor(options: SymlinkAgentIntegrationAdapterOptions) {
		this.#commandRunner = options.commandRunner;
		this.#executableName = options.executableName;
		this.integrationId = options.integrationId;
		this.managementMode = options.managementMode;
		this.#sourcePath = path.resolve(options.sourcePath);
		this.#targetPath = path.resolve(options.targetPath);
	}

	async inspect(): Promise<DesktopAgentIntegrationSnapshotItem> {
		const [executablePath, sourceAvailable, targetState] = await Promise.all([
			this.#commandRunner.resolveExecutable(this.#executableName),
			fileAvailable(this.#sourcePath),
			inspectExactSymlink(this.#targetPath, this.#sourcePath),
		]);
		const platformIssue =
			this.managementMode === 'manual-only' ? ('unsupported-platform' as const) : null;
		let state: DesktopAgentIntegrationSnapshotItem['state'];
		if (targetState === 'missing') state = 'not-installed';
		else if (targetState === 'exact') state = 'enabled';
		else state = targetState;
		return {
			availability: executablePath === null ? 'not-found' : 'available',
			integrationId: this.integrationId,
			issue:
				platformIssue ??
				(sourceAvailable && targetState !== 'unknown' ? null : 'inspection-failed'),
			managementMode: this.managementMode,
			state,
		};
	}

	async enable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		if (!(await fileAvailable(this.#sourcePath))) return { kind: 'failed' };
		const state = await inspectExactSymlink(this.#targetPath, this.#sourcePath);
		if (state === 'exact') return { kind: 'attempted' };
		if (state !== 'missing') return { kind: state === 'conflict' ? 'conflict' : 'failed' };
		try {
			await mkdir(path.dirname(this.#targetPath), { mode: 0o700, recursive: true });
			await symlink(this.#sourcePath, this.#targetPath, 'file');
			return { kind: 'attempted' };
		} catch (error: unknown) {
			return { kind: isFileExistsError(error) ? 'conflict' : 'failed' };
		}
	}

	async disable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		const state = await inspectExactSymlink(this.#targetPath, this.#sourcePath);
		if (state === 'missing') return { kind: 'attempted' };
		if (state !== 'exact') return { kind: state === 'conflict' ? 'conflict' : 'failed' };
		try {
			await unlink(this.#targetPath);
			return { kind: 'attempted' };
		} catch {
			return { kind: 'failed' };
		}
	}
}

type SymlinkState = 'conflict' | 'exact' | 'missing' | 'unknown';

async function inspectExactSymlink(
	targetPath: string,
	expectedSourcePath: string,
): Promise<SymlinkState> {
	try {
		const metadata = await lstat(targetPath);
		if (!metadata.isSymbolicLink()) return 'conflict';
		const linkedPath = await readlink(targetPath);
		const resolvedLinkedPath = path.resolve(path.dirname(targetPath), linkedPath);
		return resolvedLinkedPath === expectedSourcePath ? 'exact' : 'conflict';
	} catch (error: unknown) {
		return isFileNotFoundError(error) ? 'missing' : 'unknown';
	}
}

async function fileAvailable(filePath: string): Promise<boolean> {
	try {
		await access(filePath, constants.R_OK);
		return true;
	} catch {
		return false;
	}
}

function isFileExistsError(error: unknown): boolean {
	return isNodeError(error) && error.code === 'EEXIST';
}

function isFileNotFoundError(error: unknown): boolean {
	return isNodeError(error) && error.code === 'ENOENT';
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
