import {
	DesktopPathRefusalCauseSchema,
	type DesktopSessionRefusal,
} from '@orchbee/desktop-client-protocol';
import { z } from 'zod';

/**
 * A folder or file the Client refused, named in a failure's `refusal` detail with the rule it
 * breaks: the answer of `server start` and of the window-folder helper alike.
 */
const PathRefusedFailureSchema = z.looseObject({
	failure: z.looseObject({
		details: z.looseObject({
			refusal: z.looseObject({
				path: z.string().min(1).max(32_767),
				reason: DesktopPathRefusalCauseSchema,
			}),
		}),
		kind: z.literal('check-failed'),
	}),
});

export type DesktopPathRefusal = Extract<DesktopSessionRefusal, { reason: 'path-refused' }>;

/**
 * The refused item a failure document names, under the rules of the system this Main runs on,
 * where the Client also runs; undefined when the document names none this window can read.
 */
export function interpretPathRefusal(
	document: unknown,
	platform: NodeJS.Platform,
): DesktopPathRefusal | undefined {
	const answer = PathRefusedFailureSchema.safeParse(document);
	if (!answer.success) return undefined;
	const { path, reason } = answer.data.failure.details.refusal;
	return {
		cause: reason,
		path,
		reason: 'path-refused',
		system: platform === 'win32' ? 'windows' : 'posix',
	};
}
