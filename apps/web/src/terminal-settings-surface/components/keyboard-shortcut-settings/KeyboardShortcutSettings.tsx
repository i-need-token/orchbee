import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import {
	Button,
	InputGroup,
	InputGroupAddon,
	InputGroupButton,
	InputGroupInput,
	Kbd,
	ProductIcon,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { KeyboardShortcutSettingsController } from '../../../keyboard-shortcuts';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SETTINGS_LIST_FILTER_THRESHOLD } from '../../constants/settings-list-filter';
import { SettingsListFilterEmpty } from '../settings-list-filter-empty/SettingsListFilterEmpty';
import { SettingsRow } from '../settings-row/SettingsRow';
import { useKeyboardShortcutSettingsView } from './hooks/useKeyboardShortcutSettingsView';
import styles from './KeyboardShortcutSettings.module.css';

interface KeyboardShortcutSettingsProps {
	readonly controller: KeyboardShortcutSettingsController;
	readonly platform: KeyboardShortcutPlatform;
	readonly query: string;
	/** The configured command holding each default shortcut it took, by shortcut id. */
	readonly takenByConfiguredCommands?: ReadonlyMap<string, string> | undefined;
}

/**
 * Window and panel shortcuts as one list card. The card's filter finds an action by name or by a
 * recorded chord; each line records a new chord in place and restores a changed one.
 */
export function KeyboardShortcutSettings({
	controller,
	platform,
	query,
	takenByConfiguredCommands,
}: KeyboardShortcutSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const view = useKeyboardShortcutSettingsView({
		controller,
		platform,
		takenByConfiguredCommands,
	});
	const searching = query.trim() !== '';
	const total = controller.declarations.length;
	const filterVisible = !searching && total > SETTINGS_LIST_FILTER_THRESHOLD;

	return (
		<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupShortcuts)}>
			{filterVisible ? (
				<div className={styles['filter']} data-settings-row data-slot='keyboard-shortcut-discovery'>
					<div className={styles['filterControls']}>
						<InputGroup className={styles['filterInput']} data-recording={view.search.recording}>
							<InputGroupAddon>
								<ProductIcon name='search' />
							</InputGroupAddon>
							<InputGroupInput
								ref={view.search.inputReference}
								aria-describedby={
									view.search.status === null ? undefined : 'keyboard-shortcut-search-status'
								}
								aria-label={view.search.label}
								autoComplete='off'
								placeholder={view.search.placeholder}
								readOnly={view.search.recording}
								spellCheck={false}
								value={view.search.query}
								onChange={view.search.onChange}
								onKeyDown={view.search.onKeyDown}
							/>
							{view.search.query.length === 0 ? null : (
								<InputGroupAddon align='inline-end'>
									<InputGroupButton
										aria-label={view.search.clearLabel}
										size='icon-xs'
										onClick={view.search.onClear}
									>
										<ProductIcon name='close' />
									</InputGroupButton>
								</InputGroupAddon>
							)}
						</InputGroup>
						<Button
							aria-pressed={view.search.recording}
							data-slot='keyboard-shortcut-record-keys'
							size='sm'
							variant={view.search.recording ? 'secondary' : 'outline'}
							onClick={view.search.onRecordingToggle}
						>
							{view.search.recordingToggleLabel}
						</Button>
						<span aria-hidden='true' className={styles['count']}>
							{intl.formatMessage(terminalSettingsMessages.pageFilterCount, {
								shown: view.rows.length,
								total,
							})}
						</span>
					</div>
					{view.search.status === null ? null : (
						<p
							aria-live='polite'
							className={styles['filterStatus']}
							id='keyboard-shortcut-search-status'
							role='status'
						>
							{view.search.status}
						</p>
					)}
				</div>
			) : null}
			{filterVisible && view.rows.length === 0 ? (
				<SettingsListFilterEmpty message={view.noResults} />
			) : null}
			{view.rows.map((row) => {
				const noteId = `keyboard-shortcuts.${row.shortcutId}.configured-command`;
				return (
					<SettingsRow
						key={row.shortcutId}
						label={row.actionName}
						modified={row.sourceKind === 'custom'}
						query={query}
						searchTerms={[row.chord, row.scope, row.availability]}
						settingId={`keyboard-shortcuts.${row.shortcutId}`}
						status={
							<>
								{row.configuredCommandNote === null ? null : (
									<div className={styles['error']} id={noteId}>
										{row.configuredCommandNote}
									</div>
								)}
								{row.recording ? (
									<div className={styles['recording']} role='status'>
										{view.recordingInstruction}
									</div>
								) : null}
								{row.error === null ? null : (
									<div className={styles['error']} role='alert'>
										{row.error}
									</div>
								)}
							</>
						}
						onRestore={row.onReset}
					>
						<button
							aria-describedby={row.configuredCommandNote === null ? undefined : noteId}
							aria-label={view.changeLabel(row.actionName)}
							aria-pressed={row.recording}
							className={styles['recorder']}
							data-recording={row.recording}
							data-shortcut-id={row.shortcutId}
							type='button'
							onBlur={row.onBlur}
							onClick={row.onChangeClick}
							onKeyDown={row.onKeyDown}
						>
							<Kbd>{row.chord}</Kbd>
						</button>
					</SettingsRow>
				);
			})}
		</SettingsGroup>
	);
}
