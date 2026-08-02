import { array, assert as assertProperty, constantFrom, property } from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	createInitialProtocolLifecycleState,
	HostPhase,
	LifecycleAuthority,
	LifecycleOutcome,
	LifecycleRejection,
	ProtocolLifecycleStateSchema,
	TerminalPhase,
	transitionProtocolLifecycle,
} from '../lifecycle';
import type { LifecycleTransition, ProtocolLifecycleState } from '../lifecycle';
import { PROTOCOL_LIMITS } from '../limits';

const epoch = '11111111111111111111111111111111';
const staleEpoch = '99999999999999999999999999999999';
const client = '33333333333333333333333333333333';
const secondClient = '44444444444444444444444444444444';
const creation = '55555555555555555555555555555555';
const secondCreation = '66666666666666666666666666666666';
const terminal = '77777777777777777777777777777777';
const secondTerminal = '88888888888888888888888888888888';
const attachment = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const secondAttachment = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const launchDigest = 'c'.repeat(64);

function apply(state: ProtocolLifecycleState, event: unknown): LifecycleTransition {
	const result = transitionProtocolLifecycle(state, event);
	if (!result.ok) throw new Error(result.detail);
	return result.value;
}

function next(state: ProtocolLifecycleState, event: unknown): ProtocolLifecycleState {
	return apply(state, event).state;
}

function servingState(): ProtocolLifecycleState {
	let state = createInitialProtocolLifecycleState();
	state = next(state, { epoch, type: 'launch-host' });
	state = next(state, { epoch, type: 'endpoint-ready' });
	state = next(state, { clientId: client, epoch, type: 'connect-client' });
	return state;
}

function runningTerminalState(): ProtocolLifecycleState {
	let state = servingState();
	state = next(state, {
		authority: LifecycleAuthority.desktop,
		clientId: client,
		creationId: creation,
		epoch,
		launchDigest,
		terminalId: terminal,
		type: 'create-terminal',
	});
	state = next(state, { epoch, terminalId: terminal, type: 'spawn-succeeded' });
	return state;
}

function attachedTerminalState(): ProtocolLifecycleState {
	return next(runningTerminalState(), {
		attachmentId: attachment,
		authority: LifecycleAuthority.desktop,
		canInput: true,
		canResize: true,
		clientId: client,
		epoch,
		requestResizeLease: true,
		terminalId: terminal,
		type: 'attach-terminal',
	});
}

describe('protocol lifecycle identity and survival', () => {
	it('creates once by CreationId and rejects a conflicting retry', () => {
		const state = runningTerminalState();
		const retry = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			creationId: creation,
			epoch,
			launchDigest,
			terminalId: secondTerminal,
			type: 'create-terminal',
		});
		expect(retry).toEqual(
			expect.objectContaining({
				commands: [],
				outcome: LifecycleOutcome.idempotent,
				state,
			}),
		);
		expect(retry.state.terminals).toHaveLength(1);

		const conflict = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			creationId: creation,
			epoch,
			launchDigest: 'd'.repeat(64),
			terminalId: secondTerminal,
			type: 'create-terminal',
		});
		expect(conflict).toEqual(
			expect.objectContaining({
				outcome: LifecycleOutcome.rejected,
				rejection: LifecycleRejection.creationConflict,
				state,
			}),
		);
	});

	it('disconnects attachments without changing any terminal lifecycle fact', () => {
		const state = attachedTerminalState();
		const terminalsBefore = JSON.stringify(state.terminals);
		const disconnected = apply(state, { clientId: client, epoch, type: 'disconnect-client' });

		expect(disconnected.outcome).toBe(LifecycleOutcome.applied);
		expect(disconnected.state.attachments).toEqual([]);
		expect(disconnected.state.connections).toEqual([]);
		expect(JSON.stringify(disconnected.state.terminals)).toBe(terminalsBefore);
	});

	it('rejects stale epochs and attachment authority without mutating state', () => {
		const state = runningTerminalState();
		const stale = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			epoch: staleEpoch,
			terminalId: terminal,
			type: 'terminate-terminal',
		});
		expect(stale).toEqual(
			expect.objectContaining({
				commands: [],
				outcome: LifecycleOutcome.rejected,
				rejection: LifecycleRejection.staleEpoch,
				state,
			}),
		);

		const scoped = apply(state, {
			authority: LifecycleAuthority.attachment,
			clientId: client,
			creationId: secondCreation,
			epoch,
			launchDigest,
			terminalId: secondTerminal,
			type: 'create-terminal',
		});
		expect(scoped).toEqual(
			expect.objectContaining({
				commands: [],
				rejection: LifecycleRejection.authorityDenied,
				state,
			}),
		);
	});

	it.each([
		['child-exited', 'pty-drained'],
		['pty-drained', 'child-exited'],
	] as const)('seals exit after reordered %s then %s completion', (first, second) => {
		let state = runningTerminalState();
		state = next(state, { epoch, terminalId: terminal, type: first });
		expect(state.terminals[0]?.phase).toBe(TerminalPhase.finishing);
		state = next(state, { epoch, terminalId: terminal, type: second });
		expect(state.terminals[0]).toEqual(
			expect.objectContaining({ childExited: true, phase: TerminalPhase.exited, ptyDrained: true }),
		);
	});

	it('never disposes or implicitly terminates a running terminal', () => {
		const state = runningTerminalState();
		const disposal = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			epoch,
			terminalId: terminal,
			type: 'dispose-terminal',
		});
		expect(disposal).toEqual(
			expect.objectContaining({
				commands: [],
				rejection: LifecycleRejection.invalidPhase,
				state,
			}),
		);
	});

	it('requires graceful termination before an idempotent force escalation', () => {
		let state = runningTerminalState();
		expect(
			ProtocolLifecycleStateSchema.safeParse({
				...state,
				terminals: state.terminals.map((entry) => ({
					...entry,
					forceTerminateRequested: true,
				})),
			}).success,
		).toBe(false);
		const premature = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			epoch,
			terminalId: terminal,
			type: 'force-terminate-terminal',
		});
		expect(premature).toEqual(
			expect.objectContaining({
				commands: [],
				rejection: LifecycleRejection.invalidPhase,
				state,
			}),
		);

		const graceful = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			epoch,
			terminalId: terminal,
			type: 'terminate-terminal',
		});
		expect(graceful.commands).toEqual([{ terminalId: terminal, type: 'terminate-child' }]);
		state = graceful.state;
		const scoped = apply(state, {
			authority: LifecycleAuthority.attachment,
			clientId: client,
			epoch,
			terminalId: terminal,
			type: 'force-terminate-terminal',
		});
		expect(scoped).toEqual(
			expect.objectContaining({
				commands: [],
				rejection: LifecycleRejection.authorityDenied,
				state,
			}),
		);

		const forced = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			epoch,
			terminalId: terminal,
			type: 'force-terminate-terminal',
		});
		expect(forced.commands).toEqual([{ terminalId: terminal, type: 'force-terminate-child' }]);
		expect(forced.state.terminals[0]).toEqual(
			expect.objectContaining({ forceTerminateRequested: true, terminateRequested: true }),
		);

		const retry = apply(forced.state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			epoch,
			terminalId: terminal,
			type: 'force-terminate-terminal',
		});
		expect(retry).toEqual(
			expect.objectContaining({ commands: [], outcome: LifecycleOutcome.idempotent }),
		);
	});

	it('stops an empty local-only Host when its final client disconnects', () => {
		const disconnected = apply(servingState(), {
			clientId: client,
			epoch,
			type: 'disconnect-client',
		});
		expect(disconnected.commands).toEqual([{ type: 'stop-host' }]);
		expect(disconnected.state).toEqual(createInitialProtocolLifecycleState());
	});
});

describe('protocol lifecycle bounded delivery', () => {
	it('retains one in-flight frame and one dirty marker under output bursts', () => {
		let state = attachedTerminalState();
		state = next(state, { attachmentId: attachment, clientId: client, epoch, type: 'emit-frame' });
		const initialFrame = state.attachments[0]?.inFlightFrame;
		expect(initialFrame).toEqual({
			baseRevision: null,
			kind: 'snapshot',
			revision: 0,
			sequence: 1,
		});

		state = next(state, { epoch, terminalId: terminal, type: 'terminal-output' });
		state = next(state, { epoch, terminalId: terminal, type: 'terminal-output' });
		expect(state.attachments[0]).toEqual(
			expect.objectContaining({ inFlightFrame: initialFrame, pendingDirty: true }),
		);
		expect(state.terminals[0]?.revision).toBe(2);

		const duplicateEmission = apply(state, {
			attachmentId: attachment,
			clientId: client,
			epoch,
			type: 'emit-frame',
		});
		expect(duplicateEmission.outcome).toBe(LifecycleOutcome.idempotent);
		expect(duplicateEmission.state.attachments[0]?.inFlightFrame).toEqual(initialFrame);

		state = next(state, {
			attachmentId: attachment,
			clientId: client,
			epoch,
			revision: 0,
			sequence: 1,
			type: 'frame-applied',
		});
		state = next(state, { attachmentId: attachment, clientId: client, epoch, type: 'emit-frame' });
		expect(state.attachments[0]?.inFlightFrame).toEqual({
			baseRevision: 0,
			kind: 'delta',
			revision: 2,
			sequence: 2,
		});
	});

	it('turns decoder rejection into a replacement snapshot', () => {
		let state = attachedTerminalState();
		state = next(state, { attachmentId: attachment, clientId: client, epoch, type: 'emit-frame' });
		state = next(state, {
			attachmentId: attachment,
			clientId: client,
			epoch,
			type: 'need-snapshot',
		});
		state = next(state, { attachmentId: attachment, clientId: client, epoch, type: 'emit-frame' });
		expect(state.attachments[0]?.inFlightFrame).toEqual(
			expect.objectContaining({ baseRevision: null, kind: 'snapshot', sequence: 2 }),
		);
	});

	it('allows only the exclusive resize lease to mutate canonical dimensions', () => {
		let state = attachedTerminalState();
		state = next(state, { clientId: secondClient, epoch, type: 'connect-client' });
		state = next(state, {
			attachmentId: secondAttachment,
			authority: LifecycleAuthority.desktop,
			canInput: false,
			canResize: true,
			clientId: secondClient,
			epoch,
			requestResizeLease: false,
			terminalId: terminal,
			type: 'attach-terminal',
		});
		const denied = apply(state, {
			attachmentId: secondAttachment,
			clientId: secondClient,
			columns: 100,
			epoch,
			rows: 30,
			type: 'resize-terminal',
		});
		expect(denied.rejection).toBe(LifecycleRejection.resizeLeaseDenied);
		expect(denied.state.terminals[0]).toEqual(state.terminals[0]);

		const accepted = apply(state, {
			attachmentId: attachment,
			clientId: client,
			columns: 100,
			epoch,
			rows: 30,
			type: 'resize-terminal',
		});
		expect(accepted.commands).toEqual([
			{ columns: 100, rows: 30, terminalId: terminal, type: 'resize-pty' },
		]);
		expect(accepted.state.terminals[0]).toEqual(
			expect.objectContaining({ columns: 100, revision: 1, rows: 30 }),
		);
		expect(accepted.state.attachments.every((entry) => entry.pendingDirty)).toBe(true);
	});

	it('applies backpressure at the bounded pending input limit', () => {
		let state = attachedTerminalState();
		for (let index = 0; index < 4; index += 1) {
			state = next(state, {
				attachmentId: attachment,
				bytes: PROTOCOL_LIMITS.maxInputBytes,
				clientId: client,
				epoch,
				type: 'accept-input',
			});
		}
		expect(state.terminals[0]?.pendingInputBytes).toBe(PROTOCOL_LIMITS.maxPendingInputBytes);
		const backpressured = apply(state, {
			attachmentId: attachment,
			bytes: 1,
			clientId: client,
			epoch,
			type: 'accept-input',
		});
		expect(backpressured).toEqual(
			expect.objectContaining({
				commands: [],
				rejection: LifecycleRejection.inputBackpressure,
				state,
			}),
		);
	});
});

describe('protocol lifecycle update gate and generated traces', () => {
	it('defers upgrade until the final retained terminal exits and is disposed', () => {
		let state = runningTerminalState();
		state = next(state, { epoch, type: 'request-upgrade' });
		expect(state).toEqual(
			expect.objectContaining({ phase: HostPhase.serving, upgradePending: true }),
		);
		const createWhilePending = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			creationId: secondCreation,
			epoch,
			launchDigest,
			terminalId: secondTerminal,
			type: 'create-terminal',
		});
		expect(createWhilePending.rejection).toBe(LifecycleRejection.upgradePending);

		state = next(state, { epoch, terminalId: terminal, type: 'child-exited' });
		state = next(state, { epoch, terminalId: terminal, type: 'pty-drained' });
		const disposal = apply(state, {
			authority: LifecycleAuthority.desktop,
			clientId: client,
			epoch,
			terminalId: terminal,
			type: 'dispose-terminal',
		});
		expect(disposal.commands).toEqual([{ type: 'close-connections' }]);
		expect(disposal.state).toEqual(
			expect.objectContaining({ connections: [], phase: HostPhase.draining, terminals: [] }),
		);
		state = next(disposal.state, { epoch, type: 'stop-host' });
		expect(state).toEqual(createInitialProtocolLifecycleState());
	});

	it('keeps every generated event trace inside the lifecycle invariant schema', () => {
		const events = [
			{ epoch, type: 'launch-host' },
			{ epoch, type: 'endpoint-ready' },
			{ clientId: client, epoch, type: 'connect-client' },
			{
				authority: LifecycleAuthority.desktop,
				clientId: client,
				creationId: creation,
				epoch,
				launchDigest,
				terminalId: terminal,
				type: 'create-terminal',
			},
			{ epoch, terminalId: terminal, type: 'spawn-succeeded' },
			{
				authority: LifecycleAuthority.desktop,
				clientId: client,
				epoch,
				terminalId: terminal,
				type: 'terminate-terminal',
			},
			{
				authority: LifecycleAuthority.desktop,
				clientId: client,
				epoch,
				terminalId: terminal,
				type: 'force-terminate-terminal',
			},
			{
				attachmentId: attachment,
				authority: LifecycleAuthority.desktop,
				canInput: true,
				canResize: true,
				clientId: client,
				epoch,
				requestResizeLease: true,
				terminalId: terminal,
				type: 'attach-terminal',
			},
			{ epoch, terminalId: terminal, type: 'terminal-output' },
			{ attachmentId: attachment, clientId: client, epoch, type: 'emit-frame' },
			{
				attachmentId: attachment,
				clientId: client,
				epoch,
				revision: 0,
				sequence: 1,
				type: 'frame-applied',
			},
			{ attachmentId: attachment, clientId: client, epoch, type: 'need-snapshot' },
			{ epoch, terminalId: terminal, type: 'child-exited' },
			{ epoch, terminalId: terminal, type: 'pty-drained' },
			{ clientId: client, epoch, type: 'disconnect-client' },
			{ epoch, type: 'request-upgrade' },
		] as const;

		expect(() =>
			assertProperty(
				property(array(constantFrom(...events), { maxLength: 100 }), (generatedEvents) => {
					let state = createInitialProtocolLifecycleState();
					for (const event of generatedEvents) {
						const transition = transitionProtocolLifecycle(state, event);
						if (!transition.ok) return false;
						state = transition.value.state;
						if (!ProtocolLifecycleStateSchema.safeParse(state).success) return false;
					}
					return JSON.stringify(state) === JSON.stringify(JSON.parse(JSON.stringify(state)));
				}),
				{ numRuns: 500, seed: 0x1fec7c1e },
			),
		).not.toThrow();
	});
});
