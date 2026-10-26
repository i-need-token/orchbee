import { z } from 'zod';
import {
	SessionIdSchema,
	TerminalIdSchema,
	WorkbenchPanelIdSchema,
	WorkbenchRevisionSchema,
} from './workbench-semantic-contract';

export const ClientViewportPanelsSchema = z
	.array(WorkbenchPanelIdSchema)
	.min(1)
	.max(16)
	.refine((ids) => new Set(ids).size === ids.length)
	.readonly();
const scroll = z
	.strictObject({
		offsetFromBottom: WorkbenchRevisionSchema,
		maxOffsetFromBottom: WorkbenchRevisionSchema,
		viewportRows: z.number().int().min(1).max(4096),
	})
	.readonly();
const source = z
	.strictObject({
		hostEpoch: SessionIdSchema,
		attachmentId: SessionIdSchema,
		generation: WorkbenchRevisionSchema.refine((value) => value !== '0'),
		revision: WorkbenchRevisionSchema,
		columns: z.number().int().min(1).max(4096),
		alternateScreen: z.boolean(),
		totalRows: WorkbenchRevisionSchema,
		startRow: WorkbenchRevisionSchema,
	})
	.readonly();
export const ClientViewportObservationSchema = z
	.discriminatedUnion('state', [
		z
			.strictObject({
				state: z.literal('unavailable'),
				reason: z.enum(['not-attached', 'frame-unavailable']),
			})
			.readonly(),
		z
			.strictObject({
				state: z.literal('pending'),
				phase: z.enum(['entering', 'awaiting-window', 'recovering']),
			})
			.readonly(),
		z
			.strictObject({
				state: z.literal('observed'),
				mode: z.enum(['live', 'history']),
				source,
				scroll,
			})
			.readonly(),
	])
	.superRefine((value, context) => {
		if (value.state !== 'observed') return;
		const total = BigInt(value.source.totalRows),
			rows = BigInt(value.scroll.viewportRows);
		const maximum = total > rows ? total - rows : 0n,
			start = BigInt(value.source.startRow);
		if (
			start > total ||
			BigInt(value.scroll.maxOffsetFromBottom) !== maximum ||
			BigInt(value.scroll.offsetFromBottom) !== (maximum > start ? maximum - start : 0n) ||
			(value.mode === 'live' && start !== maximum) ||
			(value.source.alternateScreen && (value.mode !== 'live' || maximum !== 0n))
		) {
			context.addIssue({ code: 'custom', message: 'Inconsistent Client viewport metrics' });
		}
	})
	.readonly();
export const ClientViewportSnapshotSchema = z
	.strictObject({
		workbenchRevision: WorkbenchRevisionSchema,
		panels: z
			.array(
				z
					.strictObject({
						panelId: WorkbenchPanelIdSchema,
						primaryTerminalId: TerminalIdSchema.nullable(),
						observation: ClientViewportObservationSchema,
					})
					.readonly(),
			)
			.min(1)
			.max(16)
			.readonly(),
	})
	.superRefine((value, context) => {
		if (
			new Set(value.panels.map((row) => row.panelId)).size !== value.panels.length ||
			value.panels.some(
				(row) =>
					row.primaryTerminalId === null &&
					!(row.observation.state === 'unavailable' && row.observation.reason === 'not-attached'),
			) ||
			new TextEncoder().encode(JSON.stringify(value)).byteLength > 12 * 1024
		) {
			context.addIssue({ code: 'custom', message: 'Invalid Client viewport snapshot' });
		}
	})
	.readonly();
