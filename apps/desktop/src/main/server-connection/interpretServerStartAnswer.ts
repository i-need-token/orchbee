import type { DesktopSessionRefusal } from '@orchbee/desktop-client-protocol';
import { z } from 'zod';

const ServerStartDocumentSchema = z.looseObject({
	result: z.looseObject({ restoreNotice: z.enum(['recovered', 'empty']).nullable() }),
});

/** The refusal `server start` answers until a Session it cannot prove stopped is recovered. */
const RecoveryRequiredFailureSchema = z.looseObject({
	failure: z.looseObject({
		details: z.looseObject({ reason: z.literal('recovery-required') }),
		kind: z.literal('rejected'),
	}),
});

const ProductVersionSchema = z.string().regex(/^[!-~]{1,64}$/u);

/**
 * A handshake refusal, as the CLI reports it with exit 6. The bundled Client always names its own
 * version beside a version refusal; the Session names its own unless it predates the handshake.
 */
const IncompatibleFailureSchema = z.looseObject({
	failure: z.looseObject({
		details: z.discriminatedUnion('reason', [
			z.looseObject({
				clientVersion: ProductVersionSchema,
				reason: z.enum(['client-too-old', 'session-too-old']),
				sessionVersion: ProductVersionSchema.optional(),
			}),
			z.looseObject({ reason: z.enum(['environment-mismatch', 'fingerprint-mismatch']) }),
		]),
		kind: z.literal('incompatible'),
	}),
});

export type ServerRestoreState =
	| { readonly kind: 'intact' }
	| { readonly kind: 'lost-or-unknown' }
	| { readonly kind: 'refused'; readonly refusal: DesktopSessionRefusal };

const LOST_OR_UNKNOWN: ServerRestoreState = { kind: 'lost-or-unknown' };

/**
 * Reads `server start --format json`. A refusal is named only when the answer says why, because
 * only then can the window say what would help; any other failure, and an answer that cannot be
 * read, counts as a restore that may have lost everything.
 */
export function interpretServerStartAnswer(stdout: string, failed: boolean): ServerRestoreState {
	let document: unknown;
	try {
		document = JSON.parse(stdout);
	} catch {
		return LOST_OR_UNKNOWN;
	}
	if (!failed) {
		const answer = ServerStartDocumentSchema.safeParse(document);
		return answer.success && answer.data.result.restoreNotice !== 'empty'
			? { kind: 'intact' }
			: LOST_OR_UNKNOWN;
	}
	if (RecoveryRequiredFailureSchema.safeParse(document).success) {
		return { kind: 'refused', refusal: { reason: 'recovery-required' } };
	}
	const incompatible = IncompatibleFailureSchema.safeParse(document);
	if (!incompatible.success) return LOST_OR_UNKNOWN;
	const details = incompatible.data.failure.details;
	switch (details.reason) {
		case 'client-too-old':
		case 'session-too-old':
			return {
				kind: 'refused',
				refusal: {
					desktopVersion: details.clientVersion,
					reason: details.reason === 'client-too-old' ? 'desktop-too-old' : 'session-too-old',
					sessionVersion: details.sessionVersion ?? null,
				},
			};
		case 'environment-mismatch':
			return { kind: 'refused', refusal: { reason: 'other-environment' } };
		case 'fingerprint-mismatch':
			return { kind: 'refused', refusal: { reason: 'development-build-mismatch' } };
	}
}
