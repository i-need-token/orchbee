import { z } from 'zod';

const terminalGeometryDimensionSchema = z.number().int().min(1).max(4096);
const terminalCellPixelDimensionSchema = z.number().int().min(0).max(4096);

export const WorkbenchTerminalGeometrySchema = z
	.strictObject({
		columns: terminalGeometryDimensionSchema,
		rows: terminalGeometryDimensionSchema,
		cellPixelWidth: terminalCellPixelDimensionSchema,
		cellPixelHeight: terminalCellPixelDimensionSchema,
	})
	.superRefine((geometry, context) => {
		if ((geometry.cellPixelWidth === 0) !== (geometry.cellPixelHeight === 0)) {
			context.addIssue({
				code: 'custom',
				message:
					'Workbench terminal cell pixel width and height must be both positive or both zero (pixels unknown)',
				path: ['cellPixelWidth'],
			});
		}
		if (geometry.columns * geometry.rows > 262_144) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal grid exceeds 262144 cells',
				path: ['columns'],
			});
		}
		if (geometry.columns * geometry.cellPixelWidth > 65_535) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal pixel width exceeds 65535',
				path: ['cellPixelWidth'],
			});
		}
		if (geometry.rows * geometry.cellPixelHeight > 65_535) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal pixel height exceeds 65535',
				path: ['cellPixelHeight'],
			});
		}
	})
	.readonly();

export const WorkbenchTerminalUnavailableReasonSchema = z.enum([
	'terminal-runtime-unavailable',
	'server-busy',
]);
export const WorkbenchTerminalFailureReasonSchema = z.enum(['terminal-create-failed']);
export const WorkbenchTerminalOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.literal('started'), geometry: WorkbenchTerminalGeometrySchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: WorkbenchTerminalUnavailableReasonSchema,
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('failed'), reason: WorkbenchTerminalFailureReasonSchema })
		.readonly(),
	z.strictObject({ kind: z.literal('outcome-unknown') }).readonly(),
]);
