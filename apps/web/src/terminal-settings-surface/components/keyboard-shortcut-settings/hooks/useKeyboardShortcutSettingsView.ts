import {
	KEYBOARD_SHORTCUT_SEQUENCE_LEADER,
	captureKeyboardShortcutChord,
	formatKeyboardShortcutChord,
	formatKeyboardShortcutSequence,
	keyboardShortcutChordKey,
	type KeyboardShortcutChord,
	type KeyboardShortcutEvent,
	type KeyboardShortcutPlatform,
	type KeyboardShortcutSequence,
} from '@orchbee/keyboard-shortcuts';
import { keyboardShortcutMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import {
	INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE,
	findKeyboardShortcutScopedShadow,
	formatKeyboardShortcutActionName,
	isKeyboardShortcutId,
	keyboardShortcutSearchQueryValue,
	keyboardShortcutBindingSource,
	keyboardShortcutDeclaration,
	keyboardShortcutScopedDeclaration,
	keyboardShortcutScopedRecording,
	keyboardShortcutScopedSequence,
	resolveKeyboardShortcutSearch,
	transitionKeyboardShortcutSearch,
	useKeyboardShortcutSettings,
	type KeyboardShortcutAnyId,
	type KeyboardShortcutAvailability,
	type KeyboardShortcutDeclaration,
	type KeyboardShortcutScopedDeclaration,
	type KeyboardShortcutSearchEvent,
	type KeyboardShortcutSearchState,
	type KeyboardShortcutScope,
	type KeyboardShortcutSettingsController,
} from '../../../../keyboard-shortcuts';

interface KeyboardShortcutSettingsRowView {
	readonly actionName: string;
	readonly availability: string;
	readonly chord: string;
	/** Where a File Bar family shortcut applies; a window shortcut's line says nothing more. */
	readonly description: string | null;
	readonly error: string | null;
	/**
	 * Says which configured command runs this shortcut's default keys instead of it, or which wider
	 * shortcut's keys this one takes while focus is inside its scope.
	 */
	readonly note: string | null;
	readonly recording: boolean;
	/** The full recording instruction, spoken while the line records. */
	readonly recordingInstruction: string;
	/** The few words a recording line shows in place of its keys. */
	readonly recordingPrompt: string;
	readonly resetLabel: string;
	readonly scope: string;
	readonly shortcutId: KeyboardShortcutAnyId;
	readonly source: string;
	readonly sourceKind: 'custom' | 'default';
	readonly onBlur: React.FocusEventHandler<HTMLButtonElement>;
	readonly onChangeClick: React.MouseEventHandler<HTMLButtonElement>;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLButtonElement>;
	readonly onReset: () => void;
}

interface KeyboardShortcutSettingsView {
	readonly changeLabel: (action: string) => string;
	readonly noResults: string;
	readonly persistenceWarning: string | null;
	readonly rows: readonly KeyboardShortcutSettingsRowView[];
	readonly search: KeyboardShortcutSearchView;
}

interface KeyboardShortcutSearchView {
	readonly clearLabel: string;
	/** Why the last recorded chord was refused, shown on the dialog's notice line. */
	readonly failure: string | null;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly label: string;
	readonly onChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onClear: React.MouseEventHandler<HTMLButtonElement>;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onRecordingToggle: React.MouseEventHandler<HTMLButtonElement>;
	readonly placeholder: string;
	readonly query: string;
	readonly recording: boolean;
	readonly recordingToggleLabel: string;
	/** What the filter's live region speaks: recording and the match count, or nothing. */
	readonly status: string;
}

interface UseKeyboardShortcutSettingsViewOptions {
	readonly controller: KeyboardShortcutSettingsController;
	readonly platform: KeyboardShortcutPlatform;
	/** The configured command holding each default shortcut it took, by shortcut id. */
	readonly takenByConfiguredCommands?: ReadonlyMap<string, string> | undefined;
}

type KeyboardShortcutRowTarget =
	| { readonly declaration: KeyboardShortcutDeclaration; readonly kind: 'window' }
	| { readonly declaration: KeyboardShortcutScopedDeclaration; readonly kind: 'scoped' };

type ScopedShortcutScope = KeyboardShortcutScopedDeclaration['definition']['scope'];

/** The line that records, and the `Mod+K` it holds while it waits for a sequence's second step. */
interface KeyboardShortcutRecording {
	readonly leader: KeyboardShortcutChord | null;
	readonly shortcutId: KeyboardShortcutAnyId;
}

type RecordedKeys =
	| { readonly keys: KeyboardShortcutSequence; readonly kind: 'keys' }
	| { readonly kind: 'ignored' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'leader'; readonly leader: KeyboardShortcutChord };

export function useKeyboardShortcutSettingsView({
	controller,
	platform,
	takenByConfiguredCommands,
}: UseKeyboardShortcutSettingsViewOptions): KeyboardShortcutSettingsView {
	const intl = useOrchbeeIntl();
	const actionNameFor = (
		declaration: KeyboardShortcutDeclaration | KeyboardShortcutScopedDeclaration,
	): string => formatKeyboardShortcutActionName(intl, declaration);
	const scopeNameFor = (scope: KeyboardShortcutScope): string => {
		switch (scope) {
			case 'active-panel':
				return intl.formatMessage(keyboardShortcutMessages.scopeActivePanel);
			case 'active-terminal':
				return intl.formatMessage(keyboardShortcutMessages.scopeActiveTerminal);
			case 'workbench':
				return intl.formatMessage(keyboardShortcutMessages.scopeWorkbench);
			case 'focused-terminal':
				return intl.formatMessage(keyboardShortcutMessages.scopeFocusedTerminal);
			case 'focused-terminal-or-find':
				return intl.formatMessage(keyboardShortcutMessages.scopeFocusedTerminalOrFind);
		}
	};
	const scopedScopeNameFor = (scope: ScopedShortcutScope): string => {
		switch (scope) {
			case 'shell':
				return intl.formatMessage(keyboardShortcutMessages.scopeWorkbench);
			case 'file-bar':
				return intl.formatMessage(keyboardShortcutMessages.scopeFileBar);
			case 'file-overlay':
				return intl.formatMessage(keyboardShortcutMessages.scopeFileOverlay);
			case 'code-editor':
				return intl.formatMessage(keyboardShortcutMessages.scopeCodeEditor);
			case 'quick-input':
				return intl.formatMessage(keyboardShortcutMessages.scopeQuickInput);
			default:
				return intl.formatMessage(keyboardShortcutMessages.scopeWidget);
		}
	};
	const availabilityNameFor = (availability: KeyboardShortcutAvailability): string => {
		switch (availability) {
			case 'active-panel':
				return intl.formatMessage(keyboardShortcutMessages.availabilityActivePanel);
			case 'active-terminal':
				return intl.formatMessage(keyboardShortcutMessages.availabilityActiveTerminal);
			case 'always-in-workbench':
				return intl.formatMessage(keyboardShortcutMessages.availabilityWorkbench);
			case 'focused-terminal':
				return intl.formatMessage(keyboardShortcutMessages.availabilityFocusedTerminal);
			case 'focused-terminal-with-selection':
				return intl.formatMessage(
					keyboardShortcutMessages.availabilityFocusedTerminalWithSelection,
				);
			case 'directional-panel':
				return intl.formatMessage(keyboardShortcutMessages.availabilityDirectionalPanel);
			case 'multiple-panels':
				return intl.formatMessage(keyboardShortcutMessages.availabilityMultiplePanels);
			case 'last-panel':
				return intl.formatMessage(keyboardShortcutMessages.availabilityLastPanel);
			case 'multiple-tabs':
				return intl.formatMessage(keyboardShortcutMessages.availabilityMultipleTabs);
			case 'movable-tab':
				return intl.formatMessage(keyboardShortcutMessages.availabilityMovableTab);
			case 'panel-position':
				return intl.formatMessage(keyboardShortcutMessages.availabilityPanelPosition);
			case 'tab-position':
				return intl.formatMessage(keyboardShortcutMessages.availabilityTabPosition);
		}
	};
	const scopedAvailabilityNameFor = (scope: ScopedShortcutScope): string => {
		switch (scope) {
			case 'shell':
				return intl.formatMessage(keyboardShortcutMessages.availabilityWorkbench);
			case 'file-bar':
				return intl.formatMessage(keyboardShortcutMessages.availabilityFileBar);
			case 'file-overlay':
				return intl.formatMessage(keyboardShortcutMessages.availabilityFileOverlay);
			case 'code-editor':
				return intl.formatMessage(keyboardShortcutMessages.availabilityCodeEditor);
			case 'quick-input':
				return intl.formatMessage(keyboardShortcutMessages.availabilityQuickInput);
			default:
				return intl.formatMessage(keyboardShortcutMessages.availabilityWidget);
		}
	};
	const snapshot = useKeyboardShortcutSettings(controller);
	const [recording, setRecording] = React.useState<KeyboardShortcutRecording | null>(null);
	const searchInputReference = React.useRef<HTMLInputElement>(null);
	const [searchState, dispatchSearch] = React.useReducer(
		(current: KeyboardShortcutSearchState, event: KeyboardShortcutSearchEvent) =>
			transitionKeyboardShortcutSearch(current, event, platform),
		INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE,
	);
	const [errors, setErrors] = React.useState<Readonly<Partial<Record<string, string>>>>({});
	const sourceLabel = (source: 'custom' | 'default'): string =>
		intl.formatMessage(
			source === 'custom'
				? keyboardShortcutMessages.sourceCustom
				: keyboardShortcutMessages.sourceDefault,
		);
	const describeShortcut = (
		shortcutId: KeyboardShortcutAnyId,
	): { readonly action: string; readonly availability: string; readonly scope: string } | null => {
		const scoped = keyboardShortcutScopedDeclaration(shortcutId, platform);
		if (scoped !== undefined) {
			return {
				action: actionNameFor(scoped),
				availability: scopedAvailabilityNameFor(scoped.definition.scope),
				scope: scopedScopeNameFor(scoped.definition.scope),
			};
		}
		if (!isKeyboardShortcutId(shortcutId)) return null;
		const declaration = keyboardShortcutDeclaration(shortcutId);
		return {
			action: actionNameFor(declaration),
			availability: availabilityNameFor(declaration.availability),
			scope: scopeNameFor(declaration.scope),
		};
	};
	const windowRows = controller.declarations.map((declaration) => {
		const binding = snapshot.bindings[declaration.shortcutId];
		const configuredCommand = takenByConfiguredCommands?.get(declaration.shortcutId);
		const chord = formatKeyboardShortcutChord(binding, platform);
		return {
			actionName: actionNameFor(declaration),
			availability: availabilityNameFor(declaration.availability),
			chord,
			description: null,
			note:
				configuredCommand === undefined
					? null
					: intl.formatMessage(keyboardShortcutMessages.takenByConfiguredCommand, {
							command: configuredCommand,
							shortcut: chord,
						}),
			scope: scopeNameFor(declaration.scope),
			steps: [binding],
			target: { declaration, kind: 'window' } satisfies KeyboardShortcutRowTarget,
		};
	});
	const scopedRows = controller.scopedDeclarations.map((declaration) => {
		const sequence = keyboardShortcutScopedSequence(declaration, snapshot.scopedOverrides);
		const chord = formatKeyboardShortcutSequence(sequence, platform);
		const availability = scopedAvailabilityNameFor(declaration.definition.scope);
		const shadowedShortcutId = findKeyboardShortcutScopedShadow(
			snapshot.bindings,
			snapshot.scopedOverrides,
			declaration,
			platform,
		);
		const shadowed = shadowedShortcutId === null ? null : describeShortcut(shadowedShortcutId);
		return {
			actionName: actionNameFor(declaration),
			availability,
			chord,
			description: availability,
			note:
				shadowed === null
					? null
					: intl.formatMessage(keyboardShortcutMessages.takesWiderShortcut, {
							action: shadowed.action,
							availability,
							shortcut: chord,
						}),
			scope: scopedScopeNameFor(declaration.definition.scope),
			steps: sequence,
			target: { declaration, kind: 'scoped' } satisfies KeyboardShortcutRowTarget,
		};
	});
	const projectedRows = [...windowRows, ...scopedRows].map((row) => {
		const shortcutId = row.target.declaration.shortcutId;
		const sourceKind = keyboardShortcutBindingSource(snapshot, shortcutId);
		return { ...row, shortcutId, source: sourceLabel(sourceKind), sourceKind };
	});
	const searchResult = resolveKeyboardShortcutSearch(
		projectedRows.map((row) => ({
			actionId:
				row.target.kind === 'window'
					? row.target.declaration.actionId
					: row.target.declaration.definition.actionId,
			actionName: row.actionName,
			availability: row.availability,
			scope: row.scope,
			shortcutId: row.shortcutId,
			source: row.source,
			steps: row.steps,
		})),
		searchState.query,
		platform,
	);
	const matchingShortcutIds = new Set(searchResult.matchingShortcutIds);
	const leaderLabel = formatKeyboardShortcutChord(KEYBOARD_SHORTCUT_SEQUENCE_LEADER, platform);
	const rows = projectedRows
		.filter((row) => matchingShortcutIds.has(row.shortcutId))
		.map((row) => {
			const { actionName, availability, chord, description, note, scope, shortcutId } = row;
			const { source, sourceKind } = row;
			const target: KeyboardShortcutRowTarget = row.target;
			const rowRecording = recording?.shortcutId === shortcutId ? recording : null;
			const sequences =
				target.kind === 'scoped' && keyboardShortcutScopedRecording(target.declaration).sequences;
			const setError = (error: string | undefined): void => {
				setErrors((current) => ({ ...current, [shortcutId]: error }));
			};
			return {
				actionName,
				availability,
				chord,
				description,
				error: errors[shortcutId] ?? null,
				note,
				recording: rowRecording !== null,
				recordingInstruction: sequences
					? intl.formatMessage(keyboardShortcutMessages.recordingSequence, {
							leader: leaderLabel,
						})
					: intl.formatMessage(keyboardShortcutMessages.recording),
				recordingPrompt:
					rowRecording?.leader === null || rowRecording === null
						? intl.formatMessage(keyboardShortcutMessages.recordingPrompt)
						: intl.formatMessage(keyboardShortcutMessages.recordingSecondStepPrompt, {
								leader: formatKeyboardShortcutChord(rowRecording.leader, platform),
							}),
				resetLabel: intl.formatMessage(keyboardShortcutMessages.reset),
				scope,
				shortcutId,
				source,
				sourceKind,
				onBlur: (): void => {
					setRecording((current) => (current?.shortcutId === shortcutId ? null : current));
				},
				onChangeClick: (): void => {
					dispatchSearch({ type: 'recording-stopped' });
					setError(undefined);
					setRecording({ leader: null, shortcutId });
				},
				onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>): void => {
					if (rowRecording === null) return;
					if (event.key === 'Tab') {
						setRecording(null);
						return;
					}
					event.preventDefault();
					event.stopPropagation();
					if (event.key === 'Escape') {
						setRecording(null);
						return;
					}
					const recorded = recordKeys(target, rowRecording.leader, event.nativeEvent, platform);
					if (recorded.kind === 'ignored') return;
					if (recorded.kind === 'leader') {
						setRecording({ leader: recorded.leader, shortcutId });
						return;
					}
					if (recorded.kind === 'invalid') {
						setRecording({ leader: null, shortcutId });
						setError(
							intl.formatMessage(
								acceptsUnmodifiedKeys(target, rowRecording.leader)
									? keyboardShortcutMessages.invalidKey
									: keyboardShortcutMessages.invalid,
							),
						);
						return;
					}
					const [firstStep] = recorded.keys;
					const result =
						target.kind === 'window'
							? controller.setShortcut(target.declaration.shortcutId, firstStep)
							: controller.setScopedShortcut(target.declaration.shortcutId, recorded.keys);
					if (result.kind === 'conflict') {
						const conflicting = describeShortcut(result.conflict.conflictingShortcutId);
						setRecording({ leader: null, shortcutId });
						setError(
							conflicting === null
								? intl.formatMessage(keyboardShortcutMessages.invalid)
								: intl.formatMessage(keyboardShortcutMessages.conflict, {
										...conflicting,
										shortcut: formatKeyboardShortcutSequence(result.conflict.sequence, platform),
										source: sourceLabel(
											keyboardShortcutBindingSource(
												snapshot,
												result.conflict.conflictingShortcutId,
											),
										),
									}),
						);
						return;
					}
					if (result.kind === 'invalid') {
						setRecording({ leader: null, shortcutId });
						setError(intl.formatMessage(keyboardShortcutMessages.invalid));
						return;
					}
					setError(undefined);
					setRecording(null);
				},
				onReset: (): void => {
					if (target.kind === 'window') controller.resetShortcut(target.declaration.shortcutId);
					else controller.resetScopedShortcut(target.declaration.shortcutId);
					setError(undefined);
					setRecording(null);
				},
			};
		});

	const searchQuery = keyboardShortcutSearchQueryValue(searchState.query, platform);
	const hasSearchQuery =
		searchState.query.kind === 'chord' || searchState.query.value.trim().length > 0;
	const searchResultCount = intl.formatMessage(keyboardShortcutMessages.searchResultCount, {
		count: searchResult.matchingShortcutIds.length,
	});
	const searchStatus = [
		searchState.mode === 'recording'
			? intl.formatMessage(keyboardShortcutMessages.searchRecording)
			: null,
		hasSearchQuery ? searchResultCount : null,
	]
		.filter((part): part is string => part !== null)
		.join(' ');

	let persistenceWarning: string | null = null;
	switch (snapshot.persistenceIssue) {
		case 'invalid-record':
			persistenceWarning = intl.formatMessage(keyboardShortcutMessages.persistenceInvalidWarning);
			break;
		case 'read-failed':
			persistenceWarning = intl.formatMessage(keyboardShortcutMessages.persistenceReadWarning);
			break;
		case 'write-failed':
			persistenceWarning = intl.formatMessage(keyboardShortcutMessages.persistenceWriteWarning);
			break;
	}

	return {
		changeLabel: (action): string =>
			intl.formatMessage(keyboardShortcutMessages.change, { action }),
		noResults: intl.formatMessage(keyboardShortcutMessages.noResults),
		persistenceWarning,
		rows,
		search: {
			clearLabel: intl.formatMessage(keyboardShortcutMessages.searchClear),
			failure:
				searchState.recordingIssue === 'invalid-chord'
					? intl.formatMessage(keyboardShortcutMessages.invalid)
					: null,
			inputReference: searchInputReference,
			label: intl.formatMessage(keyboardShortcutMessages.searchPlaceholder),
			onChange: (event): void => {
				dispatchSearch({ type: 'text-changed', value: event.currentTarget.value });
			},
			onClear: (): void => {
				dispatchSearch({ type: 'cleared' });
				searchInputReference.current?.focus();
			},
			onKeyDown: (event): void => {
				if (searchState.mode === 'recording') {
					if (event.key === 'Tab') {
						dispatchSearch({ type: 'recording-stopped' });
						return;
					}
					event.preventDefault();
					event.stopPropagation();
					if (event.key === 'Escape') {
						dispatchSearch({ type: 'recording-stopped' });
						return;
					}
					const captured = captureKeyboardShortcutChord(event.nativeEvent, platform);
					if (captured.kind === 'captured') {
						dispatchSearch({ chord: captured.chord, type: 'shortcut-captured' });
						return;
					}
					if (captured.reason === 'missing-modifier' || captured.reason === 'unsupported-code') {
						dispatchSearch({ type: 'shortcut-rejected' });
					}
					return;
				}
				if (event.key === 'Escape' && hasSearchQuery) {
					event.preventDefault();
					event.stopPropagation();
					dispatchSearch({ type: 'cleared' });
				}
			},
			onRecordingToggle: (): void => {
				setRecording(null);
				dispatchSearch({
					type: searchState.mode === 'recording' ? 'recording-stopped' : 'recording-started',
				});
				searchInputReference.current?.focus();
			},
			placeholder: intl.formatMessage(
				searchState.mode === 'recording'
					? keyboardShortcutMessages.searchRecordingPlaceholder
					: keyboardShortcutMessages.searchPlaceholder,
			),
			query: searchQuery,
			recording: searchState.mode === 'recording',
			recordingToggleLabel: intl.formatMessage(
				searchState.mode === 'recording'
					? keyboardShortcutMessages.searchStopRecording
					: keyboardShortcutMessages.searchRecordKeys,
			),
			status: searchStatus,
		},
	};
}

/**
 * Turns one press on a recording line into keys. A window line takes one chord with a modifier. A
 * scoped line follows its scope's rule; where sequences apply, `Mod+K` waits for a second step,
 * which may be any key but Tab and Escape.
 */
function recordKeys(
	target: KeyboardShortcutRowTarget,
	leader: KeyboardShortcutChord | null,
	event: KeyboardShortcutEvent,
	platform: KeyboardShortcutPlatform,
): RecordedKeys {
	if (target.kind === 'window') {
		const captured = captureKeyboardShortcutChord(event, platform);
		return captured.kind === 'captured'
			? { keys: [captured.chord], kind: 'keys' }
			: refusedPress(captured.reason);
	}
	if (leader !== null) {
		const captured = captureKeyboardShortcutChord(event, platform, 'second-step');
		return captured.kind === 'captured'
			? { keys: [leader, captured.chord], kind: 'keys' }
			: refusedPress(captured.reason);
	}
	const { rule, sequences } = keyboardShortcutScopedRecording(target.declaration);
	const captured = captureKeyboardShortcutChord(event, platform, rule);
	if (captured.kind !== 'captured') return refusedPress(captured.reason);
	return sequences &&
		keyboardShortcutChordKey(captured.chord, platform) ===
			keyboardShortcutChordKey(KEYBOARD_SHORTCUT_SEQUENCE_LEADER, platform)
		? { kind: 'leader', leader: captured.chord }
		: { keys: [captured.chord], kind: 'keys' };
}

/** A modifier press or a repeat waits for the next key; an unusable key is refused aloud. */
function refusedPress(
	reason: 'composing' | 'missing-modifier' | 'modifier-only' | 'repeat' | 'unsupported-code',
): RecordedKeys {
	return reason === 'missing-modifier' || reason === 'unsupported-code'
		? { kind: 'invalid' }
		: { kind: 'ignored' };
}

function acceptsUnmodifiedKeys(
	target: KeyboardShortcutRowTarget,
	leader: KeyboardShortcutChord | null,
): boolean {
	return (
		target.kind === 'scoped' &&
		(leader !== null || keyboardShortcutScopedRecording(target.declaration).rule === 'widget')
	);
}
