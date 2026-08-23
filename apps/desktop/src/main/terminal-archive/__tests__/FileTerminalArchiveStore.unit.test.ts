import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';

describe('FileTerminalArchiveStore boundary', () => {
	it('treats a missing local archive as an empty stopped terminal', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-missing-terminal-archive-'));
		try {
			const store = new FileTerminalArchiveStore(directory);
			expect(await store.load(terminalPresentationId('missing-presentation'))).toBeNull();
		} finally {
			await rm(directory, { force: true, recursive: true });
		}
	});
});
