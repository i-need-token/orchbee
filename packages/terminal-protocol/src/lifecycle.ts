import { z } from 'zod';

import {
	AttachmentIdSchema,
	ClientIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from './identifiers';
import type { ClientId } from './identifiers';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';

export const HostPhase = {
	absent: 'absent',
	draining: 'draining',
	serving: 'serving',
	starting: 'starting',
} as const;

export const TerminalPhase = {
	exited: 'exited',
	finishing: 'finishing',
	running: 'running',
	starting: 'starting',
} as const;

export const TerminalPhaseSchema = z.enum(TerminalPhase);

export const LifecycleAuthority = {
	attachment: 'attachment',
	desktop: 'desktop',
} as const;

export const LifecycleOutcome = {
	applied: 'applied',
	idempotent: 'idempotent',
	rejected: 'rejected',
} as const;

export const LifecycleRejection = {
	attachmentConflict: 'attachment-conflict',
	authorityDenied: 'authority-denied',
	creationConflict: 'creation-conflict',
	duplicateIdentity: 'duplicate-identity',
	frameConflict: 'frame-conflict',
	hostNotServing: 'host-not-serving',
	inputBackpressure: 'input-backpressure',
	invalidPhase: 'invalid-phase',
	missingConnection: 'missing-connection',
	missingResource: 'missing-resource',
	resizeLeaseDenied: 'resize-lease-denied',
	staleEpoch: 'stale-epoch',
	upgradePending: 'upgrade-pending',
} as const;

export type HostPhase = (typeof HostPhase)[keyof typeof HostPhase];
export type TerminalPhase = (typeof TerminalPhase)[keyof typeof TerminalPhase];
export type LifecycleAuthority = (typeof LifecycleAuthority)[keyof typeof LifecycleAuthority];
export type LifecycleOutcome = (typeof LifecycleOutcome)[keyof typeof LifecycleOutcome];
export type LifecycleRejection = (typeof LifecycleRejection)[keyof typeof LifecycleRejection];

const safeRevisionSchema = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const positiveSafeIntegerSchema = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER);
const launchDigestSchema = z.string().regex(/^[0-9a-f]{64}$/);
const gridDimensionSchema = z.number().int().min(1).max(4096);

const InFlightFrameSchema = z.strictObject({
	baseRevision: safeRevisionSchema.nullable(),
	kind: z.enum(['delta', 'snapshot']),
	revision: safeRevisionSchema,
	sequence: positiveSafeIntegerSchema,
});

const TerminalLifecycleSchema = z.strictObject({
	childExited: z.boolean(),
	columns: gridDimensionSchema,
	creationId: CreationIdSchema,
	forceTerminateRequested: z.boolean(),
	launchDigest: launchDigestSchema,
	pendingInputBytes: z.number().int().min(0).max(PROTOCOL_LIMITS.maxPendingInputBytes),
	phase: TerminalPhaseSchema,
	ptyDrained: z.boolean(),
	revision: safeRevisionSchema,
	rows: gridDimensionSchema,
	terminalId: TerminalIdSchema,
	terminateRequested: z.boolean(),
});

const AttachmentLifecycleSchema = z.strictObject({
	acknowledgedRevision: safeRevisionSchema.nullable(),
	attachmentId: AttachmentIdSchema,
	canInput: z.boolean(),
	canResize: z.boolean(),
	clientId: ClientIdSchema,
	epoch: HostEpochSchema,
	forceSnapshot: z.boolean(),
	inFlightFrame: InFlightFrameSchema.nullable(),
	nextSequence: positiveSafeIntegerSchema,
	pendingDirty: z.boolean(),
	resizeLease: z.boolean(),
	terminalId: TerminalIdSchema,
});

export const ProtocolLifecycleStateSchema = z
	.strictObject({
		attachments: z.array(AttachmentLifecycleSchema),
		connections: z.array(ClientIdSchema),
		epoch: HostEpochSchema.nullable(),
		networkListener: z.literal(false),
		phase: z.enum(HostPhase),
		terminals: z.array(TerminalLifecycleSchema),
		upgradePending: z.boolean(),
	})
	.superRefine((state, context) => {
		if ((state.phase === HostPhase.absent) !== (state.epoch === null)) {
			addLifecycleIssue(context, ['epoch'], 'Only an absent Host may have no epoch');
		}
		if (
			state.phase === HostPhase.absent &&
			(state.terminals.length > 0 || state.attachments.length > 0 || state.connections.length > 0)
		) {
			addLifecycleIssue(context, [], 'An absent Host cannot retain protocol-owned resources');
		}
		if (state.phase === HostPhase.draining && state.terminals.length > 0) {
			addLifecycleIssue(context, ['phase'], 'A Host with retained terminals cannot drain');
		}

		validateUnique(state.connections, 'ClientId', context, ['connections']);
		validateUnique(
			state.terminals.map((terminal) => terminal.creationId),
			'CreationId',
			context,
			['terminals'],
		);
		validateUnique(
			state.terminals.map((terminal) => terminal.terminalId),
			'TerminalId',
			context,
			['terminals'],
		);
		validateUnique(
			state.attachments.map((attachment) => attachment.attachmentId),
			'AttachmentId',
			context,
			['attachments'],
		);

		for (const [index, terminal] of state.terminals.entries()) {
			if (terminal.forceTerminateRequested && !terminal.terminateRequested) {
				addLifecycleIssue(
					context,
					['terminals', index, 'forceTerminateRequested'],
					'Force termination requires a prior graceful termination request',
				);
			}
			const completionCount = Number(terminal.childExited) + Number(terminal.ptyDrained);
			if (
				(terminal.phase === TerminalPhase.starting || terminal.phase === TerminalPhase.running) &&
				completionCount !== 0
			) {
				addLifecycleIssue(
					context,
					['terminals', index, 'phase'],
					'Starting and running terminals cannot have completion facts',
				);
			}
			if (terminal.phase === TerminalPhase.finishing && completionCount !== 1) {
				addLifecycleIssue(
					context,
					['terminals', index, 'phase'],
					'A finishing terminal requires exactly one completion fact',
				);
			}
			if (terminal.phase === TerminalPhase.exited && completionCount !== 2) {
				addLifecycleIssue(
					context,
					['terminals', index, 'phase'],
					'An exited terminal requires both completion facts',
				);
			}
		}

		const resizeLeaseTerminals = new Set<string>();
		for (const [index, attachment] of state.attachments.entries()) {
			const terminal = state.terminals.find(
				(candidate) => candidate.terminalId === attachment.terminalId,
			);
			if (!terminal || !state.connections.includes(attachment.clientId)) {
				addLifecycleIssue(
					context,
					['attachments', index],
					'Attachment must reference a retained terminal and connected client',
				);
				continue;
			}
			if (attachment.epoch !== state.epoch) {
				addLifecycleIssue(
					context,
					['attachments', index, 'epoch'],
					'Attachment epoch must equal the active Host epoch',
				);
			}
			if (attachment.resizeLease) {
				if (!attachment.canResize || resizeLeaseTerminals.has(attachment.terminalId)) {
					addLifecycleIssue(
						context,
						['attachments', index, 'resizeLease'],
						'Resize lease requires capability and must be exclusive per terminal',
					);
				}
				resizeLeaseTerminals.add(attachment.terminalId);
			}
			if (
				attachment.acknowledgedRevision !== null &&
				attachment.acknowledgedRevision > terminal.revision
			) {
				addLifecycleIssue(
					context,
					['attachments', index, 'acknowledgedRevision'],
					'Attachment cannot acknowledge a future terminal revision',
				);
			}
			const frame = attachment.inFlightFrame;
			if (frame) {
				if (frame.revision > terminal.revision || frame.sequence >= attachment.nextSequence) {
					addLifecycleIssue(
						context,
						['attachments', index, 'inFlightFrame'],
						'In-flight frame must reference an emitted current-or-earlier revision and sequence',
					);
				}
				if (
					(frame.kind === 'snapshot' && frame.baseRevision !== null) ||
					(frame.kind === 'delta' &&
						(frame.baseRevision === null ||
							frame.baseRevision !== attachment.acknowledgedRevision ||
							frame.baseRevision >= frame.revision))
				) {
					addLifecycleIssue(
						context,
						['attachments', index, 'inFlightFrame'],
						'In-flight frame base must refine the attachment acknowledgement',
					);
				}
			}
		}
	});

const epochEvent = { epoch: HostEpochSchema } as const;
const clientEvent = { clientId: ClientIdSchema, epoch: HostEpochSchema } as const;
const desktopAuthority = { authority: z.enum(LifecycleAuthority) } as const;

export const ProtocolLifecycleEventSchema = z.discriminatedUnion('type', [
	z.strictObject({ epoch: HostEpochSchema, type: z.literal('launch-host') }),
	z.strictObject({ ...epochEvent, type: z.literal('endpoint-ready') }),
	z.strictObject({ ...clientEvent, type: z.literal('connect-client') }),
	z.strictObject({ ...clientEvent, type: z.literal('disconnect-client') }),
	z.strictObject({
		...clientEvent,
		...desktopAuthority,
		creationId: CreationIdSchema,
		launchDigest: launchDigestSchema,
		terminalId: TerminalIdSchema,
		type: z.literal('create-terminal'),
	}),
	z.strictObject({
		...epochEvent,
		terminalId: TerminalIdSchema,
		type: z.literal('spawn-succeeded'),
	}),
	z.strictObject({ ...epochEvent, terminalId: TerminalIdSchema, type: z.literal('spawn-failed') }),
	z.strictObject({
		...clientEvent,
		...desktopAuthority,
		terminalId: TerminalIdSchema,
		type: z.literal('terminate-terminal'),
	}),
	z.strictObject({
		...clientEvent,
		...desktopAuthority,
		terminalId: TerminalIdSchema,
		type: z.literal('force-terminate-terminal'),
	}),
	z.strictObject({ ...epochEvent, terminalId: TerminalIdSchema, type: z.literal('child-exited') }),
	z.strictObject({ ...epochEvent, terminalId: TerminalIdSchema, type: z.literal('pty-drained') }),
	z.strictObject({
		...clientEvent,
		...desktopAuthority,
		terminalId: TerminalIdSchema,
		type: z.literal('dispose-terminal'),
	}),
	z.strictObject({
		...clientEvent,
		...desktopAuthority,
		attachmentId: AttachmentIdSchema,
		canInput: z.boolean(),
		canResize: z.boolean(),
		requestResizeLease: z.boolean(),
		terminalId: TerminalIdSchema,
		type: z.literal('attach-terminal'),
	}),
	z.strictObject({
		...clientEvent,
		attachmentId: AttachmentIdSchema,
		type: z.literal('detach-attachment'),
	}),
	z.strictObject({
		...epochEvent,
		terminalId: TerminalIdSchema,
		type: z.literal('terminal-output'),
	}),
	z.strictObject({
		...clientEvent,
		attachmentId: AttachmentIdSchema,
		type: z.literal('emit-frame'),
	}),
	z.strictObject({
		...clientEvent,
		attachmentId: AttachmentIdSchema,
		revision: safeRevisionSchema,
		sequence: positiveSafeIntegerSchema,
		type: z.literal('frame-applied'),
	}),
	z.strictObject({
		...clientEvent,
		attachmentId: AttachmentIdSchema,
		type: z.literal('need-snapshot'),
	}),
	z.strictObject({
		...clientEvent,
		attachmentId: AttachmentIdSchema,
		columns: gridDimensionSchema,
		rows: gridDimensionSchema,
		type: z.literal('resize-terminal'),
	}),
	z.strictObject({
		...clientEvent,
		attachmentId: AttachmentIdSchema,
		bytes: z.number().int().min(1).max(PROTOCOL_LIMITS.maxInputBytes),
		type: z.literal('accept-input'),
	}),
	z.strictObject({
		...epochEvent,
		bytes: z.number().int().min(1).max(PROTOCOL_LIMITS.maxPendingInputBytes),
		terminalId: TerminalIdSchema,
		type: z.literal('input-written'),
	}),
	z.strictObject({ ...epochEvent, type: z.literal('request-upgrade') }),
	z.strictObject({ ...epochEvent, type: z.literal('stop-host') }),
]);

export type ProtocolLifecycleState = z.infer<typeof ProtocolLifecycleStateSchema>;
export type ProtocolLifecycleEvent = z.infer<typeof ProtocolLifecycleEventSchema>;
export type TerminalLifecycle = z.infer<typeof TerminalLifecycleSchema>;
export type AttachmentLifecycle = z.infer<typeof AttachmentLifecycleSchema>;

export type LifecycleCommand =
	| { readonly attachmentId: string; readonly type: 'emit-frame' }
	| { readonly bytes: number; readonly terminalId: string; readonly type: 'write-input' }
	| {
			readonly columns: number;
			readonly rows: number;
			readonly terminalId: string;
			readonly type: 'resize-pty';
	  }
	| { readonly terminalId: string; readonly type: 'spawn-terminal' }
	| { readonly terminalId: string; readonly type: 'terminate-child' }
	| { readonly terminalId: string; readonly type: 'force-terminate-child' }
	| { readonly type: 'close-connections' }
	| { readonly type: 'stop-host' };

export interface LifecycleTransition {
	readonly commands: readonly LifecycleCommand[];
	readonly outcome: LifecycleOutcome;
	readonly rejection: LifecycleRejection | null;
	readonly state: ProtocolLifecycleState;
}

export function createInitialProtocolLifecycleState(): ProtocolLifecycleState {
	return {
		attachments: [],
		connections: [],
		epoch: null,
		networkListener: false,
		phase: HostPhase.absent,
		terminals: [],
		upgradePending: false,
	};
}

function addLifecycleIssue(
	context: z.RefinementCtx,
	path: readonly PropertyKey[],
	message: string,
): void {
	context.addIssue({ code: 'custom', message, path: [...path] });
}

function validateUnique(
	values: readonly string[],
	name: string,
	context: z.RefinementCtx,
	path: readonly PropertyKey[],
): void {
	if (new Set(values).size !== values.length) {
		addLifecycleIssue(context, path, `${name} values must be unique`);
	}
}

function applied(
	state: ProtocolLifecycleState,
	commands: readonly LifecycleCommand[] = [],
): LifecycleTransition {
	return { commands, outcome: LifecycleOutcome.applied, rejection: null, state };
}

function idempotent(state: ProtocolLifecycleState): LifecycleTransition {
	return { commands: [], outcome: LifecycleOutcome.idempotent, rejection: null, state };
}

function rejected(
	state: ProtocolLifecycleState,
	rejection: LifecycleRejection,
): LifecycleTransition {
	return { commands: [], outcome: LifecycleOutcome.rejected, rejection, state };
}

function eventHasCurrentEpoch(
	state: ProtocolLifecycleState,
	event: ProtocolLifecycleEvent,
): boolean {
	return event.type === 'launch-host' || event.epoch === state.epoch;
}

function clientIsConnected(state: ProtocolLifecycleState, clientId: ClientId): boolean {
	return state.connections.includes(clientId);
}

function findTerminal(
	state: ProtocolLifecycleState,
	terminalId: string,
): TerminalLifecycle | undefined {
	return state.terminals.find((terminal) => terminal.terminalId === terminalId);
}

function findAttachment(
	state: ProtocolLifecycleState,
	attachmentId: string,
): AttachmentLifecycle | undefined {
	return state.attachments.find((attachment) => attachment.attachmentId === attachmentId);
}

function attachmentBelongsToClient(
	attachment: AttachmentLifecycle | undefined,
	clientId: string,
): attachment is AttachmentLifecycle {
	return attachment?.clientId === clientId;
}

function markTerminalDirty(
	attachments: ProtocolLifecycleState['attachments'],
	terminalId: string,
): ProtocolLifecycleState['attachments'] {
	return attachments.map((attachment) =>
		attachment.terminalId === terminalId ? { ...attachment, pendingDirty: true } : attachment,
	);
}

function transitionCurrentState(
	state: ProtocolLifecycleState,
	event: ProtocolLifecycleEvent,
): LifecycleTransition {
	switch (event.type) {
		case 'launch-host': {
			if (state.phase !== HostPhase.absent) return idempotent(state);
			return applied({ ...state, epoch: event.epoch, phase: HostPhase.starting });
		}
		case 'endpoint-ready': {
			if (state.phase !== HostPhase.starting)
				return rejected(state, LifecycleRejection.invalidPhase);
			return applied({ ...state, phase: HostPhase.serving });
		}
		case 'connect-client': {
			if (state.phase !== HostPhase.serving) {
				return rejected(state, LifecycleRejection.hostNotServing);
			}
			if (clientIsConnected(state, event.clientId)) return idempotent(state);
			return applied({ ...state, connections: [...state.connections, event.clientId] });
		}
		case 'disconnect-client': {
			if (!clientIsConnected(state, event.clientId)) return idempotent(state);
			const connections = state.connections.filter((clientId) => clientId !== event.clientId);
			if (
				connections.length === 0 &&
				state.terminals.length === 0 &&
				state.phase === HostPhase.serving &&
				!state.networkListener
			) {
				return applied(createInitialProtocolLifecycleState(), [{ type: 'stop-host' }]);
			}
			return applied({
				...state,
				attachments: state.attachments.filter(
					(attachment) => attachment.clientId !== event.clientId,
				),
				connections,
			});
		}
		case 'create-terminal': {
			if (event.authority !== LifecycleAuthority.desktop) {
				return rejected(state, LifecycleRejection.authorityDenied);
			}
			if (state.phase !== HostPhase.serving) {
				return rejected(state, LifecycleRejection.hostNotServing);
			}
			if (state.upgradePending) return rejected(state, LifecycleRejection.upgradePending);
			if (!clientIsConnected(state, event.clientId)) {
				return rejected(state, LifecycleRejection.missingConnection);
			}
			const existing = state.terminals.find((terminal) => terminal.creationId === event.creationId);
			if (existing) {
				return existing.launchDigest === event.launchDigest
					? idempotent(state)
					: rejected(state, LifecycleRejection.creationConflict);
			}
			if (findTerminal(state, event.terminalId)) {
				return rejected(state, LifecycleRejection.duplicateIdentity);
			}
			const terminal: TerminalLifecycle = {
				childExited: false,
				columns: 80,
				creationId: event.creationId,
				forceTerminateRequested: false,
				launchDigest: event.launchDigest,
				pendingInputBytes: 0,
				phase: TerminalPhase.starting,
				ptyDrained: false,
				revision: 0,
				rows: 24,
				terminalId: event.terminalId,
				terminateRequested: false,
			};
			return applied({ ...state, terminals: [...state.terminals, terminal] }, [
				{ terminalId: event.terminalId, type: 'spawn-terminal' },
			]);
		}
		case 'spawn-succeeded': {
			const terminal = findTerminal(state, event.terminalId);
			if (!terminal) return rejected(state, LifecycleRejection.missingResource);
			if (terminal.phase === TerminalPhase.running) return idempotent(state);
			if (terminal.phase !== TerminalPhase.starting) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			return applied({
				...state,
				terminals: state.terminals.map((candidate) =>
					candidate.terminalId === event.terminalId
						? { ...candidate, phase: TerminalPhase.running }
						: candidate,
				),
			});
		}
		case 'spawn-failed': {
			const terminal = findTerminal(state, event.terminalId);
			if (!terminal) return idempotent(state);
			if (terminal.phase !== TerminalPhase.starting) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			return applied({
				...state,
				attachments: state.attachments.filter(
					(attachment) => attachment.terminalId !== event.terminalId,
				),
				terminals: state.terminals.filter((candidate) => candidate.terminalId !== event.terminalId),
			});
		}
		case 'terminate-terminal': {
			if (event.authority !== LifecycleAuthority.desktop) {
				return rejected(state, LifecycleRejection.authorityDenied);
			}
			if (!clientIsConnected(state, event.clientId)) {
				return rejected(state, LifecycleRejection.missingConnection);
			}
			const terminal = findTerminal(state, event.terminalId);
			if (!terminal) return rejected(state, LifecycleRejection.missingResource);
			if (terminal.terminateRequested) return idempotent(state);
			if (terminal.phase !== TerminalPhase.running) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			return applied(
				{
					...state,
					terminals: state.terminals.map((candidate) =>
						candidate.terminalId === event.terminalId
							? { ...candidate, terminateRequested: true }
							: candidate,
					),
				},
				[{ terminalId: event.terminalId, type: 'terminate-child' }],
			);
		}
		case 'force-terminate-terminal': {
			if (event.authority !== LifecycleAuthority.desktop) {
				return rejected(state, LifecycleRejection.authorityDenied);
			}
			if (!clientIsConnected(state, event.clientId)) {
				return rejected(state, LifecycleRejection.missingConnection);
			}
			const terminal = findTerminal(state, event.terminalId);
			if (!terminal) return rejected(state, LifecycleRejection.missingResource);
			if (terminal.forceTerminateRequested || terminal.phase === TerminalPhase.exited) {
				return idempotent(state);
			}
			if (
				!terminal.terminateRequested ||
				(terminal.phase !== TerminalPhase.running && terminal.phase !== TerminalPhase.finishing)
			) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			return applied(
				{
					...state,
					terminals: state.terminals.map((candidate) =>
						candidate.terminalId === event.terminalId
							? { ...candidate, forceTerminateRequested: true }
							: candidate,
					),
				},
				[{ terminalId: event.terminalId, type: 'force-terminate-child' }],
			);
		}
		case 'child-exited':
			return recordTerminalCompletion(state, event.terminalId, 'child');
		case 'pty-drained':
			return recordTerminalCompletion(state, event.terminalId, 'pty');
		case 'dispose-terminal': {
			if (event.authority !== LifecycleAuthority.desktop) {
				return rejected(state, LifecycleRejection.authorityDenied);
			}
			if (!clientIsConnected(state, event.clientId)) {
				return rejected(state, LifecycleRejection.missingConnection);
			}
			const terminal = findTerminal(state, event.terminalId);
			if (!terminal) return idempotent(state);
			if (terminal.phase !== TerminalPhase.exited) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			const terminals = state.terminals.filter(
				(candidate) => candidate.terminalId !== event.terminalId,
			);
			const shouldDrain = state.upgradePending && terminals.length === 0;
			const commands: LifecycleCommand[] = shouldDrain ? [{ type: 'close-connections' }] : [];
			return applied(
				{
					...state,
					attachments: state.attachments.filter(
						(attachment) => attachment.terminalId !== event.terminalId,
					),
					connections: shouldDrain ? [] : state.connections,
					phase: shouldDrain ? HostPhase.draining : state.phase,
					terminals,
				},
				commands,
			);
		}
		case 'attach-terminal': {
			if (event.authority !== LifecycleAuthority.desktop) {
				return rejected(state, LifecycleRejection.authorityDenied);
			}
			if (state.phase !== HostPhase.serving) {
				return rejected(state, LifecycleRejection.hostNotServing);
			}
			if (!clientIsConnected(state, event.clientId)) {
				return rejected(state, LifecycleRejection.missingConnection);
			}
			const terminal = findTerminal(state, event.terminalId);
			if (!terminal || terminal.phase === TerminalPhase.starting) {
				return rejected(state, LifecycleRejection.missingResource);
			}
			if (findAttachment(state, event.attachmentId)) {
				return rejected(state, LifecycleRejection.attachmentConflict);
			}
			if (
				event.requestResizeLease &&
				(!event.canResize ||
					state.attachments.some(
						(attachment) => attachment.terminalId === event.terminalId && attachment.resizeLease,
					))
			) {
				return rejected(state, LifecycleRejection.resizeLeaseDenied);
			}
			const attachment: AttachmentLifecycle = {
				acknowledgedRevision: null,
				attachmentId: event.attachmentId,
				canInput: event.canInput,
				canResize: event.canResize,
				clientId: event.clientId,
				epoch: event.epoch,
				forceSnapshot: true,
				inFlightFrame: null,
				nextSequence: 1,
				pendingDirty: true,
				resizeLease: event.requestResizeLease,
				terminalId: event.terminalId,
			};
			return applied({ ...state, attachments: [...state.attachments, attachment] });
		}
		case 'detach-attachment': {
			const attachment = findAttachment(state, event.attachmentId);
			if (!attachmentBelongsToClient(attachment, event.clientId)) return idempotent(state);
			return applied({
				...state,
				attachments: state.attachments.filter(
					(candidate) => candidate.attachmentId !== event.attachmentId,
				),
			});
		}
		case 'terminal-output': {
			const terminal = findTerminal(state, event.terminalId);
			if (
				!terminal ||
				terminal.ptyDrained ||
				(terminal.phase !== TerminalPhase.running && terminal.phase !== TerminalPhase.finishing)
			) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			if (terminal.revision === Number.MAX_SAFE_INTEGER) {
				return rejected(state, LifecycleRejection.frameConflict);
			}
			return applied({
				...state,
				attachments: markTerminalDirty(state.attachments, event.terminalId),
				terminals: state.terminals.map((candidate) =>
					candidate.terminalId === event.terminalId
						? { ...candidate, revision: candidate.revision + 1 }
						: candidate,
				),
			});
		}
		case 'emit-frame': {
			const attachment = findAttachment(state, event.attachmentId);
			if (!attachmentBelongsToClient(attachment, event.clientId)) {
				return rejected(state, LifecycleRejection.missingResource);
			}
			if (attachment.inFlightFrame || !attachment.pendingDirty) return idempotent(state);
			if (attachment.nextSequence === Number.MAX_SAFE_INTEGER) {
				return rejected(state, LifecycleRejection.frameConflict);
			}
			const terminal = findTerminal(state, attachment.terminalId);
			if (!terminal) return rejected(state, LifecycleRejection.missingResource);
			const snapshot = attachment.forceSnapshot || attachment.acknowledgedRevision === null;
			const frame = {
				baseRevision: snapshot ? null : attachment.acknowledgedRevision,
				kind: snapshot ? ('snapshot' as const) : ('delta' as const),
				revision: terminal.revision,
				sequence: attachment.nextSequence,
			};
			return applied(
				{
					...state,
					attachments: state.attachments.map((candidate) =>
						candidate.attachmentId === event.attachmentId
							? {
									...candidate,
									forceSnapshot: false,
									inFlightFrame: frame,
									nextSequence: candidate.nextSequence + 1,
									pendingDirty: false,
								}
							: candidate,
					),
				},
				[{ attachmentId: event.attachmentId, type: 'emit-frame' }],
			);
		}
		case 'frame-applied': {
			const attachment = findAttachment(state, event.attachmentId);
			if (!attachmentBelongsToClient(attachment, event.clientId)) {
				return rejected(state, LifecycleRejection.missingResource);
			}
			const frame = attachment.inFlightFrame;
			if (!frame || frame.sequence !== event.sequence || frame.revision !== event.revision) {
				return rejected(state, LifecycleRejection.frameConflict);
			}
			return applied({
				...state,
				attachments: state.attachments.map((candidate) =>
					candidate.attachmentId === event.attachmentId
						? {
								...candidate,
								acknowledgedRevision: event.revision,
								inFlightFrame: null,
							}
						: candidate,
				),
			});
		}
		case 'need-snapshot': {
			const attachment = findAttachment(state, event.attachmentId);
			if (!attachmentBelongsToClient(attachment, event.clientId)) {
				return rejected(state, LifecycleRejection.missingResource);
			}
			return applied({
				...state,
				attachments: state.attachments.map((candidate) =>
					candidate.attachmentId === event.attachmentId
						? {
								...candidate,
								forceSnapshot: true,
								inFlightFrame: null,
								pendingDirty: true,
							}
						: candidate,
				),
			});
		}
		case 'resize-terminal': {
			const attachment = findAttachment(state, event.attachmentId);
			if (
				!attachmentBelongsToClient(attachment, event.clientId) ||
				!attachment.canResize ||
				!attachment.resizeLease
			) {
				return rejected(state, LifecycleRejection.resizeLeaseDenied);
			}
			const terminal = findTerminal(state, attachment.terminalId);
			if (!terminal || terminal.phase !== TerminalPhase.running) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			if (terminal.columns === event.columns && terminal.rows === event.rows)
				return idempotent(state);
			if (terminal.revision === Number.MAX_SAFE_INTEGER) {
				return rejected(state, LifecycleRejection.frameConflict);
			}
			return applied(
				{
					...state,
					attachments: markTerminalDirty(state.attachments, terminal.terminalId),
					terminals: state.terminals.map((candidate) =>
						candidate.terminalId === terminal.terminalId
							? {
									...candidate,
									columns: event.columns,
									revision: candidate.revision + 1,
									rows: event.rows,
								}
							: candidate,
					),
				},
				[
					{
						columns: event.columns,
						rows: event.rows,
						terminalId: terminal.terminalId,
						type: 'resize-pty',
					},
				],
			);
		}
		case 'accept-input': {
			const attachment = findAttachment(state, event.attachmentId);
			if (!attachmentBelongsToClient(attachment, event.clientId) || !attachment.canInput) {
				return rejected(state, LifecycleRejection.authorityDenied);
			}
			const terminal = findTerminal(state, attachment.terminalId);
			if (!terminal || terminal.phase !== TerminalPhase.running) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			if (terminal.pendingInputBytes + event.bytes > PROTOCOL_LIMITS.maxPendingInputBytes) {
				return rejected(state, LifecycleRejection.inputBackpressure);
			}
			return applied(
				{
					...state,
					terminals: state.terminals.map((candidate) =>
						candidate.terminalId === terminal.terminalId
							? {
									...candidate,
									pendingInputBytes: candidate.pendingInputBytes + event.bytes,
								}
							: candidate,
					),
				},
				[{ bytes: event.bytes, terminalId: terminal.terminalId, type: 'write-input' }],
			);
		}
		case 'input-written': {
			const terminal = findTerminal(state, event.terminalId);
			if (!terminal) return rejected(state, LifecycleRejection.missingResource);
			if (event.bytes > terminal.pendingInputBytes) {
				return rejected(state, LifecycleRejection.inputBackpressure);
			}
			return applied({
				...state,
				terminals: state.terminals.map((candidate) =>
					candidate.terminalId === event.terminalId
						? { ...candidate, pendingInputBytes: candidate.pendingInputBytes - event.bytes }
						: candidate,
				),
			});
		}
		case 'request-upgrade': {
			if (state.phase !== HostPhase.serving)
				return rejected(state, LifecycleRejection.invalidPhase);
			if (state.upgradePending) return idempotent(state);
			if (state.terminals.length > 0) return applied({ ...state, upgradePending: true });
			return applied(
				{
					...state,
					attachments: [],
					connections: [],
					phase: HostPhase.draining,
					upgradePending: true,
				},
				[{ type: 'close-connections' }],
			);
		}
		case 'stop-host': {
			if (
				state.phase !== HostPhase.draining ||
				state.terminals.length > 0 ||
				state.connections.length > 0
			) {
				return rejected(state, LifecycleRejection.invalidPhase);
			}
			return applied(createInitialProtocolLifecycleState(), [{ type: 'stop-host' }]);
		}
	}
}

function recordTerminalCompletion(
	state: ProtocolLifecycleState,
	terminalId: string,
	completion: 'child' | 'pty',
): LifecycleTransition {
	const terminal = findTerminal(state, terminalId);
	if (!terminal) return rejected(state, LifecycleRejection.missingResource);
	if (terminal.phase !== TerminalPhase.running && terminal.phase !== TerminalPhase.finishing) {
		return terminal.phase === TerminalPhase.exited
			? idempotent(state)
			: rejected(state, LifecycleRejection.invalidPhase);
	}
	if (
		(completion === 'child' && terminal.childExited) ||
		(completion === 'pty' && terminal.ptyDrained)
	) {
		return idempotent(state);
	}
	const childExited = completion === 'child' || terminal.childExited;
	const ptyDrained = completion === 'pty' || terminal.ptyDrained;
	return applied({
		...state,
		terminals: state.terminals.map((candidate) =>
			candidate.terminalId === terminalId
				? {
						...candidate,
						childExited,
						phase: childExited && ptyDrained ? TerminalPhase.exited : TerminalPhase.finishing,
						ptyDrained,
					}
				: candidate,
		),
	});
}

export function transitionProtocolLifecycle(
	stateValue: unknown,
	eventValue: unknown,
): ProtocolResult<LifecycleTransition> {
	const state = ProtocolLifecycleStateSchema.safeParse(stateValue);
	if (!state.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Lifecycle state failed validation: ${state.error.issues[0]?.message ?? 'unknown error'}`,
		);
	}
	const event = ProtocolLifecycleEventSchema.safeParse(eventValue);
	if (!event.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Lifecycle event failed validation: ${event.error.issues[0]?.message ?? 'unknown error'}`,
		);
	}
	if (!eventHasCurrentEpoch(state.data, event.data)) {
		return protocolSuccess(rejected(state.data, LifecycleRejection.staleEpoch));
	}

	const transition = transitionCurrentState(state.data, event.data);
	const validatedNextState = ProtocolLifecycleStateSchema.safeParse(transition.state);
	if (!validatedNextState.success) {
		throw new Error(
			`Lifecycle transition violated an invariant: ${validatedNextState.error.issues[0]?.message ?? 'unknown error'}`,
		);
	}
	return protocolSuccess({ ...transition, state: validatedNextState.data });
}
