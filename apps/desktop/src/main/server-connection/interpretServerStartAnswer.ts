import {
	DesktopBlockingProcessSchema,
	DesktopSystemErrorKindSchema,
	type DesktopSessionRefusal,
} from '@orchbee/desktop-client-protocol';
import { z } from 'zod';

import { interpretPathRefusal } from './interpretPathRefusal';

/** A capability name of the handshake: 1 to 64 lowercase letters, digits, and hyphens. */
const CapabilityNameSchema = z.string().regex(/^[a-z0-9-]{1,64}$/u);

const ProductVersionSchema = z.string().regex(/^[!-~]{1,64}$/u);

/**
 * `negotiatedCapabilities` names what both sides listed in the `client-server` handshake, and
 * `sessionVersion` the version the Session's Server named in it; a Client that predates either
 * answers none.
 */
const ServerStartDocumentSchema = z.looseObject({
	result: z.looseObject({
		negotiatedCapabilities: z.array(CapabilityNameSchema).max(256).optional(),
		restoreNotice: z.enum(['recovered', 'empty']).nullable(),
		sessionVersion: ProductVersionSchema.nullable().optional().catch(null),
	}),
});

/**
 * The refusal `server start` answers until a Session it cannot prove stopped is recovered. It names
 * the process that blocks recovery when it found one; a process it cannot read keeps the refusal.
 */
const RecoveryRequiredFailureSchema = z.looseObject({
	failure: z.looseObject({
		details: z.looseObject({
			blockingProcess: z
				.looseObject(DesktopBlockingProcessSchema.shape)
				.optional()
				.catch(undefined),
			reason: z.literal('recovery-required'),
		}),
		kind: z.literal('rejected'),
	}),
});

/** The refusal `server start` answers when the system would not create the Server, with its answer. */
const ServerNotStartedFailureSchema = z.looseObject({
	failure: z.looseObject({
		details: z.looseObject({
			ioKind: DesktopSystemErrorKindSchema.catch('other'),
			osError: z.number().int().nullable().catch(null),
			reason: z.literal('server-not-started'),
		}),
		kind: z.literal('rejected'),
	}),
});

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
 * negotiated and the version its Server named, which only a successful start names; a refusal by
 * a Session too old names that version in the refusal instead.
 */
export interface ServerStartAnswer {
	readonly negotiatedCapabilities: readonly string[];
	readonly restore: ServerRestoreState;
	readonly sessionVersion: string | null;
}

const LOST_OR_UNKNOWN: ServerStartAnswer = {
	negotiatedCapabilities: [],
	restore: { kind: 'lost-or-unknown' },
	sessionVersion: null,
};

/**
 * Reads `server start --format json`. A refusal is named only when the answer says why, because
 * only then can the window say what would help; any other failure, and an answer that cannot be
 * read, counts as a restore that may have lost everything.
 */
export function interpretServerStartAnswer(
	stdout: string,
	failed: boolean,
	platform: NodeJS.Platform,
): ServerStartAnswer {
	let document: unknown;
	try {
		document = JSON.parse(stdout);
	} catch {
		return LOST_OR_UNKNOWN;
	}
	if (!failed) {
		const answer = ServerStartDocumentSchema.safeParse(document);
		if (!answer.success) return LOST_OR_UNKNOWN;
		const {
			negotiatedCapabilities = [],
			restoreNotice,
			sessionVersion = null,
		} = answer.data.result;
		return {
			negotiatedCapabilities,
			restore: restoreNotice === 'empty' ? { kind: 'lost-or-unknown' } : { kind: 'intact' },
			sessionVersion,
		};
	}
	return {
		negotiatedCapabilities: [],
		restore: interpretRefusal(document, platform),
		sessionVersion: null,
	};
}

function interpretRefusal(document: unknown, platform: NodeJS.Platform): ServerRestoreState {
	const refusedPath = interpretPathRefusal(document, platform);
	if (refusedPath !== undefined) return { kind: 'refused', refusal: refusedPath };
	const recovery = RecoveryRequiredFailureSchema.safeParse(document);
	if (recovery.success) {
		const blocking = recovery.data.failure.details.blockingProcess;
		return {
			kind: 'refused',
			refusal:
				blocking === undefined
					? { reason: 'recovery-required' }
					: {
							blockingProcess: {
								processId: blocking.processId,
								program: blocking.program,
								role: blocking.role,
							},
							reason: 'recovery-required',
						},
		};
	}
	const notStarted = ServerNotStartedFailureSchema.safeParse(document);
	if (notStarted.success) {
		const { ioKind, osError } = notStarted.data.failure.details;
		return { kind: 'refused', refusal: { ioKind, osError, reason: 'server-not-started' } };
	}
	// Any other reason, including one a newer build names, leaves the window its general sentence.
	const incompatible = IncompatibleFailureSchema.safeParse(document);
	if (!incompatible.success) return { kind: 'lost-or-unknown' };
	const details = incompatible.data.failure.details;
	switch (details.reason) {
		case 'client-too-old':
			return {
				kind: 'refused',
				refusal: {
					desktopVersion: details.clientVersion,
					reason: 'desktop-too-old',
					sessionVersion: details.sessionVersion ?? null,
				},
			};
		case 'session-too-old':
			return {
				kind: 'refused',
				refusal: {
					desktopVersion: details.clientVersion,
					reason: 'session-too-old',
					// Only Windows and macOS restart a Session's Server while its terminals run.
					restartable: platform === 'win32' || platform === 'darwin',
					sessionVersion: details.sessionVersion ?? null,
				},
			};
		case 'environment-mismatch':
			return { kind: 'refused', refusal: { reason: 'other-environment' } };
		case 'fingerprint-mismatch':
			return { kind: 'refused', refusal: { reason: 'development-build-mismatch' } };
	}
}
