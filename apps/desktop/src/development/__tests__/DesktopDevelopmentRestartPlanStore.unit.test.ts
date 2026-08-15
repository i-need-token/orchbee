import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { DesktopDevelopmentRestartPlanStore } from '../DesktopDevelopmentRestartPlanStore';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map((directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('DesktopDevelopmentRestartPlanStore', () => {
	it('moves one pending generation to an exact durable document revision', async () => {
		const filePath = await temporaryPlanPath();
		const store = new DesktopDevelopmentRestartPlanStore(filePath, {
			randomToken: () => 'temporary',
		});

		await store.writePending(7);
		expect(await store.load()).toEqual({
			kind: 'available',
			plan: { generation: 7, state: 'pending', version: 1 },
		});
		await expect(store.markReady('11')).resolves.toBe(true);
		await expect(store.markReady('11')).resolves.toBe(true);
		expect(await store.load()).toEqual({
			kind: 'available',
			plan: { documentRevision: '11', generation: 7, state: 'ready', version: 1 },
		});
		expect(JSON.parse(await readFile(filePath, 'utf8'))).toEqual({
			documentRevision: '11',
			generation: 7,
			state: 'ready',
			version: 1,
		});
	});

	it('does not repair or replace a malformed plan while marking ready', async () => {
		const filePath = await temporaryPlanPath();
		await writeFile(filePath, '{"state":"pending","generation":0}\n', 'utf8');
		const store = new DesktopDevelopmentRestartPlanStore(filePath);

		await expect(store.markReady('3')).resolves.toBe(false);
		expect(await store.load()).toEqual({ kind: 'invalid' });
		expect(await readFile(filePath, 'utf8')).toBe('{"state":"pending","generation":0}\n');
	});

	it('removes only its exact plan file', async () => {
		const filePath = await temporaryPlanPath();
		const store = new DesktopDevelopmentRestartPlanStore(filePath);
		await store.writePending(1);

		await store.remove();
		expect(await store.load()).toEqual({ kind: 'missing' });
	});
});

async function temporaryPlanPath(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-development-plan-'));
	temporaryDirectories.push(directory);
	return path.join(directory, 'restart-plan.json');
}
