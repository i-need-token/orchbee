import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
} from './workbench-semantic-contract';

/** The Server keeps at most this many earlier layouts. */
const WORKBENCH_MAXIMUM_LAYOUT_HISTORY_ENTRIES = 48;
/** Save times are Unix milliseconds that every JSON reader holds exactly. */
const SavedAtSchema = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER);
const CountSchema = z.number().int().min(0).max(4_294_967_295);

const LayoutHistoryActionSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('list') }).readonly(),
	z.strictObject({ kind: z.literal('reopen'), savedAt: SavedAtSchema }).readonly(),
]);
export const WorkbenchLayoutHistoryRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.layout-history'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: LayoutHistoryActionSchema,
	})
	.readonly();

const LayoutHistoryEntrySchema = z
	.strictObject({
		savedAt: SavedAtSchema,
		workspaces: CountSchema,
		tabs: CountSchema,
		workspacesNotOpen: CountSchema,
		tabsNotOpen: CountSchema,
	})
	.refine(
		(entry) => entry.workspacesNotOpen <= entry.workspaces && entry.tabsNotOpen <= entry.tabs,
		{
			message: 'An entry counts more items not open than it holds',
		},
	)
	.readonly();
const LayoutHistoryOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('listed'),
			entries: z
				.array(LayoutHistoryEntrySchema)
				.max(WORKBENCH_MAXIMUM_LAYOUT_HISTORY_ENTRIES)
				.refine(
					(entries) =>
						entries.every(
							(entry, index) => index === 0 || entry.savedAt < (entries[index - 1]?.savedAt ?? 0),
						),
					{ message: 'Entries are not newest first' },
				)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('reopened'),
			revision: WorkbenchRevisionSchema,
			workspaces: CountSchema,
			tabs: CountSchema,
			focus: z
				.strictObject({
					terminalContextId: WorkbenchTerminalContextIdSchema,
					tabId: WorkbenchTabIdSchema,
				})
				.readonly()
				.nullable(),
		})
		.refine(
			(outcome) =>
				(outcome.workspaces > 0 || outcome.tabs > 0) &&
				(outcome.focus !== null || outcome.tabs === 0),
			{ message: 'A reopen names what it opened and the first reopened Tab' },
		)
		.readonly(),
	z.strictObject({ kind: z.enum(['nothing-to-reopen', 'not-found', 'denied']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('rejected'),
			reason: z.enum(['session-mismatch', 'resource-limit']),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: z.enum(['service-unavailable', 'busy']),
		})
		.readonly(),
]);
export const WorkbenchLayoutHistoryResultSchema = z
	.strictObject({
		kind: z.literal('workbench.layout-history.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		outcome: LayoutHistoryOutcomeSchema,
	})
	.readonly();
