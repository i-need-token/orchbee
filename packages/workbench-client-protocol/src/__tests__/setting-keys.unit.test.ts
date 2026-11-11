import { readFile } from 'node:fs/promises';
import { expect, it } from 'vitest';
import { SETTING_KEY_TABLE } from '../setting-keys';

it('mirrors the key table the Rust settings registry generates', async () => {
	const generated: unknown = JSON.parse(
		await readFile(
			new URL('../../../../schemas/workbench/v1/setting-keys-v1.json', import.meta.url),
			'utf8',
		),
	);
	expect(SETTING_KEY_TABLE).toEqual(generated);
});
