import { readFile, rm, stat } from 'node:fs/promises';
import path from 'node:path';

import { z } from 'zod';

/**
 * The marker Setup writes into the window store's `desktop` folder when its own launch starts
 * Orchbee: the window version it installed, which only that launch may switch the open window to.
 */
const SETUP_LAUNCH_FILE_NAME = 'setup-launch.json';

/** A marker older than this was not written by the Setup that is finishing now. */
const SETUP_LAUNCH_MAXIMUM_AGE_MILLISECONDS = 120_000;

const SetupLaunchMarkerSchema = z.object({ version: z.string() });

interface MarkerFile {
	readonly modifiedAt: number;
	readonly text: string;
}

/**
 * Reads and deletes Setup's launch marker, and answers whether this launch is the one Setup made:
 * the marker exists, was modified at most 120 seconds before `now`, parses, and names
 * `offeredVersion`. The file is deleted in every case once read, so no later launch can act on it;
 * a file that cannot be deleted answers no, since it could then act again.
 */
export async function takeSetupLaunch(options: {
	readonly desktopRoot: string;
	readonly now: number;
	readonly offeredVersion: string | null;
}): Promise<boolean> {
	const file = path.join(options.desktopRoot, SETUP_LAUNCH_FILE_NAME);
	const marker = await readMarker(file);
	if (marker === null || !(await deleteMarker(file))) return false;
	if (options.now - marker.modifiedAt > SETUP_LAUNCH_MAXIMUM_AGE_MILLISECONDS) return false;
	const version = parseVersion(marker.text);
	return version !== null && version === options.offeredVersion;
}

/** Deletes a marker a previous Setup left behind, when a first instance starts, without acting on it. */
export async function discardSetupLaunch(desktopRoot: string): Promise<void> {
	await deleteMarker(path.join(desktopRoot, SETUP_LAUNCH_FILE_NAME));
}

async function readMarker(file: string): Promise<MarkerFile | null> {
	try {
		const [text, info] = await Promise.all([readFile(file, 'utf8'), stat(file)]);
		return { modifiedAt: info.mtimeMs, text };
	} catch {
		return null;
	}
}

async function deleteMarker(file: string): Promise<boolean> {
	try {
		await rm(file, { force: true });
		return true;
	} catch {
		return false;
	}
}

function parseVersion(text: string): string | null {
	let document: unknown;
	try {
		document = JSON.parse(text);
	} catch {
		return null;
	}
	const marker = SetupLaunchMarkerSchema.safeParse(document);
	return marker.success ? marker.data.version : null;
}
