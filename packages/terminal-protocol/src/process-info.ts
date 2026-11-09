import { z } from 'zod';

import { CanonicalUint64TextSchema } from './control';
import { containsUnpairedSurrogate } from './containsUnpairedSurrogate';
import { AttachmentIdSchema, TerminalIdSchema } from './identifiers';

const MAXIMUM_PROCESS_SAMPLE_ROWS = 256;
const MAXIMUM_PROCESS_TEXT_UTF16_UNITS = 4096;

const pid = z.number().int().min(1).max(0xffff_ffff);
const text = z
	.string()
	.max(MAXIMUM_PROCESS_TEXT_UTF16_UNITS)
	.refine(
		(value) => !containsUnpairedSurrogate(value) && !value.includes('\0'),
		'Invalid process text',
	);

/** How the Host launched the Terminal root, from its own launch record. */
export const TerminalLaunchKindSchema = z.enum(['managed-powershell', 'cmd', 'other']);

/** Only `includeLaunchKind: true` adds `launchKind` to the response. */
export const ReadProcessInfoSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	includeLaunchKind: z.boolean().optional(),
});
export const TerminalProcessInfoSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	terminalId: TerminalIdSchema,
	kind: z.literal('windows-job'),
	launchKind: TerminalLaunchKindSchema.optional(),
	rootPid: pid,
	processes: z
		.array(
			// [pid, parentPid, creationTime, console, image, commandLine, currentDirectory]
			z.tuple([
				pid,
				pid.nullable(),
				CanonicalUint64TextSchema.refine(
					(value) => value !== '0',
					'Creation time must be positive',
				),
				z.enum(['attached', 'detached', 'none', 'unknown']),
				text.nullable(),
				text.nullable(),
				text.nullable(),
			]),
		)
		.max(MAXIMUM_PROCESS_SAMPLE_ROWS)
		.refine(
			(rows) => rows.every((row, index) => index === 0 || row[0] > (rows[index - 1]?.[0] ?? 0)),
			'Process PIDs must be strictly increasing',
		),
});
