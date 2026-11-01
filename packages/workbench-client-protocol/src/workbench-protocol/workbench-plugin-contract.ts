import { PluginInvocationContextSchema } from './workbench-plugin-context-contract';
import { PluginPopupSpecSchema } from './workbench-plugin-popup-contract';
import {
	WorkbenchTerminalGeometrySchema,
	WorkbenchPluginPaneSelectorSchema,
} from './workbench-terminal-launch-contract';
import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchLabelSchema,
	WorkbenchPanelIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	TerminalPresentationIdSchema,
	WorkbenchPresentationBindingSchema,
	WorkerIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

const bytes = (value: string) => new TextEncoder().encode(value).length;
const text = (maximum: number, nul = false) =>
	z
		.string()
		.refine(
			(value) =>
				!/[\uD800-\uDFFF]/u.test(value) &&
				(nul || !value.includes('\0')) &&
				bytes(value) <= maximum,
		);
const identifier = (maximum: number) =>
	z
		.string()
		.min(1)
		.max(maximum)
		.regex(/^[A-Za-z0-9._:-]+$/u);
const jobId = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((value) => value !== '0'.repeat(32));
const worker = z.strictObject({
	workerId: WorkerIdSchema,
	generation: WorkbenchRevisionSchema.refine((value) => value !== '0'),
});
const positive = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER);
const timestamp = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const cursor = z.strictObject({ worker, before: positive });
const PluginLinkCatalogGuardSchema = z.strictObject({
	revision: timestamp,
	fingerprint: z.string().regex(/^[0-9a-f]{64}$/u),
});
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
};
const failure = z.enum([
	'exit-failure',
	'spawn-unavailable',
	'spawn-failed',
	'io',
	'cancelled',
	'physical-unknown',
]);
const PluginCommandLogSchema = z
	.strictObject({
		jobId,
		sequence: positive,
		pluginId: identifier(120),
		commandId: identifier(241),
		command: z
			.array(text(4096).refine((value) => value.length > 0))
			.min(1)
			.max(128)
			.refine((values) => values.reduce((sum, value) => sum + bytes(value), 0) <= 131072),
		startedMs: timestamp,
		finishedMs: timestamp.nullable(),
		exitCode: z.number().int().min(-2147483648).max(2147483647).nullable(),
		failure: failure.nullable(),
		stdout: text(196608, true),
		stderr: text(196608, true),
		stdoutTruncated: z.boolean(),
		stderrTruncated: z.boolean(),
	})
	.refine((value) => {
		if (value.finishedMs === null)
			return (
				value.exitCode === null &&
				value.failure === null &&
				value.stdout === '' &&
				value.stderr === '' &&
				!value.stdoutTruncated &&
				!value.stderrTruncated
			);
		if (value.finishedMs < value.startedMs) return false;
		if (value.failure === null) return value.exitCode === 0;
		if (value.failure === 'exit-failure') return value.exitCode !== 0;
		return value.exitCode === null;
	});
const paneInfo = z.strictObject({
	selector: WorkbenchPluginPaneSelectorSchema,
	terminalContextId: WorkbenchTerminalContextIdSchema,
	tabId: WorkbenchTabIdSchema,
	panelId: WorkbenchPanelIdSchema,
	presentationId: TerminalPresentationIdSchema,
	binding: WorkbenchPresentationBindingSchema,
});
export const PluginRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.plugin'),
		...correlation,
		action: z.discriminatedUnion('kind', [
			z.strictObject({
				kind: z.literal('popup-open'),
				popupId: WorkbenchRequestIdSchema,
				spec: PluginPopupSpecSchema,
				geometry: WorkbenchTerminalGeometrySchema,
			}),
			z.strictObject({ kind: z.literal('popup-read'), popupId: WorkbenchRequestIdSchema }),
			z.strictObject({ kind: z.literal('popup-close'), popupId: WorkbenchRequestIdSchema }),
			z.strictObject({ kind: z.literal('pane-get'), panelId: WorkbenchPanelIdSchema }),
			z.strictObject({ kind: z.literal('pane-observe'), pane: paneInfo }),
			z.strictObject({
				kind: z.literal('invoke'),
				operationId: jobId,
				pluginId: identifier(120).nullable(),
				actionId: identifier(241),
				context: PluginInvocationContextSchema,
				linkCatalog: PluginLinkCatalogGuardSchema.optional(),
			}),
			z.strictObject({
				kind: z.literal('logs'),
				pluginId: identifier(120).nullable(),
				cursor: cursor.nullable(),
				limit: z.number().int().min(1).max(4),
			}),
			z.strictObject({ kind: z.literal('get'), worker, jobId }),
			z.strictObject({ kind: z.literal('cancel'), worker, jobId }),
		]),
	})
	.superRefine((request, context) => {
		if (request.action.kind !== 'invoke' || request.action.linkCatalog === undefined) return;
		if (
			request.action.pluginId === null ||
			request.action.context.invocationSource !== 'link_click' ||
			request.action.context.clickedUrl === undefined ||
			request.action.context.clickedUrl === null ||
			request.action.context.linkHandlerId === undefined ||
			request.action.context.linkHandlerId === null
		)
			context.addIssue({ code: 'custom', message: 'catalog guard requires link click context' });
	})
	.readonly();
export const PluginResultSchema = z
	.strictObject({
		kind: z.literal('workbench.plugin.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({
				kind: z.literal('popup-opened'),
				popupId: WorkbenchRequestIdSchema,
				title: WorkbenchLabelSchema,
			}),
			z.strictObject({
				kind: z.literal('popup-state'),
				popupId: WorkbenchRequestIdSchema,
				settled: z.boolean(),
			}),
			z.strictObject({ kind: z.literal('popup-closed'), popupId: WorkbenchRequestIdSchema }),
			z.strictObject({ kind: z.literal('popup-closing'), popupId: WorkbenchRequestIdSchema }),
			z.strictObject({ kind: z.literal('popup-unknown'), popupId: WorkbenchRequestIdSchema }),
			z.strictObject({
				kind: z.literal('pane-observed'),
				revision: WorkbenchRevisionSchema.refine((value) => value !== '0'),
				pane: paneInfo,
				settled: z.boolean(),
			}),
			z.strictObject({
				kind: z.literal('pane-found'),
				revision: WorkbenchRevisionSchema.refine((value) => value !== '0'),
				pane: z.strictObject({
					selector: WorkbenchPluginPaneSelectorSchema,
					terminalContextId: WorkbenchTerminalContextIdSchema,
					tabId: WorkbenchTabIdSchema,
					panelId: WorkbenchPanelIdSchema,
					presentationId: TerminalPresentationIdSchema,
					binding: WorkbenchPresentationBindingSchema,
				}),
			}),
			z.strictObject({
				kind: z.literal('started'),
				worker,
				log: PluginCommandLogSchema,
				existing: z.boolean(),
			}),
			z
				.strictObject({
					kind: z.literal('logs'),
					worker,
					logs: z.array(PluginCommandLogSchema).max(4),
					next: cursor.nullable(),
				})
				.refine((value) => {
					if (
						value.logs.some(
							(log, index) => index > 0 && (value.logs[index - 1]?.sequence ?? 0) <= log.sequence,
						)
					)
						return false;
					return (
						value.next === null ||
						(value.next.worker.workerId === value.worker.workerId &&
							value.next.worker.generation === value.worker.generation &&
							value.logs.at(-1)?.sequence === value.next.before)
					);
				}),
			z.strictObject({
				kind: z.literal('cancellation'),
				worker,
				jobId,
				alreadyComplete: z.boolean(),
			}),
			z.strictObject({
				kind: z.literal('failed'),
				reason: z.enum([
					'denied',
					'unavailable',
					'session-mismatch',
					'worker-changed',
					'invalid-intent',
					'not-found',
					'disabled',
					'unsupported',
					'source-unavailable',
					'ambiguous',
					'busy',
					'conflict',
					'physical-unknown',
				]),
			}),
			z.strictObject({ kind: z.literal('unknown'), worker }),
		]),
	})
	.readonly();
