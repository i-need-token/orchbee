import { mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { CreationIdSchema } from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import { FileTerminalRecoveryLedger } from '../FileTerminalRecoveryLedger';
import { TerminalRecoveryLedgerDocumentSchema } from '../recovery-binding';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('FileTerminalRecoveryLedger', () => {
	it('atomically stores and reloads a private canonical ledger', async () => {
		const directory = await createTemporaryDirectory();
		const ledgerPath = path.join(directory, 'recovery', 'bindings-v1.json');
		const ledger = new FileTerminalRecoveryLedger(ledgerPath, { randomToken: () => 'temporary' });
		const document = createDocument();

		expect(await ledger.load()).toEqual({ kind: 'missing' });
		await ledger.save(document);

		expect(await ledger.load()).toEqual({ document, kind: 'available' });
		expect((await readFile(ledgerPath, 'utf8')).endsWith('\n')).toBe(true);
		expect(await readdir(path.dirname(ledgerPath))).toEqual(['bindings-v1.json']);
		const fileMode = process.platform === 'win32' ? 0o600 : (await stat(ledgerPath)).mode & 0o777;
		const directoryMode =
			process.platform === 'win32' ? 0o700 : (await stat(path.dirname(ledgerPath))).mode & 0o777;
		expect(fileMode).toBe(0o600);
		expect(directoryMode).toBe(0o700);
	});

	it('fails closed for corrupt and oversized files without replacing them', async () => {
		const directory = await createTemporaryDirectory();
		const corruptPath = path.join(directory, 'corrupt.json');
		const oversizedPath = path.join(directory, 'oversized.json');
		await writeFile(corruptPath, '{not-json', 'utf8');
		await writeFile(oversizedPath, 'x'.repeat(128 * 1024 + 1), 'utf8');

		expect(await new FileTerminalRecoveryLedger(corruptPath).load()).toEqual({
			kind: 'unavailable',
			reason: 'invalid',
		});
		expect(await new FileTerminalRecoveryLedger(oversizedPath).load()).toEqual({
			kind: 'unavailable',
			reason: 'invalid',
		});
		expect(await readFile(corruptPath, 'utf8')).toBe('{not-json');
	});
});

async function createTemporaryDirectory(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-recovery-ledger-'));
	temporaryDirectories.push(directory);
	return directory;
}

function createDocument() {
	return TerminalRecoveryLedgerDocumentSchema.parse({
		binding: {
			bindingId: 'desktop-terminal-foundation',
			creationId: CreationIdSchema.parse('11'.repeat(16)),
			launch: {
				arguments: [],
				cellPixelHeight: 16,
				cellPixelWidth: 8,
				columns: 80,
				currentDirectory: '/users/orchbee',
				environment: [{ name: 'TERM', value: 'xterm-256color' }],
				executable: '/bin/sh',
				rows: 24,
				terminalProfile: createDefaultTerminalProfile(),
			},
			state: { kind: 'planned' },
		},
		version: 1,
	});
}
