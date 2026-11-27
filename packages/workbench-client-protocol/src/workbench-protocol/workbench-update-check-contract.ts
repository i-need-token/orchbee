import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
} from './workbench-semantic-contract';

/**
 * An admitted Client asks the Session Server to run its automatic-update job now (`start`), or
 * reads where that job stands (`read`). Both answer at once; a run's end is read, never awaited.
 * `phases`, sent only to a Session whose handshake lists `update-phases`, asks for the running
 * run's download and install and the phase a failed run ended in.
 */
export const WorkbenchUpdateCheckRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.update-check'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: z.enum(['start', 'read']),
		phases: z.boolean().optional(),
	})
	.readonly();

/** A release version, as a handshake's `productVersion`. */
const releaseVersion = z.string().regex(/^[!-~]{1,64}$/u);
const byteCount = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);

const UpdateCheckOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['checking', 'idle', 'unavailable', 'denied']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('downloading'),
			version: releaseVersion,
			receivedBytes: byteCount,
			totalBytes: byteCount.min(1),
		})
		.refine((outcome) => outcome.receivedBytes <= outcome.totalBytes)
		.readonly(),
	z.strictObject({ kind: z.literal('installing'), version: releaseVersion }).readonly(),
	z
		.strictObject({
			kind: z.literal('finished'),
			result: z.enum([
				'up-to-date',
				'installed',
				'failed',
				'check-failed',
				'download-failed',
				'install-failed',
			]),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('rejected'), reason: z.literal('session-mismatch') }).readonly(),
]);
export const WorkbenchUpdateCheckResultSchema = z
	.strictObject({
		kind: z.literal('workbench.update-check.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		outcome: UpdateCheckOutcomeSchema,
	})
	.readonly();
