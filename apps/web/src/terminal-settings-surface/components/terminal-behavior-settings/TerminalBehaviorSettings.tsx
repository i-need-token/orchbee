import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Input, Switch } from '@orchbee/ui/components';
import * as React from 'react';

import type { MacosTerminalInputSettingsController } from '../../../macos-terminal-input-settings';
import type { TerminalDefaultSettingsControl } from '../../../terminal-default-settings-control';
import {
	useTerminalInteractionSettings,
	type TerminalInteractionSettingsController,
} from '../../../terminal-interaction-settings';
import type { TerminalLaunchProfileControl } from '../../../terminal-launch-profile-control';
import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { useSettingDraft } from '../../hooks/useSettingDraft';
import {
	formatScrollbackMegabytes,
	parseScrollbackMegabytes,
	SCROLLBACK_MEGABYTE_RANGE,
} from '../../utils/terminal-settings-draft-parsing';
import { MacosTerminalInputSettings } from '../macos-terminal-input-settings/MacosTerminalInputSettings';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';
import { TerminalLaunchProfileSettings } from '../terminal-launch-profile-settings/TerminalLaunchProfileSettings';
import styles from './TerminalBehaviorSettings.module.css';

interface TerminalBehaviorSettingsProps {
	readonly defaultSettingsControl?: TerminalDefaultSettingsControl | undefined;
	readonly interactionController?: TerminalInteractionSettingsController | undefined;
	readonly launchProfileClient?: TerminalLaunchProfileControl | undefined;
	readonly macosInputController?: MacosTerminalInputSettingsController | undefined;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/**
 * Selection and keys, terminal history, what a restart brings back, and launch profiles; each part
 * but the restart switch needs its host control.
 */
export function TerminalBehaviorSettings({
	defaultSettingsControl,
	interactionController,
	launchProfileClient,
	macosInputController,
	query,
	userSettings,
}: TerminalBehaviorSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const interaction = useTerminalInteractionSettings(interactionController);
	const copyOnSelect = useSetting(userSettings, 'terminal.copy_on_select');
	const copyOnSelectLabel = intl.formatMessage(terminalSettingsMessages.pageCopyOnSelectLabel);
	return (
		<>
			{interactionController === undefined && macosInputController === undefined ? null : (
				<SettingsGroup
					label={intl.formatMessage(terminalSettingsMessages.pageGroupSelectionAndKeys)}
				>
					{interactionController === undefined ? null : (
						<SettingsRow
							description={intl.formatMessage(terminalSettingsMessages.pageCopyOnSelectDescription)}
							label={copyOnSelectLabel}
							modified={copyOnSelect.modified}
							query={query}
							settingId='terminal.copy-on-select'
							status={<SettingFailure message={copyOnSelect.failureMessage} />}
							onRestore={copyOnSelect.readOnly ? undefined : copyOnSelect.restore}
						>
							<Switch
								aria-label={copyOnSelectLabel}
								checked={interaction.preferences.copyOnSelect}
								disabled={copyOnSelect.readOnly}
								onCheckedChange={(checked): void => {
									interactionController.update({ copyOnSelect: checked });
								}}
							/>
						</SettingsRow>
					)}
					{macosInputController === undefined ? null : (
						<MacosTerminalInputSettings
							controller={macosInputController}
							query={query}
							userSettings={userSettings}
						/>
					)}
				</SettingsGroup>
			)}
			{defaultSettingsControl === undefined ? null : (
				<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupOutput)}>
					<TerminalHistoryRow
						control={defaultSettingsControl}
						query={query}
						userSettings={userSettings}
					/>
				</SettingsGroup>
			)}
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupAfterRestart)}>
				<RestoreTerminalsRow query={query} userSettings={userSettings} />
			</SettingsGroup>
			{launchProfileClient === undefined ? null : (
				<TerminalLaunchProfileSettings client={launchProfileClient} query={query} />
			)}
		</>
	);
}

interface SettingRowProps {
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** Whether a restart resumes supported Agents and shows other terminals' recent output again. */
function RestoreTerminalsRow({ query, userSettings }: SettingRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const setting = useSetting(userSettings, 'terminal.restore_after_restart');
	const label = intl.formatMessage(terminalSettingsMessages.pageRestoreTerminalsLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageRestoreTerminalsDescription)}
			effect='next-restart'
			label={label}
			modified={setting.modified}
			query={query}
			settingId='terminal.restore-after-restart'
			status={<SettingFailure message={setting.failureMessage} />}
			onRestore={setting.readOnly ? undefined : setting.restore}
		>
			<Switch
				aria-label={label}
				checked={setting.value}
				disabled={setting.readOnly}
				onCheckedChange={setting.set}
			/>
		</SettingsRow>
	);
}

interface TerminalHistoryRowProps extends SettingRowProps {
	readonly control: TerminalDefaultSettingsControl;
}

/** How much output a new terminal keeps, edited in megabytes. */
function TerminalHistoryRow({
	control,
	query,
	userSettings,
}: TerminalHistoryRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const bytes = snapshot.configuration.scrollbackBytes;
	const setting = useSetting(userSettings, 'terminal.scrollback_bytes');
	const scrollback = useSettingDraft(formatScrollbackMegabytes(bytes), setting.failure);
	const [invalid, setInvalid] = React.useState(false);
	React.useEffect(() => {
		setInvalid(false);
	}, [bytes]);
	const commit = (): void => {
		const scrollbackBytes = parseScrollbackMegabytes(scrollback.draft);
		setInvalid(scrollbackBytes === null);
		if (scrollbackBytes !== null && scrollbackBytes !== bytes) setting.set(scrollbackBytes);
	};
	const label = intl.formatMessage(terminalSettingsMessages.pageTerminalHistoryLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageTerminalHistoryDescription)}
			effect='new-terminals'
			label={label}
			modified={setting.modified}
			query={query}
			settingId='terminal.history'
			status={
				<SettingFailure
					message={
						invalid
							? intl.formatMessage(
									terminalSettingsMessages.pageTerminalHistoryInvalid,
									SCROLLBACK_MEGABYTE_RANGE,
								)
							: setting.failureMessage
					}
				/>
			}
			onRestore={setting.readOnly ? undefined : setting.restore}
		>
			<span className={styles['unitField']}>
				<Input
					aria-invalid={invalid}
					aria-label={label}
					className={styles['numberInput']}
					inputMode='decimal'
					readOnly={setting.readOnly}
					value={scrollback.draft}
					onBlur={commit}
					onChange={(event) => scrollback.change(event.currentTarget.value)}
					onKeyDown={commitOnEnter(commit)}
				/>
				<span className={styles['unit']}>
					{intl.formatMessage(terminalSettingsMessages.pageUnitMegabytes)}
				</span>
			</span>
		</SettingsRow>
	);
}

function commitOnEnter(commit: () => void): React.KeyboardEventHandler<HTMLInputElement> {
	return (event) => {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		commit();
		event.currentTarget.blur();
	};
}
