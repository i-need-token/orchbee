import type { ActionInputValidation } from '@orchbee/actions';
import type { z } from 'zod';

/** An Action input checked by a schema, which turns away anything else with `code`. */
export function scmActionInput<Input>(
	name: string,
	code: string,
	schema: z.ZodType<Input>,
): ActionInputValidation<Input> {
	return {
		name,
		parse: (input) => {
			const parsed = schema.safeParse(input);
			return parsed.success ? { data: parsed.data, success: true } : { code, success: false };
		},
	};
}
