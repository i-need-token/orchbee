import type { DesktopSessionRefusal } from '@orchbee/desktop-client-protocol';
import { z } from 'zod';

/** A capability name of the handshake: 1 to 64 lowercase letters, digits, and hyphens. */
const CapabilityNameSchema = z.string().regex(/^[a-z0-9-]{1,64}$/u);

/**
 * `negotiatedCapabilities` names what both sides listed in the `client-server` handshake; a Client
 * that predates it answers none.
 */
const ServerStartDocumentSchema = z.looseObject({
	result: z.looseObject({
		negotiatedCapabilities: z.array(CapabilityNameSchema).max(256).optional(),
		restoreNotice: z.enum(['recovered', 'empty']).nullable(),
	}),
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

type ServerRestoreState =
	| { readonly kind: 'intact' }
	| { readonly kind: 'lost-or-unknown' }
	| { readonly kind: 'refused'; readonly refusal: DesktopSessionRefusal };

/**
 * What one `server start` said: how the Session's restore went, and the capabilities its handshake
 * negotiated, which only a successful start names.
 */
export interface ServerStartAnswer {
	readonly negotiatedCapabilities: readonly string[];
	readonly restore: ServerRestoreState;
}

const LOST_OR_UNKNOWN: ServerStartAnswer = {
	negotiatedCapabilities: [],
	restore: { kind: 'lost-or-unknown' },
};

/**
 * Reads `server start --format json`. A refusal is named only when the answer says why, because
 * only then can the window say what would help; any other failure, and an answer that cannot be
 * read, counts as a restore that may have lost everything.
 */
export function interpretServerStartAnswer(stdout: string, failed: boolean): ServerStartAnswer {
	let document: unknown;
	try {
		document = JSON.parse(stdout);
	} catch {
		return LOST_OR_UNKNOWN;
	}
	if (!failed) {
		const answer = ServerStartDocumentSchema.safeParse(document);
		if (!answer.success) return LOST_OR_UNKNOWN;
		const { negotiatedCapabilities = [], restoreNotice } = answer.data.result;
		return {
			negotiatedCapabilities,
			restore: restoreNotice === 'empty' ? { kind: 'lost-or-unknown' } : { kind: 'intact' },
		};
	}
	return { negotiatedCapabilities: [], restore: interpretRefusal(document) };
}

function interpretRefusal(document: unknown): ServerRestoreState {
	if (RecoveryRequiredFailureSchema.safeParse(document).success) {
		return { kind: 'refused', refusal: { reason: 'recovery-required' } };
	}
	const incompatible = IncompatibleFailureSchema.safeParse(document);
	if (!incompatible.success) return { kind: 'lost-or-unknown' };
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
