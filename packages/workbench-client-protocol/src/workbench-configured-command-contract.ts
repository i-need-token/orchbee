import { z } from 'zod';
import {
	ConfiguredCommandIdSchema,
	ConfiguredCommandDescriptorSchema,
	ConfiguredCommandPopupSourceSchema,
	ConfiguredCommandPopupSpecSchema,
	ConfiguredCommandSelectionSchema,
} from './workbench-configured-command-spec-contract';
import {
	WorkbenchClientIdSchema,
	ClientViewSequenceSchema,
	ClientViewOutcomeSchema,
	WorkbenchClientViewResultSchema,
} from './workbench-client-view-contract';
import { PluginInvocationContextSchema } from './workbench-plugin-context-contract';
import { PluginResultSchema } from './workbench-plugin-contract';
import { WorkbenchCommandOutcomeSchema } from './workbench-command-outcome-contract';
import { WorkbenchTerminalGeometrySchema } from './workbench-terminal-launch-contract';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkerIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_CONFIGURED_COMMAND_BYTES = 256 * 1024;
const encoder = new TextEncoder();
const bounded = (value: unknown) =>
	encoder.encode(JSON.stringify(value)).byteLength <= WORKBENCH_MAXIMUM_CONFIGURED_COMMAND_BYTES;
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
};
export const ConfiguredCommandInvocationSchema = z
	.strictObject({
		commandId: ConfiguredCommandIdSchema,
		expectedRevision: WorkbenchRevisionSchema,
		source: z
			.strictObject({
				clientId: WorkbenchClientIdSchema,
				viewRevision: ClientViewSequenceSchema,
				presentation: ConfiguredCommandPopupSourceSchema,
			})
			.nullable(),
		selection: ConfiguredCommandSelectionSchema.nullable(),
		context: PluginInvocationContextSchema,
	})
	.refine(
		(value) =>
			(value.selection === null || value.source !== null) &&
			(value.source === null ||
				(value.context.tabId === value.source.presentation.tabId &&
					value.context.focusedPaneId === value.source.presentation.panelId)),
	)
	.readonly();
const popup = { popupId: WorkbenchRequestIdSchema };
export const ConfiguredCommandRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.configured-command'),
		...correlation,
		action: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('list') }),
			z.strictObject({ kind: z.literal('invoke'), invocation: ConfiguredCommandInvocationSchema }),
			z.strictObject({
				kind: z.literal('popup-open'),
				...popup,
				spec: ConfiguredCommandPopupSpecSchema,
				geometry: WorkbenchTerminalGeometrySchema,
			}),
			z.strictObject({ kind: z.literal('popup-read'), ...popup }),
			z.strictObject({ kind: z.literal('popup-close'), ...popup }),
		]),
	})
	.refine(bounded)
	.readonly();
const worker = z.strictObject({
	workerId: WorkerIdSchema,
	generation: WorkbenchRevisionSchema.refine((value) => value !== '0'),
});
const command = { commandId: ConfiguredCommandIdSchema };
const clientResult = z
	.strictObject({ ...correlation, outcome: ClientViewOutcomeSchema })
	.refine(
		(value) =>
			WorkbenchClientViewResultSchema.safeParse({ kind: 'workbench.client-view.result', ...value })
				.success,
	);
export const ConfiguredCommandResultSchema = z
	.strictObject({
		kind: z.literal('workbench.configured-command.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({
				kind: z.literal('catalog'),
				revision: WorkbenchRevisionSchema,
				entries: z
					.array(ConfiguredCommandDescriptorSchema)
					.max(64)
					.refine((entries) => new Set(entries.map((entry) => entry.id)).size === entries.length),
			}),
			z.strictObject({
				kind: z.literal('shell-started'),
				...command,
				worker,
				jobId: WorkbenchRequestIdSchema,
				existing: z.boolean(),
			}),
			z.strictObject({
				kind: z.literal('pane'),
				...command,
				result: z.strictObject({
					protocolVersion: correlation.protocolVersion,
					requestId: WorkbenchRequestIdSchema,
					outcome: WorkbenchCommandOutcomeSchema,
				}),
			}),
			z.strictObject({
				kind: z.literal('plugin'),
				...command,
				result: z.strictObject({
					...correlation,
					outcome: PluginResultSchema.unwrap().shape.outcome,
				}),
			}),
			z.strictObject({ kind: z.literal('popup-control'), ...command, result: clientResult }),
			z.strictObject({
				kind: z.literal('popup-opened'),
				...popup,
				title: z
					.string()
					.min(1)
					.refine(
						(value) =>
							!value.includes('\0') &&
							!/[\uD800-\uDFFF]/u.test(value) &&
							encoder.encode(value).byteLength <= 4096,
					),
			}),
			z.strictObject({ kind: z.literal('popup-state'), ...popup, settled: z.boolean() }),
			z.strictObject({ kind: z.literal('popup-closing'), ...popup }),
			z.strictObject({ kind: z.literal('popup-closed'), ...popup }),
			z.strictObject({ kind: z.literal('popup-unknown'), ...popup }),
			z.strictObject({
				kind: z.literal('failed'),
				reason: z.enum([
					'denied',
					'invalid-intent',
					'site-mismatch',
					'unavailable',
					'busy',
					'conflict',
					'not-found',
					'stale-source',
					'physical-unknown',
				]),
			}),
		]),
	})
	.refine((value) => {
		const outcome = value.outcome;
		if (outcome.kind === 'shell-started') return outcome.jobId === value.requestId;
		if (outcome.kind === 'pane')
			return (
				outcome.result.requestId === value.requestId &&
				[
					'panel-split-pending',
					'panel-split-created',
					'rejected',
					'denied',
					'unavailable',
				].includes(outcome.result.outcome.kind)
			);
		if (outcome.kind === 'plugin')
			return (
				outcome.result.requestId === value.requestId &&
				outcome.result.siteId === value.siteId &&
				['started', 'failed', 'unknown'].includes(outcome.result.outcome.kind)
			);
		if (outcome.kind === 'popup-control') {
			if (outcome.result.requestId !== value.requestId || outcome.result.siteId !== value.siteId)
				return false;
			const nested = outcome.result.outcome;
			if (nested.kind !== 'operation')
				return ['not-found', 'conflict', 'denied', 'unavailable'].includes(nested.kind);
			return (
				nested.operation.command.operationId === value.requestId &&
				nested.operation.command.intent.kind === 'command-popup-open' &&
				nested.operation.command.intent.spec.commandId === outcome.commandId
			);
		}
		return true;
	})
	.refine(bounded)
	.readonly();
export type ConfiguredCommandInvocation = z.infer<typeof ConfiguredCommandInvocationSchema>;
export type ConfiguredCommandRequest = z.infer<typeof ConfiguredCommandRequestSchema>;
export type ConfiguredCommandResult = z.infer<typeof ConfiguredCommandResultSchema>;
