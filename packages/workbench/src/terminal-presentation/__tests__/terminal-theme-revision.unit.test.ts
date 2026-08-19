import { describe, expect, it } from 'vitest';

import {
	createTerminalThemeRevisionState,
	normalizeTerminalThemeSnapshot,
	terminalThemeOperationId,
	terminalThemeRevision,
	TerminalThemeFailureSchema,
	TerminalThemeRevisionSchema,
	TerminalThemeSnapshotSchema,
	transitionTerminalThemeRevision,
	type TerminalThemeHostState,
	type TerminalThemeRevisionState,
	type TerminalThemeSnapshot,
} from '../terminal-theme-revision';

const palette = Array.from({ length: 256 }, (_value, index) => index * 0x01_01_01);

function theme(overrides: Partial<TerminalThemeSnapshot> = {}): TerminalThemeSnapshot {
	return normalizeTerminalThemeSnapshot({
		application: { colorScheme: 'light' },
		overlays: {
			cursorAccentRgb: 0xfc_fc_fc,
			searchActiveMatchBackgroundRgb: 0xf5_a9_7f,
			searchActiveMatchForegroundRgb: 0x1e_1e_1e,
			searchMatchBackgroundRgb: 0x51_5c_6a,
			searchMatchForegroundRgb: 0xff_ff_ff,
			selectionBackgroundRgb: 0xd4_d4_d4,
			selectionForegroundRgb: 0x1e_1e_1e,
		},
		terminal: {
			colorScheme: 'light',
			defaultBackgroundRgb: 0xfc_fc_fc,
			defaultCursorRgb: 0x57_be_f8,
			defaultForegroundRgb: 0x14_14_14,
			palette,
		},
		...overrides,
	});
}

function hostState(
	operation: string | null,
	revision: string,
	snapshot: TerminalThemeSnapshot,
): TerminalThemeHostState {
	return {
		operationId: operation === null ? null : terminalThemeOperationId(operation),
		revision: terminalThemeRevision(revision),
		terminal: snapshot.terminal,
	};
}

function request(
	state: TerminalThemeRevisionState,
	operation: string,
	snapshot: TerminalThemeSnapshot,
) {
	return transitionTerminalThemeRevision(state, {
		operationId: terminalThemeOperationId(operation),
		snapshot,
		type: 'request',
	});
}

describe('terminal theme revision contract', () => {
	it('normalizes one complete immutable terminal, overlay, and application snapshot', () => {
		const inputPalette = [...palette];
		const snapshot = normalizeTerminalThemeSnapshot({
			...theme(),
			terminal: { ...theme().terminal, palette: inputPalette },
		});

		expect(snapshot.terminal.palette).toHaveLength(256);
		expect(snapshot.terminal.defaultCursorRgb).toBe(0x57_be_f8);
		expect(Object.isFrozen(snapshot)).toBe(true);
		expect(Object.isFrozen(snapshot.application)).toBe(true);
		expect(Object.isFrozen(snapshot.overlays)).toBe(true);
		expect(Object.isFrozen(snapshot.terminal)).toBe(true);
		expect(Object.isFrozen(snapshot.terminal.palette)).toBe(true);
		inputPalette[0] = 0xff_ff_ff;
		expect(snapshot.terminal.palette[0]).toBe(0);

		expect(
			TerminalThemeSnapshotSchema.safeParse({
				...snapshot,
				terminal: { ...snapshot.terminal, palette: snapshot.terminal.palette.slice(1) },
			}).success,
		).toBe(false);
		expect(
			TerminalThemeSnapshotSchema.safeParse({
				...snapshot,
				overlays: { ...snapshot.overlays, cursorAccentRgb: 0x01_00_00_00 },
			}).success,
		).toBe(false);
		expect(TerminalThemeSnapshotSchema.safeParse({ ...snapshot, extra: true }).success).toBe(false);
		expect(TerminalThemeRevisionSchema.safeParse('01').success).toBe(false);
		expect(TerminalThemeRevisionSchema.safeParse('18446744073709551616').success).toBe(false);
		expect(
			TerminalThemeFailureSchema.safeParse({
				code: '',
				outcome: 'known-unchanged',
				retryable: true,
			}).success,
		).toBe(false);
	});

	it('publishes one complete revision after Host and client preparation settle in either order', () => {
		const initial = theme();
		const dark = theme({
			application: { colorScheme: 'dark' },
			terminal: {
				...initial.terminal,
				colorScheme: 'dark',
				defaultBackgroundRgb: 0x1e_1e_1e,
				defaultForegroundRgb: 0xd4_d4_d4,
			},
		});
		let state = createTerminalThemeRevisionState(initial);
		const started = request(state, 'theme:dark:1', dark);
		state = started.state;

		expect(started.kind).toBe('accepted');
		expect(started.commands).toEqual([
			{
				expectedRevision: '0',
				operationId: 'theme:dark:1',
				terminal: dark.terminal,
				type: 'apply-host-theme',
			},
			{
				operationId: 'theme:dark:1',
				snapshot: dark,
				type: 'prepare-client-theme',
			},
		]);

		const clientPrepared = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:dark:1'),
			type: 'client-prepared',
		});
		state = clientPrepared.state;
		expect(clientPrepared.commands).toEqual([]);
		expect(state.current.snapshot).toBe(initial);

		const hostCommitted = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:dark:1'),
			state: hostState('theme:dark:1', '1', dark),
			type: 'host-committed',
		});
		state = hostCommitted.state;
		expect(hostCommitted.commands).toEqual([
			{
				operationId: 'theme:dark:1',
				revision: '1',
				snapshot: dark,
				type: 'publish-theme',
			},
		]);
		expect(state.current.snapshot).toBe(initial);

		const completed = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:dark:1'),
			revision: terminalThemeRevision('1'),
			type: 'presentation-committed',
		});

		expect(completed.kind).toBe('completed');
		expect(completed.state.active).toBeNull();
		expect(completed.state.current).toEqual({
			operationId: 'theme:dark:1',
			revision: '1',
			snapshot: dark,
		});
	});

	it('keeps superseded results invisible and rebases the latest stale operation', () => {
		const initial = theme();
		const first = theme({ application: { colorScheme: 'dark' } });
		const latest = theme({
			terminal: { ...initial.terminal, defaultBackgroundRgb: 0x12_34_56 },
		});
		let state = request(createTerminalThemeRevisionState(initial), 'theme:first', first).state;
		state = request(state, 'theme:latest', latest).state;

		state = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:first'),
			state: hostState('theme:first', '1', first),
			type: 'host-committed',
		}).state;
		const staleClient = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:first'),
			type: 'client-prepared',
		});
		state = staleClient.state;
		expect(staleClient.kind).toBe('unchanged');
		expect(staleClient.commands).toEqual([]);
		expect(state.current.revision).toBe('0');

		const stale = transitionTerminalThemeRevision(state, {
			current: hostState('theme:first', '1', first),
			operationId: terminalThemeOperationId('theme:latest'),
			type: 'host-stale',
		});
		state = stale.state;
		expect(stale.kind).toBe('accepted');
		expect(state.current.revision).toBe('0');

		const retried = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:latest'),
			type: 'retry',
		});
		expect(retried.commands).toEqual([
			{
				expectedRevision: '1',
				operationId: 'theme:latest',
				terminal: latest.terminal,
				type: 'apply-host-theme',
			},
		]);
	});

	it('supersedes an unpresented Host commit when the latest intent returns to the visible theme', () => {
		const initial = theme();
		const dark = theme({ application: { colorScheme: 'dark' } });
		let state = request(createTerminalThemeRevisionState(initial), 'theme:dark', dark).state;
		state = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:dark'),
			state: hostState('theme:dark', '1', dark),
			type: 'host-committed',
		}).state;

		const reverted = request(state, 'theme:revert', initial);

		expect(reverted.kind).toBe('accepted');
		expect(reverted.state.active?.operationId).toBe('theme:revert');
		expect(reverted.commands).toEqual([
			{
				expectedRevision: '1',
				operationId: 'theme:revert',
				terminal: initial.terminal,
				type: 'apply-host-theme',
			},
			{
				operationId: 'theme:revert',
				snapshot: initial,
				type: 'prepare-client-theme',
			},
		]);
	});

	it('reconciles an unknown Host outcome and retries only known unchanged failures', () => {
		const initial = theme();
		const next = theme({ application: { colorScheme: 'dark' } });
		let state = request(createTerminalThemeRevisionState(initial), 'theme:unknown', next).state;

		state = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:unknown'),
			type: 'host-outcome-unknown',
		}).state;
		const reconcile = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:unknown'),
			type: 'retry',
		});
		expect(reconcile.commands).toEqual([
			{ operationId: 'theme:unknown', type: 'reconcile-host-theme' },
		]);

		state = transitionTerminalThemeRevision(reconcile.state, {
			current: hostState('theme:unknown', '1', next),
			operationId: terminalThemeOperationId('theme:unknown'),
			type: 'host-reconciled',
		}).state;
		expect(state.active?.host.kind).toBe('committed');

		const clientFailed = transitionTerminalThemeRevision(state, {
			failure: { code: 'renderer-preparation-failed', outcome: 'known-unchanged', retryable: true },
			operationId: terminalThemeOperationId('theme:unknown'),
			type: 'client-failed',
		});
		const clientRetry = transitionTerminalThemeRevision(clientFailed.state, {
			operationId: terminalThemeOperationId('theme:unknown'),
			type: 'retry',
		});
		expect(clientRetry.commands).toEqual([
			{
				operationId: 'theme:unknown',
				snapshot: next,
				type: 'prepare-client-theme',
			},
		]);
		expect(clientRetry.state.host.revision).toBe('1');
	});

	it('retries a known unchanged Host failure without discarding prepared client state', () => {
		const initial = theme();
		const next = theme({ application: { colorScheme: 'dark' } });
		let state = request(createTerminalThemeRevisionState(initial), 'theme:host-retry', next).state;
		state = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:host-retry'),
			type: 'client-prepared',
		}).state;
		state = transitionTerminalThemeRevision(state, {
			failure: { code: 'host-busy', outcome: 'known-unchanged', retryable: true },
			operationId: terminalThemeOperationId('theme:host-retry'),
			type: 'host-failed',
		}).state;

		const retried = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:host-retry'),
			type: 'retry',
		});
		expect(retried.commands).toEqual([
			{
				expectedRevision: '0',
				operationId: 'theme:host-retry',
				terminal: next.terminal,
				type: 'apply-host-theme',
			},
		]);
		expect(retried.state.active?.client.kind).toBe('prepared');

		const committed = transitionTerminalThemeRevision(retried.state, {
			operationId: terminalThemeOperationId('theme:host-retry'),
			state: hostState('theme:host-retry', '1', next),
			type: 'host-committed',
		});
		expect(committed.commands).toEqual([
			{
				operationId: 'theme:host-retry',
				revision: '1',
				snapshot: next,
				type: 'publish-theme',
			},
		]);
	});

	it('does not publish while hidden or suspended and resumes only the latest complete operation', () => {
		const initial = theme();
		const next = theme({ application: { colorScheme: 'dark' } });
		let state = createTerminalThemeRevisionState(initial);
		state = transitionTerminalThemeRevision(state, { mode: 'hidden', type: 'mode-changed' }).state;
		state = request(state, 'theme:hidden', next).state;
		state = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:hidden'),
			state: hostState('theme:hidden', '1', next),
			type: 'host-committed',
		}).state;
		const prepared = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:hidden'),
			type: 'client-prepared',
		});
		state = prepared.state;
		expect(prepared.commands).toEqual([]);

		state = transitionTerminalThemeRevision(state, {
			mode: 'suspended',
			type: 'mode-changed',
		}).state;
		const resumed = transitionTerminalThemeRevision(state, {
			mode: 'active',
			type: 'mode-changed',
		});
		expect(resumed.commands).toEqual([
			{
				operationId: 'theme:hidden',
				revision: '1',
				snapshot: next,
				type: 'publish-theme',
			},
		]);
	});

	it('rejects conflicting identities and non-retryable failures without partial publication', () => {
		const initial = theme();
		const next = theme({ application: { colorScheme: 'dark' } });
		let state = request(createTerminalThemeRevisionState(initial), 'theme:fixed', next).state;
		const conflict = request(
			state,
			'theme:fixed',
			theme({ application: { colorScheme: 'light' } }),
		);
		expect(conflict).toMatchObject({ kind: 'rejected', reason: 'operation-conflict' });

		const hostConflict = transitionTerminalThemeRevision(state, {
			current: hostState(
				'theme:fixed',
				'1',
				theme({ terminal: { ...initial.terminal, defaultBackgroundRgb: 0x12_34_56 } }),
			),
			operationId: terminalThemeOperationId('theme:fixed'),
			type: 'host-stale',
		});
		expect(hostConflict).toMatchObject({ kind: 'rejected', reason: 'host-result-conflict' });
		expect(hostConflict.state.current.revision).toBe('0');

		state = transitionTerminalThemeRevision(state, {
			failure: { code: 'invalid-theme', outcome: 'known-unchanged', retryable: false },
			operationId: terminalThemeOperationId('theme:fixed'),
			type: 'host-failed',
		}).state;
		const retry = transitionTerminalThemeRevision(state, {
			operationId: terminalThemeOperationId('theme:fixed'),
			type: 'retry',
		});
		expect(retry).toMatchObject({ kind: 'rejected', reason: 'not-retryable' });
		expect(retry.state.current).toEqual({
			operationId: null,
			revision: '0',
			snapshot: initial,
		});
	});
});
