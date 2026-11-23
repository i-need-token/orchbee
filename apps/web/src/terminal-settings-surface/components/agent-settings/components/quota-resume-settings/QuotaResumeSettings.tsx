import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Input, Switch } from '@orchbee/ui/components';
import { isQuotaResumePrompt } from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import type { UserSettingsControl } from '../../../../../user-settings';
import { useSetting } from '../../../../hooks/useSetting';
import { useSettingDraft } from '../../../../hooks/useSettingDraft';
import { SettingFailure } from '../../../setting-failure/SettingFailure';
import { SettingsGroup } from '../../../settings-group/SettingsGroup';
import { SettingsRow } from '../../../settings-row/SettingsRow';

interface QuotaResumeSettingsProps {
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

export function QuotaResumeSettings({
	query,
	userSettings,
}: QuotaResumeSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const enabled = useSetting(userSettings, 'agents.quota_resume.enabled');
	const prompt = useSetting(userSettings, 'agents.quota_resume.prompt');
	const editor = useSettingDraft(prompt.value, prompt.failure);
	const [checkedDraft, setCheckedDraft] = React.useState(false);
	const invalid = checkedDraft && !isQuotaResumePrompt(editor.draft);
	if (!enabled.supported || !prompt.supported) return null;

	const heading = intl.formatMessage(terminalSettingsMessages.pageQuotaResumeGroup);
	const description = intl.formatMessage(terminalSettingsMessages.pageQuotaResumeDescription);
	const enabledLabel = intl.formatMessage(terminalSettingsMessages.pageQuotaResumeEnabled);
	const promptLabel = intl.formatMessage(terminalSettingsMessages.pageQuotaResumePrompt);
	const commit = (): void => {
		if (prompt.readOnly) return;
		setCheckedDraft(true);
		if (editor.draft !== prompt.value && isQuotaResumePrompt(editor.draft)) {
			prompt.set(editor.draft);
		}
	};
	return (
		<SettingsGroup description={description} label={heading}>
			<SettingsRow
				label={enabledLabel}
				modified={enabled.modified}
				query={query}
				searchTerms={[heading, description]}
				settingId='agents.quota-resume.enabled'
				status={<SettingFailure message={enabled.failureMessage} />}
				onRestore={enabled.readOnly ? undefined : enabled.restore}
			>
				<Switch
					aria-label={enabledLabel}
					checked={enabled.value}
					disabled={enabled.readOnly}
					onCheckedChange={enabled.set}
				/>
			</SettingsRow>
			<SettingsRow
				description={intl.formatMessage(terminalSettingsMessages.pageQuotaResumePromptDescription)}
				label={promptLabel}
				modified={prompt.modified}
				query={query}
				searchTerms={[heading, description]}
				settingId='agents.quota-resume.prompt'
				status={
					<SettingFailure
						message={
							invalid
								? intl.formatMessage(terminalSettingsMessages.pageQuotaResumePromptInvalid)
								: prompt.failureMessage
						}
					/>
				}
				onRestore={
					prompt.readOnly
						? undefined
						: () => {
								editor.change(prompt.value);
								setCheckedDraft(false);
								prompt.restore();
							}
				}
			>
				<Input
					aria-invalid={invalid}
					aria-label={promptLabel}
					readOnly={prompt.readOnly}
					value={editor.draft}
					onBlur={(event) => {
						// Moving to this row's Restore discards the draft instead of saving it first.
						const next = event.relatedTarget;
						const row = event.currentTarget.closest('[data-settings-row]');
						if (next instanceof HTMLButtonElement && row?.contains(next) === true) return;
						commit();
					}}
					onChange={(event) => {
						editor.change(event.currentTarget.value);
						setCheckedDraft(false);
					}}
					onKeyDown={(event) => {
						if (event.key !== 'Enter' || event.nativeEvent.isComposing) return;
						event.preventDefault();
						commit();
					}}
				/>
			</SettingsRow>
		</SettingsGroup>
	);
}
