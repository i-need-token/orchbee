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
import type { UserPreferencesControl } from '../../../user-preferences';
import { useSharedPreferenceDefault } from '../../hooks/useSharedPreferenceDefault';
import { useSharedPreferenceDraft } from '../../hooks/useSharedPreferenceDraft';
import {
	formatScrollbackMegabytes,
	parseScrollbackMegabytes,
	SCROLLBACK_MEGABYTE_RANGE,
} from '../../utils/terminal-settings-draft-parsing';
import { MacosTerminalInputSettings } from '../macos-terminal-input-settings/MacosTerminalInputSettings';
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
	readonly userPreferences: UserPreferencesControl;
}

/** Selection and keys, terminal output, and launch profiles; each part needs its host control. */
export function TerminalBehaviorSettings({
	defaultSettingsControl,
	interactionController,
	launchProfileClient,
	macosInputController,
	query,
	userPreferences,
}: TerminalBehaviorSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const interaction = useTerminalInteractionSettings(interactionController);
	const copyOnSelect = useSharedPreferenceDefault(userPreferences, 'copyOnSelect');
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
							onRestore={copyOnSelect.restore}
						>
							<Switch
								aria-label={copyOnSelectLabel}
								checked={interaction.preferences.copyOnSelect}
								onCheckedChange={(checked): void => {
									interactionController.update({ copyOnSelect: checked });
								}}
							/>
						</SettingsRow>
					)}
					{macosInputController === undefined ? null : (
						<MacosTerminalInputSettings controller={macosInputController} query={query} />
					)}
				</SettingsGroup>
			)}
			{defaultSettingsControl === undefined ? null : (
				<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupOutput)}>
					<TerminalHistoryRow
						control={defaultSettingsControl}
						query={query}
						userPreferences={userPreferences}
					/>
				</SettingsGroup>
			)}
			{launchProfileClient === undefined ? null : (
				<TerminalLaunchProfileSettings client={launchProfileClient} query={query} />
			)}
		</>
	);
}

interface TerminalHistoryRowProps {
	readonly control: TerminalDefaultSettingsControl;
	readonly query: string;
	readonly userPreferences: UserPreferencesControl;
}

/** How much output a new terminal keeps, edited in megabytes. */
function TerminalHistoryRow({
	control,
	query,
	userPreferences,
}: TerminalHistoryRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const bytes = snapshot.configuration.scrollbackBytes;
	const scrollback = useSharedPreferenceDraft(
		formatScrollbackMegabytes(bytes),
		'scrollbackBytes',
		userPreferences,
		(stored) => (typeof stored === 'number' ? formatScrollbackMegabytes(stored) : String(stored)),
	);
	const restore = useSharedPreferenceDefault(userPreferences, 'scrollbackBytes');
	const [invalid, setInvalid] = React.useState(false);
	React.useEffect(() => {
		setInvalid(false);
	}, [bytes]);
	const commit = (): void => {
		const scrollbackBytes = parseScrollbackMegabytes(scrollback.draft);
		setInvalid(scrollbackBytes === null);
		if (scrollbackBytes !== null) scrollback.submit({ scrollbackBytes });
	};
	const label = intl.formatMessage(terminalSettingsMessages.pageTerminalHistoryLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageTerminalHistoryDescription)}
			effect='new-terminals'
			label={label}
			modified={restore.modified}
			query={query}
			settingId='terminal.history'
			status={
				invalid ? (
					<div className={styles['error']} role='alert'>
						{intl.formatMessage(
							terminalSettingsMessages.pageTerminalHistoryInvalid,
							SCROLLBACK_MEGABYTE_RANGE,
						)}
					</div>
				) : null
			}
			onRestore={restore.restore}
		>
			<span className={styles['unitField']}>
				<Input
					aria-invalid={invalid}
					aria-label={label}
					className={styles['numberInput']}
					inputMode='decimal'
					value={scrollback.draft}
					onBlur={commit}
					onChange={(event) => scrollback.change(event.currentTarget.value)}
					onKeyDown={(event) => {
						if (event.key !== 'Enter') return;
						event.preventDefault();
						commit();
						event.currentTarget.blur();
					}}
				/>
				<span className={styles['unit']}>
					{intl.formatMessage(terminalSettingsMessages.pageUnitMegabytes)}
				</span>
			</span>
		</SettingsRow>
	);
}
