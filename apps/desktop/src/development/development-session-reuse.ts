import { z } from 'zod';

const COHORT_DIRECTORY_PATTERN = /^cohort-[0-9a-f]{16}$/u;
const DigestSchema = z.string().regex(/^[0-9a-f]{64}$/u);

/**
 * The executables a running Session runs. A Client only joins it, admitted by protocol fingerprint,
 * and the Agent observation Capsule stays the one beside the Session's own Worker.
 */
const SessionRolesSchema = z.looseObject({
	artifacts: z.looseObject({
		'orchbee-server': DigestSchema,
		'orchbee-terminal-host': DigestSchema,
		'orchbee-worker': DigestSchema,
	}),
	schemaVersion: z.literal(1),
});

/**
 * What `target/desktop-development/<session>.session.json` records about a development Session, by
 * staged cohort directory name.
 */
export const DevelopmentSessionMarkerSchema = z.strictObject({
	/** The cohort that started the Session; its Client stops it, and cleanup keeps it. */
	cohort: z.string().regex(COHORT_DIRECTORY_PATTERN),
	/** The cohort whose Client the latest `pnpm desktop` used, which starts a stopped Session. */
	client: z.string().regex(COHORT_DIRECTORY_PATTERN).optional(),
	/** The Server's process ID when `cohort` was last known to have started it. */
	pid: z.int().positive().optional(),
});

export type DevelopmentSessionMarker = z.infer<typeof DevelopmentSessionMarkerSchema>;

/** What `session list` said about the Session just before this start. */
export type DevelopmentSessionObservation =
	| { readonly kind: 'running'; readonly pid: number }
	| { readonly kind: 'stopped' }
	| { readonly kind: 'unknown' };

/**
 * Whether the development Session that the cohort with the `previous` `cohort.json` started can keep
 * running for the newly staged cohort with the `current` one. It can only when both manifests read
 * and name the same Server, Worker, and Terminal Host; anything else stops it, as a Server, Worker,
 * or Host change takes effect only after a Session stop.
 */
export function keepsDevelopmentSession(previous: unknown, current: unknown): boolean {
	const before = SessionRolesSchema.safeParse(previous);
	const after = SessionRolesSchema.safeParse(current);
	if (!before.success || !after.success) return false;
	const roles = ['orchbee-server', 'orchbee-worker', 'orchbee-terminal-host'] as const;
	return roles.every((role) => before.data.artifacts[role] === after.data.artifacts[role]);
}

/**
 * The marker once a kept Session is handed to the `current` cohort's Client. The Session runs its
 * Server, Worker, and Host from the directory of the cohort that started it, which cleanup must keep.
 * A Server with the process ID last recorded is still the marker's; another one was started since by
 * the Client the latest run used, and a stopped Session will be started by the current one. When
 * nothing says, the marker's cohort stays.
 */
export function keepDevelopmentSession(
	marker: DevelopmentSessionMarker,
	current: string,
	observation: DevelopmentSessionObservation,
): DevelopmentSessionMarker {
	switch (observation.kind) {
		case 'stopped':
			return { client: current, cohort: current };
		case 'running':
			return {
				client: current,
				cohort: observation.pid === marker.pid ? marker.cohort : (marker.client ?? marker.cohort),
				pid: observation.pid,
			};
		case 'unknown':
			return {
				client: current,
				cohort: marker.cohort,
				...(marker.pid === undefined ? {} : { pid: marker.pid }),
			};
	}
}
