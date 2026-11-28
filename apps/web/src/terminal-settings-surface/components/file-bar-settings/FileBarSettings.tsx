import { terminalSettingsMessages, useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import { isSettingOnPage } from '@orchbee/workbench-client-protocol';
import type * as React from 'react';

import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';
import { SettingsSelect } from '../settings-select/SettingsSelect';

interface FileBarSettingsProps {
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** The File Bar's on and off keys: what its views confirm, suggest, fetch and warn about. */
type FileBarSwitchKey =
	| 'explorer.confirm_delete'
	| 'explorer.confirm_drag_and_drop'
	| 'explorer.confirm_paste_native'
	| 'git.autofetch'
	| 'git.confirm_force_push'
	| 'git.confirm_sync'
	| 'git.enable_smart_commit'
	| 'git.ignore_limit_warning'
	| 'git.ignore_missing_git_warning'
	| 'git.suggest_smart_commit';

interface FileBarSwitch {
	readonly description: MessageDescriptor;
	readonly key: FileBarSwitchKey;
	readonly label: MessageDescriptor;
	/** The row's stable identifier in the settings page. */
	readonly settingId: string;
}

const EXPLORER_SWITCHES: readonly FileBarSwitch[] = [
	{
		description: terminalSettingsMessages.pageFileBarConfirmDeleteDescription,
		key: 'explorer.confirm_delete',
		label: terminalSettingsMessages.pageFileBarConfirmDeleteLabel,
		settingId: 'file-bar.confirm-delete',
	},
	{
		description: terminalSettingsMessages.pageFileBarConfirmDragAndDropDescription,
		key: 'explorer.confirm_drag_and_drop',
		label: terminalSettingsMessages.pageFileBarConfirmDragAndDropLabel,
		settingId: 'file-bar.confirm-drag-and-drop',
	},
	{
		description: terminalSettingsMessages.pageFileBarConfirmPasteNativeDescription,
		key: 'explorer.confirm_paste_native',
		label: terminalSettingsMessages.pageFileBarConfirmPasteNativeLabel,
		settingId: 'file-bar.confirm-paste-native',
	},
];

/** Source Control's switches after the parent repository choice, in the settings table's order. */
const SOURCE_CONTROL_SWITCHES: readonly FileBarSwitch[] = [
	{
		description: terminalSettingsMessages.pageFileBarSmartCommitDescription,
		key: 'git.enable_smart_commit',
		label: terminalSettingsMessages.pageFileBarSmartCommitLabel,
		settingId: 'file-bar.smart-commit',
	},
	{
		description: terminalSettingsMessages.pageFileBarSuggestSmartCommitDescription,
		key: 'git.suggest_smart_commit',
		label: terminalSettingsMessages.pageFileBarSuggestSmartCommitLabel,
		settingId: 'file-bar.suggest-smart-commit',
	},
	{
		description: terminalSettingsMessages.pageFileBarConfirmSyncDescription,
		key: 'git.confirm_sync',
		label: terminalSettingsMessages.pageFileBarConfirmSyncLabel,
		settingId: 'file-bar.confirm-sync',
	},
	{
		description: terminalSettingsMessages.pageFileBarConfirmForcePushDescription,
		key: 'git.confirm_force_push',
		label: terminalSettingsMessages.pageFileBarConfirmForcePushLabel,
		settingId: 'file-bar.confirm-force-push',
	},
	{
		description: terminalSettingsMessages.pageFileBarAutofetchDescription,
		key: 'git.autofetch',
		label: terminalSettingsMessages.pageFileBarAutofetchLabel,
		settingId: 'file-bar.autofetch',
	},
	{
		description: terminalSettingsMessages.pageFileBarIgnoreLimitWarningDescription,
		key: 'git.ignore_limit_warning',
		label: terminalSettingsMessages.pageFileBarIgnoreLimitWarningLabel,
		settingId: 'file-bar.ignore-limit-warning',
	},
	{
		description: terminalSettingsMessages.pageFileBarIgnoreMissingGitWarningDescription,
		key: 'git.ignore_missing_git_warning',
		label: terminalSettingsMessages.pageFileBarIgnoreMissingGitWarningLabel,
		settingId: 'file-bar.ignore-missing-git-warning',
	},
];

/**
 * The File Bar's side and what its views ask, suggest and warn about. A row shows once the settings
 * registry marks its key for the page, which a key's feature does when it ships.
 */
export function FileBarSettings({ query, userSettings }: FileBarSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const switchRows = (rows: readonly FileBarSwitch[]): React.ReactNode =>
		rows.map((row) => (
			<FileBarSwitchRow key={row.key} query={query} row={row} userSettings={userSettings} />
		));
	return (
		<>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageFileBarGroupLayout)}>
				<PositionRow query={query} userSettings={userSettings} />
			</SettingsGroup>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageFileBarGroupExplorer)}>
				{switchRows(EXPLORER_SWITCHES)}
			</SettingsGroup>
			<SettingsGroup
				label={intl.formatMessage(terminalSettingsMessages.pageFileBarGroupSourceControl)}
			>
				<ParentRepositoriesRow query={query} userSettings={userSettings} />
				{switchRows(SOURCE_CONTROL_SWITCHES)}
			</SettingsGroup>
		</>
	);
}

/** The side of the terminal area the File Bar docks on, which Move File Bar Left and Right write. */
function PositionRow({ query, userSettings }: FileBarSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const position = useSetting(userSettings, 'appearance.file_bar.position');
	if (!isSettingOnPage('appearance.file_bar.position')) return null;
	const label = intl.formatMessage(terminalSettingsMessages.pageFileBarPositionLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageFileBarPositionDescription)}
			label={label}
			modified={position.modified}
			query={query}
			settingId='file-bar.position'
			status={<SettingFailure message={position.failureMessage} />}
			onRestore={position.readOnly ? undefined : position.restore}
		>
			<SettingsSelect
				disabled={position.readOnly}
				label={label}
				options={[
					{
						label: intl.formatMessage(terminalSettingsMessages.pageFileBarPositionRight),
						value: 'right',
					},
					{
						label: intl.formatMessage(terminalSettingsMessages.pageFileBarPositionLeft),
						value: 'left',
					},
				]}
				value={position.value}
				onValueChange={(value): void => {
					position.set(value);
				}}
			/>
		</SettingsRow>
	);
}

/** Whether Source Control opens a Git repository above a Workspace's directory, as VS Code asks. */
function ParentRepositoriesRow({ query, userSettings }: FileBarSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const parentRepositories = useSetting(userSettings, 'git.open_repository_in_parent_folders');
	if (!isSettingOnPage('git.open_repository_in_parent_folders')) return null;
	const label = intl.formatMessage(terminalSettingsMessages.pageFileBarParentRepositoriesLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(
				terminalSettingsMessages.pageFileBarParentRepositoriesDescription,
			)}
			label={label}
			modified={parentRepositories.modified}
			query={query}
			settingId='file-bar.parent-repositories'
			status={<SettingFailure message={parentRepositories.failureMessage} />}
			onRestore={parentRepositories.readOnly ? undefined : parentRepositories.restore}
		>
			<SettingsSelect
				disabled={parentRepositories.readOnly}
				label={label}
				options={[
					{
						label: intl.formatMessage(terminalSettingsMessages.pageFileBarParentRepositoriesAlways),
						value: 'always',
					},
					{
						label: intl.formatMessage(terminalSettingsMessages.pageFileBarParentRepositoriesNever),
						value: 'never',
					},
					{
						label: intl.formatMessage(terminalSettingsMessages.pageFileBarParentRepositoriesPrompt),
						value: 'prompt',
					},
				]}
				value={parentRepositories.value}
				onValueChange={(value): void => {
					parentRepositories.set(value);
				}}
			/>
		</SettingsRow>
	);
}

interface FileBarSwitchRowProps {
	readonly query: string;
	readonly row: FileBarSwitch;
	readonly userSettings: UserSettingsControl;
}

function FileBarSwitchRow({ query, row, userSettings }: FileBarSwitchRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const setting = useSetting(userSettings, row.key);
	if (!isSettingOnPage(row.key)) return null;
	const label = intl.formatMessage(row.label);
	return (
		<SettingsRow
			description={intl.formatMessage(row.description)}
			label={label}
			modified={setting.modified}
			query={query}
			settingId={row.settingId}
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
