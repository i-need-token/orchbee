import type { TerminalAppearancePreferences } from '../../../terminal-settings';
import type { ProjectedTerminal } from '@orchbee/terminal';
import {
	ProjectedAttachmentBootstrapSchema,
	type ProjectedTerminalCommand,
} from '@orchbee/workbench-client-protocol';
import { CanonicalUint64TextSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalTransport } from '../../terminal-projection';
import { createTerminalThemePresentationParticipant } from '../createTerminalThemePresentationParticipant';

const bootstrap = ProjectedAttachmentBootstrapSchema.parse({
	attachmentId: '11'.repeat(16),
	canInput: true,
	canResize: true,
	capabilityRevision: '1',
	creationId: '22'.repeat(16),
	hostEpoch: '33'.repeat(16),
	terminalId: '44'.repeat(16),
	terminalOutcome: null,
	theme: {
		operationId: null,
		revision: '0',
		terminal: {
			colorScheme: 'light',
			defaultBackgroundRgb: 0xfc_fc_fc,
			defaultCursorRgb: null,
			defaultForegroundRgb: 0x14_14_14,
			palette: Array.from({ length: 256 }, (_value, index) => index),
		},
	},
});

type ThemeTerminalDriver = Pick<
	ProjectedTerminal,
	| 'abortThemePresentation'
	| 'canCommitThemePresentation'
	| 'commitThemePresentation'
	| 'prepareThemePresentation'
	| 'waitForThemePresentation'
>;

describe('createTerminalThemePresentationParticipant', () => {
	it('restores configured Host colors after reopening an overridden terminal without replacing its palette', async () => {
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = terminalDriver({});
		const participant = createTerminalThemePresentationParticipant({
			bootstrap: {
				...bootstrap,
				hostDefaultColors: {
					colorScheme: 'light',
					defaultBackgroundRgb: 0xfcfcfc,
					defaultForegroundRgb: 0x141414,
				},
				theme: {
					operationId: null,
					revision: CanonicalUint64TextSchema.parse('0'),
					terminal: {
						...bootstrap.theme.terminal,
						colorScheme: 'dark',
						defaultBackgroundRgb: 0x1e1e1e,
						defaultForegroundRgb: 0xd4d4d4,
					},
				},
			},
			createOperationId: () => '55'.repeat(16),
			onPresented: () => undefined,
			terminal,
			transport: transportFixture((command) => {
				commands.push(command);
			}),
		});
		const preparation = participant.prepareThemePresentation({
			...appearancePreferences,
			applicationTheme: 'light',
			terminalDisplayTheme: 'host',
		});
		const apply = commands[0];
		if (apply?.kind !== 'apply-terminal-theme') throw new Error('Missing Host restore');
		expect(apply.payload.terminal).toEqual(bootstrap.theme.terminal);
		participant.dispose();
		await expect(preparation).resolves.toEqual({ kind: 'cancelled' });
	});
	it('preflights the renderer before sending one valid Host operation and publishes its exact revision', async () => {
		const order: string[] = [];
		const commands: ProjectedTerminalCommand[] = [];
		const terminal = terminalDriver({
			commitThemePresentation: vi.fn<ProjectedTerminal['commitThemePresentation']>(() => true),
			prepareThemePresentation: vi.fn<ProjectedTerminal['prepareThemePresentation']>(() => {
				order.push('renderer-prepare');
				return { kind: 'prepared' };
			}),
			waitForThemePresentation: vi.fn<ProjectedTerminal['waitForThemePresentation']>(async () => ({
				kind: 'ready',
			})),
		});
		const transport = transportFixture((command) => {
			order.push('host-send');
			commands.push(command);
		});
		const onPresented = vi.fn<() => void>();
		const participant = createTerminalThemePresentationParticipant({
			bootstrap,
			createOperationId: () => '55'.repeat(16),
			onPresented,
			terminal,
			transport,
		});

		const preparation = participant.prepareThemePresentation({
			...appearancePreferences,
			applicationTheme: 'dark',
			terminalDisplayTheme: 'dark',
		});
		expect(order).toEqual(['renderer-prepare', 'host-send']);
		const apply = commands[0];
		if (apply?.kind !== 'apply-terminal-theme') {
			throw new Error('Theme participant did not send the Host apply command.');
		}
		expect(apply.payload).toMatchObject({
			expectedThemeRevision: '0',
			operationId: '55'.repeat(16),
			terminal: {
				colorScheme: 'dark',
				defaultBackgroundRgb: 0x1e_1e_1e,
				defaultForegroundRgb: 0xd4_d4_d4,
			},
		});
		expect(terminal.prepareThemePresentation).toHaveBeenCalledWith(
			'55'.repeat(16),
			expect.objectContaining({
				selectionBackgroundRgb: 0x41_55_76,
				selectionForegroundRgb: 0xd4_d4_d4,
			}),
		);

		expect(
			participant.handleEvent({
				event: {
					current: {
						operationId: apply.payload.operationId,
						revision: CanonicalUint64TextSchema.parse('1'),
						terminal: apply.payload.terminal,
					},
					operationId: apply.payload.operationId,
					terminalId: bootstrap.terminalId,
					transition: 'applied',
				},
				kind: 'terminal-theme-result',
			}),
		).toBe(true);
		const prepared = await preparation;
		if (prepared.kind !== 'prepared') throw new Error('Theme publication was not prepared.');
		expect(terminal.waitForThemePresentation).toHaveBeenCalledWith('55'.repeat(16), 1n);
		expect(prepared.commit.canCommit()).toBe(true);
		prepared.commit.commit();
		expect(terminal.commitThemePresentation).toHaveBeenCalledWith('55'.repeat(16), 1n);
		expect(onPresented).toHaveBeenCalledOnce();
	});

	it('does not contact the Host when renderer preflight rejects the complete overlay set', async () => {
		const terminal = terminalDriver({
			prepareThemePresentation: vi.fn<ProjectedTerminal['prepareThemePresentation']>(() => ({
				code: 'renderer-preparation-failed',
				kind: 'failed',
				retryable: true,
			})),
		});
		const transport = transportFixture();
		const participant = createTerminalThemePresentationParticipant({
			bootstrap,
			createOperationId: () => '66'.repeat(16),
			onPresented: () => undefined,
			terminal,
			transport,
		});

		await expect(
			participant.prepareThemePresentation({
				...appearancePreferences,
				applicationTheme: 'light',
				terminalDisplayTheme: 'dark',
			}),
		).resolves.toEqual({
			code: 'renderer-preparation-failed',
			kind: 'failed',
			retryable: true,
		});
		expect(transport.send).not.toHaveBeenCalled();
		expect(terminal.abortThemePresentation).toHaveBeenCalledWith('66'.repeat(16));
	});

	it('composites the selection accent over the effective light terminal background', async () => {
		const terminal = terminalDriver();
		const participant = createTerminalThemePresentationParticipant({
			bootstrap,
			createOperationId: () => '77'.repeat(16),
			onPresented: () => undefined,
			terminal,
			transport: transportFixture(),
		});

		const preparation = participant.prepareThemePresentation({
			...appearancePreferences,
			applicationTheme: 'dark',
		});
		expect(terminal.prepareThemePresentation).toHaveBeenCalledWith(
			'77'.repeat(16),
			expect.objectContaining({
				selectionBackgroundRgb: 0xc6_da_fb,
				selectionForegroundRgb: 0x14_14_14,
			}),
		);

		participant.dispose();
		await expect(preparation).resolves.toEqual({ kind: 'cancelled' });
	});
});

function terminalDriver(overrides: Partial<ThemeTerminalDriver> = {}): ThemeTerminalDriver {
	return {
		abortThemePresentation: vi.fn<ProjectedTerminal['abortThemePresentation']>(),
		canCommitThemePresentation: vi.fn<ProjectedTerminal['canCommitThemePresentation']>(() => true),
		commitThemePresentation: vi.fn<ProjectedTerminal['commitThemePresentation']>(() => true),
		prepareThemePresentation: vi.fn<ProjectedTerminal['prepareThemePresentation']>(() => ({
			kind: 'prepared',
		})),
		waitForThemePresentation: vi.fn<ProjectedTerminal['waitForThemePresentation']>(async () => ({
			kind: 'ready',
		})),
		...overrides,
	};
}

function transportFixture(
	onSend: (command: ProjectedTerminalCommand) => void = () => undefined,
): ProjectedTerminalTransport & {
	readonly send: ReturnType<typeof vi.fn<ProjectedTerminalTransport['send']>>;
} {
	return {
		close: vi.fn<ProjectedTerminalTransport['close']>(),
		projectionBufferTrust: 'local-host',
		send: vi.fn<ProjectedTerminalTransport['send']>(onSend),
		subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>(() => () => undefined),
	};
}

const appearancePreferences: TerminalAppearancePreferences = {
	applicationTheme: 'system',
	backgroundOpacityPercent: 100,
	customBackground: '#fcfcfc',
	customForeground: '#141414',
	fontFamily: 'monospace',
	fontLigatures: true,
	fontSize: 14,
	fontWeight: 400,
	fontWeightBold: 700,
	terminalPadding: 0,
	terminalDisplayTheme: 'host',
};
