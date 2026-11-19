import { existsSync } from 'node:fs';
import { mkdtemp, rm, utimes, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { discardSetupLaunch, takeSetupLaunch } from '../setup-launch';

const NOW = Date.parse('2026-11-18T12:00:00Z');
const OFFERED = '0.1.0-beta.21';
const MARKER_FILE_NAME = 'setup-launch.json';

let desktopRoot: string;
let marker: string;

beforeEach(async () => {
	desktopRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-setup-launch-'));
	marker = path.join(desktopRoot, MARKER_FILE_NAME);
});

afterEach(async () => {
	await rm(desktopRoot, { force: true, recursive: true });
});

/** Writes the marker, last modified `ageMilliseconds` before `NOW`. */
async function writeMarker(text: string, ageMilliseconds = 0): Promise<void> {
	await writeFile(marker, text);
	const modified = new Date(NOW - ageMilliseconds);
	await utimes(marker, modified, modified);
}

function takeAt(offeredVersion: string | null = OFFERED): Promise<boolean> {
	return takeSetupLaunch({ desktopRoot, now: NOW, offeredVersion });
}

describe('takeSetupLaunch', () => {
	it('counts a fresh marker that names the offered version, and deletes it', async () => {
		await writeMarker(JSON.stringify({ version: OFFERED }));

		expect(await takeAt()).toBe(true);
		expect(existsSync(marker)).toBe(false);
	});

	it('refuses a marker older than two minutes, and deletes it', async () => {
		await writeMarker(JSON.stringify({ version: OFFERED }), 120_001);

		expect(await takeAt()).toBe(false);
		expect(existsSync(marker)).toBe(false);
	});

	it('refuses a marker that names another version, and deletes it', async () => {
		await writeMarker(JSON.stringify({ version: '0.1.0-beta.99' }));

		expect(await takeAt()).toBe(false);
		expect(existsSync(marker)).toBe(false);
	});

	it('refuses a marker that does not parse, and deletes it', async () => {
		await writeMarker('{ not json');

		expect(await takeAt()).toBe(false);
		expect(existsSync(marker)).toBe(false);
	});

	it('refuses when no version is offered, and deletes the marker', async () => {
		await writeMarker(JSON.stringify({ version: OFFERED }));

		expect(await takeAt(null)).toBe(false);
		expect(existsSync(marker)).toBe(false);
	});

	it('refuses when no marker exists', async () => {
		expect(await takeAt()).toBe(false);
	});

	it('discards a marker left by an earlier Setup, so a later launch cannot act on it', async () => {
		await writeMarker(JSON.stringify({ version: OFFERED }));

		await discardSetupLaunch(desktopRoot);

		expect(existsSync(marker)).toBe(false);
		expect(await takeAt()).toBe(false);
	});
});
