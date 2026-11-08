import { chmod, mkdir, open, readdir, readFile, rm, unlink } from 'node:fs/promises';
import path from 'node:path';

import { compareProductVersions } from '../product-version';

// Cross-runtime contract: search for `upgrade-backup-folders` before changing the folder name, the
// relative path, the numbered copies, or retention; native writers back up into the same folders.
/** How many upgrade folders the root keeps; creating another deletes the older ones. */
const KEPT_UPGRADE_FOLDERS = 3;
/** Names taken beyond this many in one folder leave the record unchanged instead of searching on. */
const NAME_LIMIT = 1_000;
const UPGRADE = '-upgrade-';

/**
 * Copies `original`, a record about to be migrated, into its upgrade backup and flushes it before
 * the store replaces the record. Resolves the copy's path; a rejection leaves the record unchanged.
 */
export type UpgradeBackup = (recordPath: string, original: Uint8Array) => Promise<string>;

export interface UpgradeBackupOptions {
	/** The local date, `YYYY-MM-DD`. */
	readonly today?: () => string;
}

/**
 * The upgrade backup of `dataRoot` for `productVersion`: every record this version migrates shares
 * `internal/backups/<YYYY-MM-DD>-upgrade-<version>/`, holding each copy at the record's path
 * relative to the root.
 */
export function createUpgradeBackup(
	dataRoot: string,
	productVersion: string,
	options: UpgradeBackupOptions = {},
): UpgradeBackup {
	const root = path.resolve(dataRoot);
	const today = options.today ?? localDate;
	return async (recordPath, original) => {
		const relative = path.relative(root, path.resolve(recordPath));
		if (relative.length === 0 || relative.startsWith('..') || path.isAbsolute(relative)) {
			throw new Error('The record is outside the data root.');
		}
		const folder = await upgradeFolder(
			path.join(root, 'internal', 'backups'),
			today(),
			productVersion,
		);
		return copyInto(path.join(folder, relative), original);
	};
}

/**
 * The upgrade folder of `version`: an existing `<date>-upgrade-<version>` folder, or a new one named
 * by `date`, after which only the {@link KEPT_UPGRADE_FOLDERS} newest upgrade folders stay.
 */
async function upgradeFolder(backups: string, date: string, version: string): Promise<string> {
	await privateDirectory(backups);
	const held = versionFolderName(version);
	const folders = await upgradeFolders(backups);
	const existing = folders.find((folder) => folder.version === held);
	if (existing !== undefined) return path.join(backups, existing.name);
	const name = `${date}${UPGRADE}${held}`;
	const folder = path.join(backups, name);
	await privateDirectory(folder);
	folders.push({ date, name, version: held });
	folders.sort(
		(left, right) =>
			compareText(right.date, left.date) || compareProductVersions(right.version, left.version),
	);
	await Promise.all(
		folders.slice(KEPT_UPGRADE_FOLDERS).map(async (old) =>
			// A folder that cannot go stays until the next upgrade folder is created.
			rm(path.join(backups, old.name), { force: true, recursive: true }).catch(() => undefined),
		),
	);
	return folder;
}

interface UpgradeFolder {
	readonly date: string;
	readonly name: string;
	readonly version: string;
}

async function upgradeFolders(backups: string): Promise<UpgradeFolder[]> {
	const folders: UpgradeFolder[] = [];
	for (const entry of await readdir(backups, { withFileTypes: true })) {
		if (!entry.isDirectory()) continue;
		const date = entry.name.slice(0, 10);
		const rest = entry.name.slice(10);
		if (!/^\d{4}-\d{2}-\d{2}$/u.test(date) || !rest.startsWith(UPGRADE)) continue;
		const version = rest.slice(UPGRADE.length);
		if (version.length > 0) folders.push({ date, name: entry.name, version });
	}
	return folders;
}

/**
 * Writes `original` at `target` and flushes it. A copy already there with the same content is kept;
 * different content takes the next free `-2`, `-3` and so on before the extension, so the first
 * original a version backed up is never replaced.
 */
async function copyInto(target: string, original: Uint8Array): Promise<string> {
	const parent = path.dirname(target);
	await privateDirectory(parent);
	const leaf = path.basename(target);
	const dot = leaf.lastIndexOf('.');
	const stem = dot === -1 ? leaf : leaf.slice(0, dot);
	const extension = dot === -1 ? '' : leaf.slice(dot);
	for (let number = 1; number <= NAME_LIMIT; number += 1) {
		const candidate = path.join(parent, `${number <= 1 ? stem : `${stem}-${number}`}${extension}`);
		let file;
		try {
			file = await open(candidate, 'wx', 0o600);
		} catch (error: unknown) {
			if (!isNodeError(error) || error.code !== 'EEXIST') throw error;
			if (Buffer.from(original).equals(await readFile(candidate))) return candidate;
			continue;
		}
		try {
			try {
				await file.writeFile(original);
				await file.sync();
			} finally {
				await file.close();
			}
		} catch (error: unknown) {
			await unlink(candidate).catch(() => undefined);
			throw error;
		}
		return candidate;
	}
	throw new Error('Every backup name is taken.');
}

/** The folder-name spelling of a product version: every character but `[A-Za-z0-9._-]` becomes `-`. */
function versionFolderName(version: string): string {
	return version.replaceAll(/[^A-Za-z0-9._-]/gu, '-');
}

async function privateDirectory(directory: string): Promise<void> {
	await mkdir(directory, { mode: 0o700, recursive: true });
	if (process.platform !== 'win32') await chmod(directory, 0o700);
}

function localDate(): string {
	const now = new Date();
	const month = String(now.getMonth() + 1).padStart(2, '0');
	const day = String(now.getDate()).padStart(2, '0');
	return `${now.getFullYear()}-${month}-${day}`;
}

function compareText(left: string, right: string): number {
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
