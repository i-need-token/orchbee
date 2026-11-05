import type { ProjectedTerminal, TerminalThemeOverlayPresentation } from '@orchbee/terminal';
import type {
	ProjectedAttachmentBootstrap,
	ProjectedTerminalEvent,
} from '@orchbee/workbench-client-protocol';
import { CanonicalUint64TextSchema } from '@orchbee/terminal-protocol';
import {
	createTerminalThemeRevisionStateFromHost,
	normalizeTerminalThemeHostState,
	normalizeTerminalThemeSnapshot,
	terminalThemeOperationId,
	transitionTerminalThemeRevision,
	type TerminalThemeFailure,
	type TerminalThemeOperationId,
	type TerminalThemeRevisionCommand,
	type TerminalThemeRevisionEvent,
	type TerminalThemeRevisionState,
	type TerminalThemeSnapshot,
} from '../../terminal-presentation';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
import type {
	TerminalResolvedThemePreferences,
	TerminalThemePresentationCommit,
	TerminalThemePresentationParticipant,
	TerminalThemePresentationPreparation,
} from '../../terminal-settings';
import type { ProjectedTerminalTransport } from '../terminal-projection';

type ThemeTerminalDriver = Pick<
	ProjectedTerminal,
	| 'abortThemePresentation'
	| 'canCommitThemePresentation'
	| 'commitThemePresentation'
	| 'prepareThemePresentation'
	| 'waitForThemePresentation'
>;

export interface TerminalThemePresentationParticipantSession extends TerminalThemePresentationParticipant {
	dispose(): void;
	handleEvent(event: ProjectedTerminalEvent): boolean;
}

interface PendingPreparation {
	hostReconciliationPending: boolean;
	readonly operationId: TerminalThemeOperationId;
	readonly resolve: (result: TerminalThemePresentationPreparation) => void;
	settled: boolean;
}

const LIGHT_DEFAULTS = Object.freeze({ background: 0xfc_fc_fc, foreground: 0x14_14_14 });
const DARK_DEFAULTS = Object.freeze({ background: 0x1e_1e_1e, foreground: 0xd4_d4_d4 });
const SELECTION_ACCENT_RGB = 0x76_a7_fa;
const SELECTION_ACCENT_WEIGHT = 2;
const SELECTION_BACKGROUND_WEIGHT = 3;
const SELECTION_BLEND_WEIGHT = SELECTION_ACCENT_WEIGHT + SELECTION_BACKGROUND_WEIGHT;

export function createTerminalThemePresentationParticipant(options: {
	readonly bootstrap: ProjectedAttachmentBootstrap;
	readonly createOperationId?: () => string;
	readonly onPresented: () => void;
	readonly terminal: ThemeTerminalDriver;
	readonly transport: ProjectedTerminalTransport;
}): TerminalThemePresentationParticipantSession {
	const baselineSnapshot = normalizeTerminalThemeSnapshot({
		application: { colorScheme: options.bootstrap.theme.terminal.colorScheme },
		overlays: clientThemeOverlays(options.bootstrap.theme.terminal),
		terminal: options.bootstrap.theme.terminal,
	});
	let state = createTerminalThemeRevisionStateFromHost(
		baselineSnapshot,
		normalizeTerminalThemeHostState(options.bootstrap.theme),
	);
	const createOperationId = options.createOperationId ?? createBrowserRandomIdentifier;
	let disposed = false;
	let pending: PendingPreparation | undefined;

	const abortPending = (): void => {
		const active = pending;
		if (active === undefined) return;
		pending = undefined;
		options.terminal.abortThemePresentation(active.operationId);
		if (!active.settled) {
			active.settled = true;
			active.resolve({ kind: 'cancelled' });
		}
	};

	const failPending = (
		operationId: TerminalThemeOperationId,
		failure: TerminalThemeFailure,
	): void => {
		const active = pending;
		if (active?.operationId !== operationId) return;
		pending = undefined;
		options.terminal.abortThemePresentation(operationId);
		if (active.settled) return;
		active.settled = true;
		active.resolve({
			code: failure.code,
			kind: 'failed',
			retryable: failure.retryable,
		});
	};

	const applyEvent = (
		event: TerminalThemeRevisionEvent,
	): ReturnType<typeof transitionTerminalThemeRevision> => {
		const transition = transitionTerminalThemeRevision(state, event);
		state = transition.state;
		if (transition.kind === 'rejected') {
			const operationId = 'operationId' in event ? event.operationId : state.active?.operationId;
			if (operationId !== undefined) {
				failPending(operationId, {
					code: transition.reason,
					outcome: 'known-unchanged',
					retryable: false,
				});
			}
			return transition;
		}
		driveCommands(transition.commands);
		return transition;
	};

	const preparedCommit = (
		operationId: TerminalThemeOperationId,
		revision: bigint,
	): TerminalThemePresentationCommit => {
		let active = true;
		return {
			abort: () => {
				if (!active) return;
				active = false;
				// Host already committed this revision. Keep its frame quarantined until a
				// replacement operation or disposal supersedes the terminal stage.
				if (state.active?.operationId === operationId) {
					applyEvent({
						failure: {
							code: 'application-theme-barrier-aborted',
							outcome: 'known-unchanged',
							retryable: true,
						},
						operationId,
						type: 'presentation-failed',
					});
				}
			},
			canCommit: () =>
				active &&
				!disposed &&
				state.active?.operationId === operationId &&
				options.terminal.canCommitThemePresentation(operationId, revision),
			commit: () => {
				if (
					!active ||
					disposed ||
					!options.terminal.commitThemePresentation(operationId, revision)
				) {
					throw new Error('Prepared terminal theme publication lost eligibility');
				}
				active = false;
				if (pending?.operationId === operationId) pending = undefined;
				applyEvent({
					operationId,
					revision: state.host.revision,
					type: 'presentation-committed',
				});
				options.onPresented();
			},
		};
	};

	const waitForPublication = (
		command: Extract<TerminalThemeRevisionCommand, { readonly type: 'publish-theme' }>,
	): void => {
		void options.terminal
			.waitForThemePresentation(command.operationId, BigInt(command.revision))
			.then((result) => {
				const active = pending;
				if (disposed || active?.operationId !== command.operationId || active.settled) {
					return;
				}
				if (result.kind === 'ready') {
					active.settled = true;
					active.resolve({
						commit: preparedCommit(command.operationId, BigInt(command.revision)),
						kind: 'prepared',
					});
					return;
				}
				if (result.kind === 'cancelled') {
					pending = undefined;
					active.settled = true;
					active.resolve({ kind: 'cancelled' });
					return;
				}
				applyEvent({
					failure: {
						code: result.code,
						outcome: 'known-unchanged',
						retryable: result.retryable,
					},
					operationId: command.operationId,
					type: 'presentation-failed',
				});
				failPending(command.operationId, {
					code: result.code,
					outcome: 'known-unchanged',
					retryable: result.retryable,
				});
			});
	};

	function driveCommands(commands: readonly TerminalThemeRevisionCommand[]): void {
		for (const command of commands) {
			if (disposed || pending?.operationId !== command.operationId) continue;
			switch (command.type) {
				case 'apply-host-theme':
					try {
						options.transport.send({
							kind: 'apply-terminal-theme',
							payload: {
								attachmentId: options.bootstrap.attachmentId,
								expectedThemeRevision: CanonicalUint64TextSchema.parse(command.expectedRevision),
								operationId: command.operationId,
								terminal: { ...command.terminal, palette: [...command.terminal.palette] },
								terminalId: options.bootstrap.terminalId,
							},
						});
					} catch {
						const failure = {
							code: 'terminal-theme-transport-failed',
							outcome: 'known-unchanged',
							retryable: true,
						} as const;
						applyEvent({
							failure,
							operationId: command.operationId,
							type: 'host-failed',
						});
						failPending(command.operationId, failure);
					}
					break;
				case 'prepare-client-theme': {
					const result = options.terminal.prepareThemePresentation(
						command.operationId,
						command.snapshot.overlays,
					);
					if (result.kind === 'prepared') {
						applyEvent({ operationId: command.operationId, type: 'client-prepared' });
					} else {
						applyEvent({
							failure: {
								code: result.code,
								outcome: 'known-unchanged',
								retryable: result.retryable,
							},
							operationId: command.operationId,
							type: 'client-failed',
						});
						failPending(command.operationId, {
							code: result.code,
							outcome: 'known-unchanged',
							retryable: result.retryable,
						});
					}
					break;
				}
				case 'publish-theme':
					waitForPublication(command);
					break;
				case 'reconcile-host-theme':
					if (pending !== undefined) pending.hostReconciliationPending = true;
					try {
						options.transport.send({
							kind: 'reconcile-terminal-theme',
							payload: {
								attachmentId: options.bootstrap.attachmentId,
								operationId: command.operationId,
								terminalId: options.bootstrap.terminalId,
							},
						});
					} catch {
						failPending(command.operationId, {
							code: 'terminal-theme-reconciliation-failed',
							outcome: 'unknown',
							retryable: true,
						});
					}
					break;
			}
		}
	}

	const retryCurrentOperation = (operationId: TerminalThemeOperationId): void => {
		if (pending?.operationId !== operationId) return;
		applyEvent({ operationId, type: 'retry' });
	};

	return {
		dispose: () => {
			if (disposed) return;
			disposed = true;
			abortPending();
		},
		handleEvent: (event) => {
			switch (event.kind) {
				case 'terminal-theme-result': {
					if (event.event.terminalId !== options.bootstrap.terminalId) return true;
					const operationId = terminalThemeOperationId(event.event.operationId);
					if (pending?.operationId !== operationId) return true;
					const host = normalizeTerminalThemeHostState(event.event.current);
					if (event.event.transition === 'stale') {
						applyEvent({ current: host, operationId, type: 'host-stale' });
						retryCurrentOperation(operationId);
					} else {
						applyEvent({ operationId, state: host, type: 'host-committed' });
					}
					return true;
				}
				case 'terminal-theme-reconciled': {
					if (event.terminalId !== options.bootstrap.terminalId) return true;
					const operationId = terminalThemeOperationId(event.operationId);
					if (pending?.operationId !== operationId) return true;
					pending.hostReconciliationPending = false;
					applyEvent({
						current: normalizeTerminalThemeHostState(event.current),
						operationId,
						type: 'host-reconciled',
					});
					if (state.active?.operationId === operationId && state.active.host.kind === 'stale') {
						retryCurrentOperation(operationId);
					}
					return true;
				}
				case 'terminal-theme-failed': {
					const operationId = terminalThemeOperationId(event.operationId);
					if (pending?.operationId !== operationId) return true;
					if (pending.hostReconciliationPending) {
						failPending(operationId, event.failure);
					} else if (event.failure.outcome === 'unknown') {
						applyEvent({ operationId, type: 'host-outcome-unknown' });
						retryCurrentOperation(operationId);
					} else {
						applyEvent({ failure: event.failure, operationId, type: 'host-failed' });
						failPending(operationId, event.failure);
					}
					return true;
				}
				default:
					return false;
			}
		},
		prepareThemePresentation: (preferences) => {
			if (disposed) return Promise.resolve({ kind: 'cancelled' });
			abortPending();
			const snapshot = snapshotFromPreferences(
				preferences,
				state,
				options.bootstrap.hostDefaultColors,
			);
			const operationId = terminalThemeOperationId(createOperationId());
			const transition = transitionTerminalThemeRevision(state, {
				operationId,
				snapshot,
				type: 'request',
			});
			state = transition.state;
			if (transition.kind === 'unchanged' && transition.reason === 'already-current') {
				let active = true;
				return Promise.resolve({
					commit: {
						abort: () => {
							active = false;
						},
						canCommit: () => active && !disposed,
						commit: () => {
							if (!active || disposed) {
								throw new Error('Baseline terminal theme publication lost eligibility');
							}
							active = false;
							options.onPresented();
						},
					},
					kind: 'prepared',
				});
			}
			if (transition.kind === 'rejected' || transition.kind === 'unchanged') {
				return Promise.resolve({
					code: transition.reason,
					kind: 'failed',
					retryable: false,
				});
			}
			return new Promise<TerminalThemePresentationPreparation>((resolve) => {
				pending = {
					hostReconciliationPending: false,
					operationId,
					resolve,
					settled: false,
				};
				driveCommands(transition.commands);
			});
		},
	};
}

function snapshotFromPreferences(
	preferences: TerminalResolvedThemePreferences,
	state: TerminalThemeRevisionState,
	hostDefaultColors: ProjectedAttachmentBootstrap['hostDefaultColors'],
): TerminalThemeSnapshot {
	const current = state.host.terminal;
	let terminal: TerminalThemeSnapshot['terminal'];
	switch (preferences.terminalDisplayTheme) {
		case 'host':
			terminal = { ...current, ...hostDefaultColors };
			break;
		case 'theme': {
			const defaults = preferences.applicationTheme === 'dark' ? DARK_DEFAULTS : LIGHT_DEFAULTS;
			terminal = {
				...current,
				colorScheme: preferences.applicationTheme,
				defaultBackgroundRgb: defaults.background,
				defaultForegroundRgb: defaults.foreground,
			};
			break;
		}
		case 'custom':
			terminal = {
				...current,
				defaultBackgroundRgb: hexRgb(preferences.customBackground),
				defaultForegroundRgb: hexRgb(preferences.customForeground),
			};
			break;
	}
	return normalizeTerminalThemeSnapshot({
		application: { colorScheme: preferences.applicationTheme },
		overlays: clientThemeOverlays(terminal),
		terminal,
	});
}

function clientThemeOverlays(
	terminal: Pick<
		TerminalThemeSnapshot['terminal'],
		'defaultBackgroundRgb' | 'defaultForegroundRgb'
	>,
): TerminalThemeOverlayPresentation {
	return Object.freeze({
		cursorAccentRgb: 0xfc_fc_fc,
		searchActiveMatchBackgroundRgb: 0xf5_a9_7f,
		searchActiveMatchForegroundRgb: 0x1e_1e_1e,
		searchMatchBackgroundRgb: 0x51_5c_6a,
		searchMatchForegroundRgb: 0xff_ff_ff,
		selectionBackgroundRgb: compositeSelectionBackground(terminal.defaultBackgroundRgb),
		selectionForegroundRgb: terminal.defaultForegroundRgb,
	});
}

function compositeSelectionBackground(backgroundRgb: number): number {
	const red = compositeSelectionChannel(SELECTION_ACCENT_RGB >>> 16, backgroundRgb >>> 16);
	const green = compositeSelectionChannel(SELECTION_ACCENT_RGB >>> 8, backgroundRgb >>> 8);
	const blue = compositeSelectionChannel(SELECTION_ACCENT_RGB, backgroundRgb);
	return (red << 16) | (green << 8) | blue;
}

function compositeSelectionChannel(accentRgb: number, backgroundRgb: number): number {
	const accent = accentRgb & 0xff;
	const background = backgroundRgb & 0xff;
	return Math.round(
		(accent * SELECTION_ACCENT_WEIGHT + background * SELECTION_BACKGROUND_WEIGHT) /
			SELECTION_BLEND_WEIGHT,
	);
}

function hexRgb(value: string): number {
	return Number.parseInt(value.slice(1), 16);
}
