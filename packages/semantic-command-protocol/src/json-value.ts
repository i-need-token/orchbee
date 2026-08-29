import { z } from 'zod';

export type SemanticCommandJsonValue =
	| boolean
	| null
	| number
	| string
	| readonly SemanticCommandJsonValue[]
	| { readonly [key: string]: SemanticCommandJsonValue };

export const SemanticCommandJsonValueSchema: z.ZodType<SemanticCommandJsonValue> = z.lazy(() =>
	z.union([
		z.null(),
		z.boolean(),
		z.number().finite(),
		z.string(),
		z.array(SemanticCommandJsonValueSchema),
		z.record(z.string(), SemanticCommandJsonValueSchema),
	]),
);
