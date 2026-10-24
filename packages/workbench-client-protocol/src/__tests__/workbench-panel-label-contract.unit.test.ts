import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { expect, it } from 'vitest';
import { z } from 'zod';
import { WorkbenchPresentationSchema } from '../workbench-semantic-contract';

it('keeps fallback text and admits exactly canonical stored Panel overrides', async () => {
	const schema = z
		.record(z.string(), z.unknown())
		.parse(
			JSON.parse(
				await readFile(
					new URL('../../../../schemas/workbench/v1/workbench.schema.json', import.meta.url),
					'utf8',
				),
			),
		);
	const ajv = new Ajv2020({ strict: true });
	ajv.addFormat(
		'orchbee-uint64',
		(value: string) =>
			/^(?:0|[1-9][0-9]{0,19})$/u.test(value) && BigInt(value) <= 18_446_744_073_709_551_615n,
	);
	ajv.addSchema(schema);
	const validate = ajv.compile({ $ref: 'urn:orchbee:schema:workbench:v1#/$defs/presentation' });
	const base = {
		customLabel: null,
		label: 'creation fallback',
		binding: null,
		presentationId: '1'.repeat(32),
	};
	expect(WorkbenchPresentationSchema.parse(base)).toEqual(base);
	expect(validate(base)).toBe(true);
	for (const customLabel of ['标题', '🐝'.repeat(512), '\ufefftitle', '\u200btitle', 'a\nb']) {
		const document = { ...base, customLabel };
		expect(WorkbenchPresentationSchema.parse(document)).toEqual(document);
		expect(validate(document)).toBe(true);
	}
	for (const customLabel of [
		undefined,
		'',
		' x',
		'x\u0085',
		'x\n',
		'x\r',
		'\u00a0x',
		'🐝'.repeat(513),
		1,
		[],
	]) {
		const document = { ...base, customLabel };
		expect(WorkbenchPresentationSchema.safeParse(document).success).toBe(false);
		expect(validate(document)).toBe(false);
	}
});
