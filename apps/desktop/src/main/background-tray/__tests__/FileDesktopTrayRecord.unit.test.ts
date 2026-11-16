import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { FileDesktopTrayRecord } from '../FileDesktopTrayRecord';

describe('FileDesktopTrayRecord', () => {
	it('opens the stored sample of the current record format', async () => {
		const samplePath = fileURLToPath(
			new URL('./record-samples/desktop-tray-v1.json', import.meta.url),
		);

		// readNoticeShown also answers true for a newer version, so the sample's version is checked first.
		expect(JSON.parse(await readFile(samplePath, 'utf8'))).toMatchObject({ version: 1 });
		expect(await new FileDesktopTrayRecord(samplePath).readNoticeShown()).toBe(true);
	});
});
