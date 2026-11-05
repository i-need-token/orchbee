import {
	captureKeyboardShortcutChord,
	formatKeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import { keyboardShortcutMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import {
	INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE,
	KEYBOARD_SHORTCUT_CATALOG,
	formatKeyboardShortcutActionName,
	keyboardShortcutSearchQueryValue,
	keyboardShortcutBindingSource,
	keyboardShortcutDeclaration,
	resolveKeyboardShortcutSearch,
	transitionKeyboardShortcutSearch,
	useKeyboardShortcutSettings,
	type KeyboardShortcutAvailability,
	type KeyboardShortcutDeclaration,
	type KeyboardShortcutId,
	type KeyboardShortcutSearchEvent,
	type KeyboardShortcutSearchState,
	type KeyboardShortcutScope,
	type KeyboardShortcutSettingsController,
} from '../../../../keyboard-shortcuts';

interface KeyboardShortcutSettingsRowView {
	readonly actionName: string;
	readonly availability: string;
	readonly chord: string;
	readonly error: string | null;
	readonly recording: boolean;
	readonly resetLabel: string;
	readonly scope: string;
	readonly shortcutId: KeyboardShortcutId;
	readonly source: string;
	readonly sourceKind: 'custom' | 'default';
	readonly onBlur: React.FocusEventHandler<HTMLButtonElement>;
	readonly onChangeClick: React.MouseEventHandler<HTMLButtonElement>;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLButtonElement>;
	readonly onReset: React.MouseEventHandler<HTMLButtonElement>;
}

interface KeyboardShortcutSettingsView {
	readonly changeLabel: (action: string) => string;
	readonly noResults: string;
	readonly persistenceWarning: string | null;
	readonly recordingInstruction: string;
	readonly rows: readonly KeyboardShortcutSettingsRowView[];
	readonly search: KeyboardShortcutSearchView;
}

interface KeyboardShortcutSearchView {
	readonly clearLabel: string;
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
	readonly status: string | null;
}

interface UseKeyboardShortcutSettingsViewOptions {
	readonly controller: KeyboardShortcutSettingsController;
	readonly platform: KeyboardShortcutPlatform;
}

export function useKeyboardShortcutSettingsView({
	controller,
	platform,
}: UseKeyboardShortcutSettingsViewOptions): KeyboardShortcutSettingsView {
	const intl = useOrchbeeIntl();
	const actionNameFor = (declaration: KeyboardShortcutDeclaration): string =>
		formatKeyboardShortcutActionName(intl, declaration);
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
	const snapshot = useKeyboardShortcutSettings(controller);
	const [recordingShortcutId, setRecordingShortcutId] = React.useState<KeyboardShortcutId | null>(
		null,
	);
	const searchInputReference = React.useRef<HTMLInputElement>(null);
	const [searchState, dispatchSearch] = React.useReducer(
		(current: KeyboardShortcutSearchState, event: KeyboardShortcutSearchEvent) =>
			transitionKeyboardShortcutSearch(current, event, platform),
		INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE,
	);
	const [errors, setErrors] = React.useState<Readonly<Partial<Record<KeyboardShortcutId, string>>>>(
		{},
	);
	const sourceLabel = (source: 'custom' | 'default'): string =>
		intl.formatMessage(
			source === 'custom'
				? keyboardShortcutMessages.sourceCustom
				: keyboardShortcutMessages.sourceDefault,
		);
	const projectedRows = KEYBOARD_SHORTCUT_CATALOG.map((declaration) => {
		const actionName = actionNameFor(declaration);
		const scope = scopeNameFor(declaration.scope);
		const availability = availabilityNameFor(declaration.availability);
		const binding = snapshot.bindings[declaration.shortcutId];
		const chord = formatKeyboardShortcutChord(binding, platform);
		const sourceKind = keyboardShortcutBindingSource(snapshot, declaration.shortcutId);
		return {
			actionName,
			binding,
			availability,
			chord,
			declaration,
			scope,
			source: sourceLabel(sourceKind),
			sourceKind,
		};
	});
	const searchResult = resolveKeyboardShortcutSearch(
		projectedRows.map((row) => ({
			actionId: row.declaration.actionId,
			actionName: row.actionName,
			availability: row.availability,
			chord: row.binding,
			scope: row.scope,
			shortcutId: row.declaration.shortcutId,
			source: row.source,
		})),
		searchState.query,
		platform,
	);
	const matchingShortcutIds = new Set(searchResult.matchingShortcutIds);
	const rows = projectedRows
		.filter((row) => matchingShortcutIds.has(row.declaration.shortcutId))
		.map((row) => {
			const { actionName, availability, chord, declaration, scope, source, sourceKind } = row;
			return {
				actionName,
				availability,
				chord,
				error: errors[declaration.shortcutId] ?? null,
				recording: recordingShortcutId === declaration.shortcutId,
				resetLabel: intl.formatMessage(keyboardShortcutMessages.reset),
				scope,
				shortcutId: declaration.shortcutId,
				source,
				sourceKind,
				onBlur: (): void => {
					setRecordingShortcutId((current) =>
						current === declaration.shortcutId ? null : current,
					);
				},
				onChangeClick: (): void => {
					dispatchSearch({ type: 'recording-stopped' });
					setErrors((current) => ({ ...current, [declaration.shortcutId]: undefined }));
					setRecordingShortcutId(declaration.shortcutId);
				},
				onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>): void => {
					if (recordingShortcutId !== declaration.shortcutId) return;
					if (event.key === 'Tab') {
						setRecordingShortcutId(null);
						return;
					}
					event.preventDefault();
					event.stopPropagation();
					if (event.key === 'Escape') {
						setRecordingShortcutId(null);
						return;
					}
					const captured = captureKeyboardShortcutChord(event.nativeEvent, platform);
					if (captured.kind === 'ignored') {
						if (captured.reason === 'missing-modifier' || captured.reason === 'unsupported-code') {
							setErrors((current) => ({
								...current,
								[declaration.shortcutId]: intl.formatMessage(keyboardShortcutMessages.invalid),
							}));
						}
						return;
					}
					const result = controller.setShortcut(declaration.shortcutId, captured.chord);
					if (result.kind === 'conflict') {
						const conflictDeclaration = keyboardShortcutDeclaration(
							result.conflict.conflictingShortcutId,
						);
						const conflictSource = keyboardShortcutBindingSource(
							snapshot,
							result.conflict.conflictingShortcutId,
						);
						setErrors((current) => ({
							...current,
							[declaration.shortcutId]: intl.formatMessage(keyboardShortcutMessages.conflict, {
								action: actionNameFor(conflictDeclaration),
								availability: availabilityNameFor(conflictDeclaration.availability),
								scope: scopeNameFor(conflictDeclaration.scope),
								shortcut: formatKeyboardShortcutChord(result.conflict.chord, platform),
								source: sourceLabel(conflictSource),
							}),
						}));
						return;
					}
					if (result.kind === 'invalid') {
						setErrors((current) => ({
							...current,
							[declaration.shortcutId]: intl.formatMessage(keyboardShortcutMessages.invalid),
						}));
						return;
					}
					setErrors((current) => ({ ...current, [declaration.shortcutId]: undefined }));
					setRecordingShortcutId(null);
				},
				onReset: (): void => {
					controller.resetShortcut(declaration.shortcutId);
					setErrors((current) => ({ ...current, [declaration.shortcutId]: undefined }));
					setRecordingShortcutId(null);
				},
			};
		});

	const searchQuery = keyboardShortcutSearchQueryValue(searchState.query, platform);
	const hasSearchQuery =
		searchState.query.kind === 'chord' || searchState.query.value.trim().length > 0;
	const searchResultCount = intl.formatMessage(keyboardShortcutMessages.searchResultCount, {
		count: searchResult.matchingShortcutIds.length,
	});
	let searchStatus: string | null = hasSearchQuery ? searchResultCount : null;
	if (searchState.mode === 'recording') {
		searchStatus = [
			intl.formatMessage(keyboardShortcutMessages.searchRecording),
			hasSearchQuery ? searchResultCount : null,
		]
			.filter((part): part is string => part !== null)
			.join(' ');
	}
	if (searchState.recordingIssue === 'invalid-chord') {
		searchStatus = intl.formatMessage(keyboardShortcutMessages.invalid);
	}

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
		recordingInstruction: intl.formatMessage(keyboardShortcutMessages.recording),
		rows,
		search: {
			clearLabel: intl.formatMessage(keyboardShortcutMessages.searchClear),
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
				setRecordingShortcutId(null);
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
