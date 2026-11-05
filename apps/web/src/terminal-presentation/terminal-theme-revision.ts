import { z } from 'zod';

const MAXIMUM_UINT64_TEXT = '18446744073709551615';
const opaqueIdentitySchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u);
const rgbSchema = z.number().int().min(0).max(0xff_ff_ff);

const TerminalThemeOperationIdSchema = opaqueIdentitySchema.brand<'TerminalThemeOperationId'>();
export type TerminalThemeOperationId = z.infer<typeof TerminalThemeOperationIdSchema>;

const TerminalThemeRevisionSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]{0,19})$/u)
	.refine(
		(value) =>
			value.length < MAXIMUM_UINT64_TEXT.length ||
			(value.length === MAXIMUM_UINT64_TEXT.length && value <= MAXIMUM_UINT64_TEXT),
		'Theme revision exceeds unsigned 64-bit range',
	)
	.brand<'TerminalThemeRevision'>();
type TerminalThemeRevision = z.infer<typeof TerminalThemeRevisionSchema>;

const terminalThemeApplicationSchema = z
	.strictObject({ colorScheme: z.enum(['dark', 'light']) })
	.readonly();
const terminalThemeOverlaySchema = z
	.strictObject({
		cursorAccentRgb: rgbSchema,
		ansiPalette: z.array(rgbSchema).length(16).readonly(),
		searchActiveMatchBackgroundRgb: rgbSchema,
		searchActiveMatchForegroundRgb: rgbSchema,
		searchMatchBackgroundRgb: rgbSchema,
		searchMatchForegroundRgb: rgbSchema,
		selectionBackgroundRgb: rgbSchema,
		selectionForegroundRgb: rgbSchema,
	})
	.readonly();
const terminalThemeHostSnapshotSchema = z
	.strictObject({
		colorScheme: z.enum(['dark', 'light']),
		defaultBackgroundRgb: rgbSchema,
		defaultCursorRgb: rgbSchema.nullable(),
		defaultForegroundRgb: rgbSchema,
		palette: z.array(rgbSchema).length(256).readonly(),
	})
	.readonly();

const TerminalThemeSnapshotSchema = z
	.strictObject({
		application: terminalThemeApplicationSchema,
		overlays: terminalThemeOverlaySchema,
		terminal: terminalThemeHostSnapshotSchema,
	})
	.readonly();
export type TerminalThemeSnapshot = z.infer<typeof TerminalThemeSnapshotSchema>;
type TerminalThemeHostSnapshot = TerminalThemeSnapshot['terminal'];

const TerminalThemeHostStateSchema = z
	.strictObject({
		operationId: TerminalThemeOperationIdSchema.nullable(),
		revision: TerminalThemeRevisionSchema,
		terminal: terminalThemeHostSnapshotSchema,
	})
	.superRefine((state, context) => {
		if ((state.revision === '0') !== (state.operationId === null)) {
			context.addIssue({
				code: 'custom',
				message: 'Revision zero is the only Host theme state without an operation ID',
				path: ['operationId'],
			});
		}
	})
	.readonly();
export type TerminalThemeHostState = z.infer<typeof TerminalThemeHostStateSchema>;

const TerminalThemeFailureSchema = z
	.strictObject({
		code: z.string().min(1).max(128),
		outcome: z.enum(['known-unchanged', 'unknown']),
		retryable: z.boolean(),
	})
	.readonly();
export type TerminalThemeFailure = z.infer<typeof TerminalThemeFailureSchema>;

type TerminalThemePresentationMode = 'active' | 'hidden' | 'suspended';

interface TerminalThemePresentationCommit {
	readonly operationId: TerminalThemeOperationId | null;
	readonly revision: TerminalThemeRevision;
	readonly snapshot: TerminalThemeSnapshot;
}

type TerminalThemeHostProgress =
	| { readonly expectedRevision: TerminalThemeRevision; readonly kind: 'pending' }
	| { readonly kind: 'committed'; readonly revision: TerminalThemeRevision }
	| { readonly failure: TerminalThemeFailure; readonly kind: 'failed' }
	| { readonly kind: 'reconciling' }
	| { readonly kind: 'stale' }
	| { readonly kind: 'unknown' };

type TerminalThemeClientProgress =
	| { readonly kind: 'pending' }
	| { readonly kind: 'prepared' }
	| { readonly kind: 'presenting'; readonly revision: TerminalThemeRevision }
	| { readonly failure: TerminalThemeFailure; readonly kind: 'failed' };

interface ActiveTerminalThemeRevision {
	readonly client: TerminalThemeClientProgress;
	readonly host: TerminalThemeHostProgress;
	readonly operationId: TerminalThemeOperationId;
	readonly snapshot: TerminalThemeSnapshot;
}

export interface TerminalThemeRevisionState {
	readonly active: ActiveTerminalThemeRevision | null;
	readonly current: TerminalThemePresentationCommit;
	readonly host: TerminalThemeHostState;
	readonly mode: TerminalThemePresentationMode;
}

export type TerminalThemeRevisionCommand =
	| {
			readonly expectedRevision: TerminalThemeRevision;
			readonly operationId: TerminalThemeOperationId;
			readonly terminal: TerminalThemeHostSnapshot;
			readonly type: 'apply-host-theme';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly snapshot: TerminalThemeSnapshot;
			readonly type: 'prepare-client-theme';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'reconcile-host-theme';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly revision: TerminalThemeRevision;
			readonly snapshot: TerminalThemeSnapshot;
			readonly type: 'publish-theme';
	  };

export type TerminalThemeRevisionEvent =
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly snapshot: TerminalThemeSnapshot;
			readonly type: 'request';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly state: TerminalThemeHostState;
			readonly type: 'host-committed';
	  }
	| {
			readonly current: TerminalThemeHostState;
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'host-stale';
	  }
	| {
			readonly failure: TerminalThemeFailure;
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'host-failed';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'host-outcome-unknown';
	  }
	| {
			readonly current: TerminalThemeHostState;
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'host-reconciled';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'client-prepared';
	  }
	| {
			readonly failure: TerminalThemeFailure;
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'client-failed';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly revision: TerminalThemeRevision;
			readonly type: 'presentation-committed';
	  }
	| {
			readonly failure: TerminalThemeFailure;
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'presentation-failed';
	  }
	| {
			readonly operationId: TerminalThemeOperationId;
			readonly type: 'retry';
	  }
	| {
			readonly mode: TerminalThemePresentationMode;
			readonly type: 'mode-changed';
	  };

type TerminalThemeTransitionReason =
	| 'already-current'
	| 'host-result-conflict'
	| 'invalid-transition'
	| 'not-retryable'
	| 'operation-conflict'
	| 'operation-in-progress'
	| 'revision-regression'
	| 'stale-operation';

type TerminalThemeRevisionTransition =
	| {
			readonly commands: readonly TerminalThemeRevisionCommand[];
			readonly kind: 'accepted' | 'completed';
			readonly state: TerminalThemeRevisionState;
	  }
	| {
			readonly commands: readonly TerminalThemeRevisionCommand[];
			readonly kind: 'rejected' | 'unchanged';
			readonly reason: TerminalThemeTransitionReason;
			readonly state: TerminalThemeRevisionState;
	  };

export function normalizeTerminalThemeSnapshot(input: unknown): TerminalThemeSnapshot {
	return TerminalThemeSnapshotSchema.parse(input);
}

export function normalizeTerminalThemeHostState(input: unknown): TerminalThemeHostState {
	return TerminalThemeHostStateSchema.parse(input);
}

export function terminalThemeOperationId(value: string): TerminalThemeOperationId {
	return TerminalThemeOperationIdSchema.parse(value);
}

/** Restore the presentation coordinator from the complete Host recovery state. */
export function createTerminalThemeRevisionStateFromHost(
	initialSnapshot: TerminalThemeSnapshot,
	hostState: TerminalThemeHostState,
): TerminalThemeRevisionState {
	const snapshot = normalizeTerminalThemeSnapshot(initialSnapshot);
	const host = normalizeTerminalThemeHostState(hostState);
	const currentSnapshot = Object.freeze({ ...snapshot, terminal: host.terminal });
	return freezeState({
		active: null,
		current: Object.freeze({
			operationId: host.operationId,
			revision: host.revision,
			snapshot: currentSnapshot,
		}),
		host,
		mode: 'active',
	});
}

export function transitionTerminalThemeRevision(
	state: TerminalThemeRevisionState,
	event: TerminalThemeRevisionEvent,
): TerminalThemeRevisionTransition {
	switch (event.type) {
		case 'request':
			return requestTheme(state, event.operationId, event.snapshot);
		case 'host-committed':
			return hostCommitted(state, event.operationId, event.state);
		case 'host-stale':
			return hostStale(state, event.operationId, event.current);
		case 'host-failed':
			return ownerFailed(state, 'host', event.operationId, event.failure);
		case 'host-outcome-unknown':
			return hostOutcomeUnknown(state, event.operationId);
		case 'host-reconciled':
			return hostReconciled(state, event.operationId, event.current);
		case 'client-prepared':
			return clientPrepared(state, event.operationId);
		case 'client-failed':
			return ownerFailed(state, 'client', event.operationId, event.failure);
		case 'presentation-committed':
			return presentationCommitted(state, event.operationId, event.revision);
		case 'presentation-failed':
			return ownerFailed(state, 'presentation', event.operationId, event.failure);
		case 'retry':
			return retryTheme(state, event.operationId);
		case 'mode-changed':
			return changeMode(state, event.mode);
	}
}

function requestTheme(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
	snapshot: TerminalThemeSnapshot,
): TerminalThemeRevisionTransition {
	if (
		state.current.operationId === operationId &&
		!terminalThemeSnapshotsEqual(state.current.snapshot, snapshot)
	) {
		return rejected(state, 'operation-conflict');
	}
	const active = state.active;
	if (active?.operationId === operationId) {
		return terminalThemeSnapshotsEqual(active.snapshot, snapshot)
			? unchanged(state, 'operation-in-progress')
			: rejected(state, 'operation-conflict');
	}
	if (
		active === null &&
		state.host.operationId === state.current.operationId &&
		state.host.revision === state.current.revision &&
		terminalHostThemesEqual(state.host.terminal, state.current.snapshot.terminal) &&
		terminalThemeSnapshotsEqual(state.current.snapshot, snapshot)
	) {
		return unchanged(state, 'already-current');
	}

	const nextActive = freezeActive({
		client: Object.freeze({ kind: 'pending' }),
		host: Object.freeze({ expectedRevision: state.host.revision, kind: 'pending' }),
		operationId,
		snapshot,
	});
	const nextState = freezeState({ ...state, active: nextActive });
	return accepted(nextState, [
		prepareClientCommand(nextActive),
		applyHostCommand(nextActive, state.host.revision),
	]);
}

function hostCommitted(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
	input: TerminalThemeHostState,
): TerminalThemeRevisionTransition {
	const normalized = safeHostState(input);
	if (normalized === null || normalized.operationId !== operationId) {
		return rejected(state, 'host-result-conflict');
	}
	const adopted = adoptHostState(state, normalized);
	if (adopted.kind === 'conflict') return rejected(state, adopted.reason);
	let nextState = adopted.state;
	const active = nextState.active;
	if (active?.operationId !== operationId) return accepted(nextState);
	if (!terminalHostThemesEqual(active.snapshot.terminal, normalized.terminal)) {
		return rejected(nextState, 'host-result-conflict');
	}
	nextState = freezeState({
		...nextState,
		active: freezeActive({
			...active,
			host: Object.freeze({ kind: 'committed', revision: normalized.revision }),
		}),
	});
	return publishIfReady(nextState);
}

function hostStale(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
	input: TerminalThemeHostState,
): TerminalThemeRevisionTransition {
	const normalized = safeHostState(input);
	if (normalized === null) return rejected(state, 'host-result-conflict');
	const adopted = adoptHostState(state, normalized);
	if (adopted.kind === 'conflict') return rejected(state, adopted.reason);
	let nextState = adopted.state;
	const active = nextState.active;
	if (active?.operationId !== operationId) return accepted(nextState);
	if (
		normalized.operationId === operationId &&
		!terminalHostThemesEqual(active.snapshot.terminal, normalized.terminal)
	) {
		return rejected(nextState, 'host-result-conflict');
	}
	const exactReplay =
		normalized.operationId === operationId &&
		terminalHostThemesEqual(active.snapshot.terminal, normalized.terminal);
	nextState = freezeState({
		...nextState,
		active: freezeActive({
			...active,
			host: exactReplay
				? Object.freeze({ kind: 'committed', revision: normalized.revision })
				: Object.freeze({ kind: 'stale' }),
		}),
	});
	return exactReplay ? publishIfReady(nextState) : accepted(nextState);
}

function hostOutcomeUnknown(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
): TerminalThemeRevisionTransition {
	const active = currentActive(state, operationId);
	if (active === null) return unchanged(state, 'stale-operation');
	if (active.host.kind !== 'pending') return rejected(state, 'invalid-transition');
	return accepted(
		freezeState({
			...state,
			active: freezeActive({ ...active, host: Object.freeze({ kind: 'unknown' }) }),
		}),
	);
}

function hostReconciled(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
	input: TerminalThemeHostState,
): TerminalThemeRevisionTransition {
	const normalized = safeHostState(input);
	if (normalized === null) return rejected(state, 'host-result-conflict');
	const adopted = adoptHostState(state, normalized);
	if (adopted.kind === 'conflict') return rejected(state, adopted.reason);
	let nextState = adopted.state;
	const active = nextState.active;
	if (active?.operationId !== operationId) return accepted(nextState);
	if (active.host.kind !== 'reconciling' && active.host.kind !== 'unknown') {
		return rejected(nextState, 'invalid-transition');
	}
	if (
		normalized.operationId === operationId &&
		!terminalHostThemesEqual(active.snapshot.terminal, normalized.terminal)
	) {
		return rejected(nextState, 'host-result-conflict');
	}
	const committed =
		normalized.operationId === operationId &&
		terminalHostThemesEqual(active.snapshot.terminal, normalized.terminal);
	nextState = freezeState({
		...nextState,
		active: freezeActive({
			...active,
			host: committed
				? Object.freeze({ kind: 'committed', revision: normalized.revision })
				: Object.freeze({ kind: 'stale' }),
		}),
	});
	return committed ? publishIfReady(nextState) : accepted(nextState);
}

function clientPrepared(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
): TerminalThemeRevisionTransition {
	const active = currentActive(state, operationId);
	if (active === null) return unchanged(state, 'stale-operation');
	if (active.client.kind !== 'pending') return rejected(state, 'invalid-transition');
	return publishIfReady(
		freezeState({
			...state,
			active: freezeActive({ ...active, client: Object.freeze({ kind: 'prepared' }) }),
		}),
	);
}

function ownerFailed(
	state: TerminalThemeRevisionState,
	owner: 'client' | 'host' | 'presentation',
	operationId: TerminalThemeOperationId,
	failure: TerminalThemeFailure,
): TerminalThemeRevisionTransition {
	const active = currentActive(state, operationId);
	if (active === null) return unchanged(state, 'stale-operation');
	if (failure.outcome !== 'known-unchanged') return rejected(state, 'invalid-transition');
	if (owner === 'host') {
		if (active.host.kind !== 'pending') return rejected(state, 'invalid-transition');
		return accepted(
			freezeState({
				...state,
				active: freezeActive({
					...active,
					host: Object.freeze({ failure: freezeFailure(failure), kind: 'failed' }),
				}),
			}),
		);
	}
	if (
		(owner === 'client' && active.client.kind !== 'pending') ||
		(owner === 'presentation' && active.client.kind !== 'presenting')
	) {
		return rejected(state, 'invalid-transition');
	}
	return accepted(
		freezeState({
			...state,
			active: freezeActive({
				...active,
				client: Object.freeze({ failure: freezeFailure(failure), kind: 'failed' }),
			}),
		}),
	);
}

function retryTheme(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
): TerminalThemeRevisionTransition {
	const active = currentActive(state, operationId);
	if (active === null) return unchanged(state, 'stale-operation');
	if (
		(active.host.kind === 'failed' && !active.host.failure.retryable) ||
		(active.client.kind === 'failed' && !active.client.failure.retryable)
	) {
		return rejected(state, 'not-retryable');
	}

	const commands: TerminalThemeRevisionCommand[] = [];
	let host = active.host;
	let client = active.client;
	if (host.kind === 'stale' || (host.kind === 'failed' && host.failure.retryable)) {
		host = Object.freeze({ expectedRevision: state.host.revision, kind: 'pending' });
		commands.push(applyHostCommand(active, state.host.revision));
	} else if (host.kind === 'unknown') {
		host = Object.freeze({ kind: 'reconciling' });
		commands.push(Object.freeze({ operationId, type: 'reconcile-host-theme' }));
	}
	if (client.kind === 'failed' && client.failure.retryable) {
		client = Object.freeze({ kind: 'pending' });
		commands.push(prepareClientCommand(active));
	}
	if (commands.length === 0) return unchanged(state, 'invalid-transition');
	return accepted(
		freezeState({ ...state, active: freezeActive({ ...active, client, host }) }),
		commands,
	);
}

function presentationCommitted(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
	revision: TerminalThemeRevision,
): TerminalThemeRevisionTransition {
	const active = currentActive(state, operationId);
	if (active === null) return unchanged(state, 'stale-operation');
	if (
		active.client.kind !== 'presenting' ||
		active.client.revision !== revision ||
		active.host.kind !== 'committed' ||
		active.host.revision !== revision ||
		state.host.operationId !== operationId ||
		state.host.revision !== revision
	) {
		return rejected(state, 'invalid-transition');
	}
	return completed(
		freezeState({
			...state,
			active: null,
			current: Object.freeze({ operationId, revision, snapshot: active.snapshot }),
		}),
	);
}

function changeMode(
	state: TerminalThemeRevisionState,
	mode: TerminalThemePresentationMode,
): TerminalThemeRevisionTransition {
	if (state.mode === mode) return unchanged(state, 'invalid-transition');
	return publishIfReady(freezeState({ ...state, mode }));
}

function publishIfReady(state: TerminalThemeRevisionState): TerminalThemeRevisionTransition {
	const active = state.active;
	if (
		state.mode !== 'active' ||
		active === null ||
		active.host.kind !== 'committed' ||
		active.client.kind !== 'prepared' ||
		state.host.operationId !== active.operationId ||
		state.host.revision !== active.host.revision ||
		!terminalHostThemesEqual(state.host.terminal, active.snapshot.terminal)
	) {
		return accepted(state);
	}
	const nextState = freezeState({
		...state,
		active: freezeActive({
			...active,
			client: Object.freeze({ kind: 'presenting', revision: active.host.revision }),
		}),
	});
	return accepted(nextState, [
		Object.freeze({
			operationId: active.operationId,
			revision: active.host.revision,
			snapshot: active.snapshot,
			type: 'publish-theme',
		}),
	]);
}

type HostAdoption =
	| { readonly kind: 'adopted'; readonly state: TerminalThemeRevisionState }
	| { readonly kind: 'conflict'; readonly reason: 'host-result-conflict' | 'revision-regression' };

function adoptHostState(
	state: TerminalThemeRevisionState,
	host: TerminalThemeHostState,
): HostAdoption {
	const comparison = compareRevisions(host.revision, state.host.revision);
	if (comparison < 0) return { kind: 'conflict', reason: 'revision-regression' };
	if (comparison === 0 && !terminalThemeHostStatesEqual(host, state.host)) {
		return { kind: 'conflict', reason: 'host-result-conflict' };
	}
	return comparison === 0
		? { kind: 'adopted', state }
		: { kind: 'adopted', state: freezeState({ ...state, host }) };
}

function currentActive(
	state: TerminalThemeRevisionState,
	operationId: TerminalThemeOperationId,
): ActiveTerminalThemeRevision | null {
	return state.active?.operationId === operationId ? state.active : null;
}

function applyHostCommand(
	active: Pick<ActiveTerminalThemeRevision, 'operationId' | 'snapshot'>,
	expectedRevision: TerminalThemeRevision,
): TerminalThemeRevisionCommand {
	return Object.freeze({
		expectedRevision,
		operationId: active.operationId,
		terminal: active.snapshot.terminal,
		type: 'apply-host-theme',
	});
}

function prepareClientCommand(
	active: Pick<ActiveTerminalThemeRevision, 'operationId' | 'snapshot'>,
): TerminalThemeRevisionCommand {
	return Object.freeze({
		operationId: active.operationId,
		snapshot: active.snapshot,
		type: 'prepare-client-theme',
	});
}

function safeHostState(input: unknown): TerminalThemeHostState | null {
	const result = TerminalThemeHostStateSchema.safeParse(input);
	return result.success ? result.data : null;
}

function terminalThemeSnapshotsEqual(
	left: TerminalThemeSnapshot,
	right: TerminalThemeSnapshot,
): boolean {
	return (
		left === right ||
		(left.application.colorScheme === right.application.colorScheme &&
			left.overlays.ansiPalette.every(
				(color, index) => color === right.overlays.ansiPalette[index],
			) &&
			left.overlays.cursorAccentRgb === right.overlays.cursorAccentRgb &&
			left.overlays.searchActiveMatchBackgroundRgb ===
				right.overlays.searchActiveMatchBackgroundRgb &&
			left.overlays.searchActiveMatchForegroundRgb ===
				right.overlays.searchActiveMatchForegroundRgb &&
			left.overlays.searchMatchBackgroundRgb === right.overlays.searchMatchBackgroundRgb &&
			left.overlays.searchMatchForegroundRgb === right.overlays.searchMatchForegroundRgb &&
			left.overlays.selectionBackgroundRgb === right.overlays.selectionBackgroundRgb &&
			left.overlays.selectionForegroundRgb === right.overlays.selectionForegroundRgb &&
			terminalHostThemesEqual(left.terminal, right.terminal))
	);
}

function terminalHostThemesEqual(
	left: TerminalThemeHostSnapshot,
	right: TerminalThemeHostSnapshot,
): boolean {
	if (
		left === right ||
		(left.colorScheme === right.colorScheme &&
			left.defaultBackgroundRgb === right.defaultBackgroundRgb &&
			left.defaultCursorRgb === right.defaultCursorRgb &&
			left.defaultForegroundRgb === right.defaultForegroundRgb &&
			left.palette.length === right.palette.length)
	) {
		return left === right || left.palette.every((color, index) => color === right.palette[index]);
	}
	return false;
}

function terminalThemeHostStatesEqual(
	left: TerminalThemeHostState,
	right: TerminalThemeHostState,
): boolean {
	return (
		left === right ||
		(left.operationId === right.operationId &&
			left.revision === right.revision &&
			terminalHostThemesEqual(left.terminal, right.terminal))
	);
}

function compareRevisions(left: TerminalThemeRevision, right: TerminalThemeRevision): number {
	const leftValue = BigInt(left);
	const rightValue = BigInt(right);
	if (leftValue < rightValue) return -1;
	if (leftValue > rightValue) return 1;
	return 0;
}

function freezeFailure(failure: TerminalThemeFailure): TerminalThemeFailure {
	return TerminalThemeFailureSchema.parse(failure);
}

function freezeActive(active: ActiveTerminalThemeRevision): ActiveTerminalThemeRevision {
	return Object.freeze(active);
}

function freezeState(state: TerminalThemeRevisionState): TerminalThemeRevisionState {
	return Object.freeze(state);
}

function accepted(
	state: TerminalThemeRevisionState,
	commands: readonly TerminalThemeRevisionCommand[] = [],
): TerminalThemeRevisionTransition {
	return Object.freeze({ commands: Object.freeze([...commands]), kind: 'accepted', state });
}

function completed(state: TerminalThemeRevisionState): TerminalThemeRevisionTransition {
	return Object.freeze({ commands: Object.freeze([]), kind: 'completed', state });
}

function rejected(
	state: TerminalThemeRevisionState,
	reason: TerminalThemeTransitionReason,
): TerminalThemeRevisionTransition {
	return Object.freeze({ commands: Object.freeze([]), kind: 'rejected', reason, state });
}

function unchanged(
	state: TerminalThemeRevisionState,
	reason: TerminalThemeTransitionReason,
): TerminalThemeRevisionTransition {
	return Object.freeze({ commands: Object.freeze([]), kind: 'unchanged', reason, state });
}
