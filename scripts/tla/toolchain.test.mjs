import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { sha256Bytes } from './model-inputs.mjs';
import { provisionJar } from './toolchain.mjs';

test('concurrent tool provisioning publishes only the pinned bytes and removes staging files', async (t) => {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-provision-'));
	t.after(() => rm(root, { recursive: true, force: true }));
	const bytes = Buffer.from('pinned tool fixture');
	const tool = {
		version: 'fixture',
		sha256: sha256Bytes(bytes),
		downloadUrl: `data:application/octet-stream;base64,${bytes.toString('base64')}`,
	};
	const jar = path.join(root, 'tool.jar');
	await Promise.all(Array.from({ length: 4 }, () => provisionJar(tool, jar, true)));
	assert.deepEqual(await readFile(jar), bytes);
	assert.deepEqual(await readdir(root), ['tool.jar']);
	await provisionJar({ ...tool, downloadUrl: 'invalid:' }, jar, false);
});

test('provisioning rejects mismatched downloads and missing configured jars without residue', async (t) => {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-provision-'));
	t.after(() => rm(root, { recursive: true, force: true }));
	const tool = {
		version: 'fixture',
		sha256: sha256Bytes(Buffer.from('expected')),
		downloadUrl: 'data:application/octet-stream,wrong',
	};
	const jar = path.join(root, 'tool.jar');
	await assert.rejects(provisionJar(tool, jar, false), /does not match pinned/u);
	await assert.rejects(provisionJar(tool, jar, true), /failed its pinned SHA-256 check/u);
	assert.deepEqual(await readdir(root), []);
});
