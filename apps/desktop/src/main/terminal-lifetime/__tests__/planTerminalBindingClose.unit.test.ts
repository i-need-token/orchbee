import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	TerminalPhase,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import { TerminalBindingIdSchema, type TerminalBinding } from '../../recovery';
import { planTerminalBindingClose } from '../planTerminalBindingClose';

const firstEpoch = HostEpochSchema.parse('11'.repeat(16));
const secondEpoch = HostEpochSchema.parse('12'.repeat(16));
const creationId = CreationIdSchema.parse('21'.repeat(16));
const terminalId = TerminalIdSchema.parse('31'.repeat(16));
const identity = { creationId, hostEpoch: firstEpoch, terminalId };

describe('planTerminalBindingClose', () => {
	it('treats an absent binding as already closed', () => {
		expect(planTerminalBindingClose(undefined, [], firstEpoch)).toEqual({
			kind: 'already-closed',
		});
	});

	it('removes intent-only bindings without naming an unrelated terminal', () => {
		for (const state of [
			{ kind: 'planned' as const },
			{ kind: 'lost' as const, previousIdentity: identity, reason: 'identity-missing' as const },
		]) {
			expect(planTerminalBindingClose(createBinding(state), [createSummary()], firstEpoch)).toEqual(
				{ kind: 'remove-binding' },
			);
		}
	});

	it('targets a same-epoch requested creation and removes a missing or old-epoch request', () => {
		const requested = createBinding({ hostEpoch: firstEpoch, kind: 'create-requested' });
		expect(planTerminalBindingClose(requested, [createSummary()], firstEpoch)).toMatchObject({
			identity,
			kind: 'close-terminal',
		});
		expect(planTerminalBindingClose(requested, [], firstEpoch)).toEqual({
			kind: 'remove-binding',
		});
		expect(planTerminalBindingClose(requested, [createSummary()], secondEpoch)).toEqual({
			kind: 'remove-binding',
		});
	});

	it('targets only the complete retained identity and ignores unrelated inventory', () => {
		const binding = createBinding({ identity, kind: 'retained' });
		const unrelated = createSummary({
			identity: {
				creationId: CreationIdSchema.parse('22'.repeat(16)),
				hostEpoch: firstEpoch,
				terminalId: TerminalIdSchema.parse('32'.repeat(16)),
			},
		});

		expect(planTerminalBindingClose(binding, [unrelated, createSummary()], firstEpoch)).toEqual({
			binding,
			identity,
			kind: 'close-terminal',
			summary: createSummary(),
		});
	});

	it('rejects CreationId, TerminalId, and durable-exit contradictions', () => {
		const binding = createBinding({ identity, kind: 'retained' });
		const creationCollision = createSummary({
			identity: { ...identity, terminalId: TerminalIdSchema.parse('33'.repeat(16)) },
		});
		expect(planTerminalBindingClose(binding, [creationCollision], firstEpoch)).toEqual({
			kind: 'conflict',
			reason: 'identity-conflict',
		});

		const exited = createBinding({
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('7'),
			identity,
			kind: 'exited',
		});
		expect(planTerminalBindingClose(exited, [createSummary()], firstEpoch)).toEqual({
			kind: 'conflict',
			reason: 'identity-conflict',
		});
	});
});

function createBinding(state: TerminalBinding['state']): TerminalBinding {
	return {
		bindingId: TerminalBindingIdSchema.parse('binding-1'),
		creationId,
		launch: {
			arguments: [],
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: 80,
			configuration: null,
			currentDirectory: '/users/orchbee',
			environment: [],
			executable: '/bin/sh',
			rows: 24,
			terminalProfile: createDefaultTerminalProfile(),
		},
		presentationId: terminalPresentationId('presentation-1'),
		state,
	};
}

type RunningTerminalSummary = Extract<
	TerminalSummary,
	{ readonly phase: typeof TerminalPhase.running }
>;

function createSummary(overrides: Partial<RunningTerminalSummary> = {}): RunningTerminalSummary {
	return {
		columns: 80,
		configuration: { scrollbackBytes: 10_000_000 },
		configurationRevision: CanonicalUint64TextSchema.parse('1'),
		exit: null,
		failure: null,
		failureContainment: null,
		identity,
		phase: TerminalPhase.running,
		process: null,
		revision: CanonicalUint64TextSchema.parse('4'),
		rows: 24,
		terminateRequested: false,
		theme: createInitialTheme(),
		...overrides,
	};
}

function createInitialTheme(): TerminalSummary['theme'] {
	const profile = createDefaultTerminalProfile();
	return {
		operationId: null,
		revision: CanonicalUint64TextSchema.parse('0'),
		terminal: {
			colorScheme: profile.colorScheme,
			defaultBackgroundRgb: profile.defaultBackgroundRgb,
			defaultCursorRgb: profile.defaultCursorRgb,
			defaultForegroundRgb: profile.defaultForegroundRgb,
			palette: profile.palette,
		},
	};
}
