import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	DEFAULT_USER_PREFERENCES,
	USER_PREFERENCE_KEYS,
	UserPreferencesPatchSchema,
	UserPreferencesResponseSchema,
} from '../user-preferences-contract';

const caseSchema = z.strictObject({
	name: z.string(),
	direction: z.enum(['request', 'response', 'values']),
	document: z.unknown(),
	boundary: z.enum(['schema', 'semantic']).optional(),
});
const corpusSchema = z.strictObject({
	contractVersion: z.literal(1),
	defaults: z.unknown(),
	valid: z.array(caseSchema),
	invalid: z.array(caseSchema),
});

describe('shared device preference contract', () => {
	it('admits the same defaults, boundaries and semantic failures as the Rust binding', async () => {
		const corpus = corpusSchema.parse(
			JSON.parse(
				await readFile(
					new URL(
						'../../../../schemas/workbench/v1/user-preferences-conformance-v1.json',
						import.meta.url,
					),
					'utf8',
				),
			),
		);
		const schema = z
			.record(z.string(), z.unknown())
			.parse(
				JSON.parse(
					await readFile(
						new URL(
							'../../../../schemas/workbench/v1/user-preferences.schema.json',
							import.meta.url,
						),
						'utf8',
					),
				),
			);
		const validate = new Ajv2020({ allErrors: true, strict: true }).compile(schema);
		expect(DEFAULT_USER_PREFERENCES).toEqual(corpus.defaults);
		const schemas = {
			response: UserPreferencesResponseSchema,
			values: UserPreferencesResponseSchema.options[0].shape.snapshot.shape.values,
		};
		for (const fixture of corpus.valid) {
			expect({ name: fixture.name, schema: validate(fixture.document) }).toEqual({
				name: fixture.name,
				schema: true,
			});
			// Preference requests cross no TypeScript boundary; only the Rust binding admits them.
			if (fixture.direction === 'request') continue;
			expect({
				name: fixture.name,
				typed: schemas[fixture.direction].safeParse(fixture.document).success,
			}).toEqual({ name: fixture.name, typed: true });
		}
		for (const fixture of corpus.invalid) {
			if (fixture.direction === 'request') continue;
			expect({
				name: fixture.name,
				typed: schemas[fixture.direction].safeParse(fixture.document).success,
			}).toEqual({ name: fixture.name, typed: false });
		}
		for (const fixture of corpus.invalid.filter((candidate) => candidate.boundary === 'schema')) {
			expect({ name: fixture.name, schema: validate(fixture.document) }).toEqual({
				name: fixture.name,
				schema: false,
			});
		}
	});
	it('keeps the editable inventory complete and null resets distinct from omitted fields', () => {
		expect(
			Object.keys(
				UserPreferencesResponseSchema.options[0].shape.snapshot.shape.values.shape,
			).sort(),
		).toEqual([...USER_PREFERENCE_KEYS].sort());
		expect(Object.keys(UserPreferencesPatchSchema.shape).sort()).toEqual(
			[...USER_PREFERENCE_KEYS].sort(),
		);
		expect(UserPreferencesPatchSchema.parse({ soundEnabled: null })).toEqual({
			soundEnabled: null,
		});
	});
});
