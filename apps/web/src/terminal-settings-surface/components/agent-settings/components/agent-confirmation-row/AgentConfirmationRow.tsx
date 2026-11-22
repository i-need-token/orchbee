import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Switch } from '@orchbee/ui/components';
import * as React from 'react';

import { AgentProviderMark } from '../../../../../agent-provider-mark';
import type { UserSettingsControl } from '../../../../../user-settings';
import { useSetting } from '../../../../hooks/useSetting';
import { SettingFailure } from '../../../setting-failure/SettingFailure';
import { SettingsRow } from '../../../settings-row/SettingsRow';

/** The saved settings that turn answering on, one for each Agent type Orchbee can answer for. */
type AgentConfirmationSetting =
	| 'agents.auto_allow.claude'
	| 'agents.auto_allow.codex'
	| 'agents.auto_allow.gemini'
	| 'agents.auto_allow.opencode';

interface AgentConfirmationRowProps {
	/** The Agent type's name, which is the row's label. */
	readonly label: string;
	/** The Agent's start provider, whose product mark leads the row. */
	readonly providerId: string;
	readonly query: string;
	/** The row's stable identifier in the settings page. */
	readonly settingId: string;
	readonly settingKey: AgentConfirmationSetting;
	readonly userSettings: UserSettingsControl;
}

/** The switch that lets Orchbee answer routine confirmations for every run of one Agent type. */
export function AgentConfirmationRow({
	label,
	providerId,
	query,
	settingId,
	settingKey,
	userSettings,
}: AgentConfirmationRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const allow = useSetting(userSettings, settingKey);
	const markPrefix = React.useId();
	return (
		<SettingsRow
			description={intl.formatMessage(
				terminalSettingsMessages.pageAgentConfirmationsRowDescription,
			)}
			icon={<AgentProviderMark gradientPrefix={markPrefix} providerId={providerId} />}
			label={label}
			modified={allow.modified}
			query={query}
			searchTerms={[
				intl.formatMessage(terminalSettingsMessages.pageGroupAgentConfirmations),
				intl.formatMessage(terminalSettingsMessages.pageAgentConfirmationsDescription),
			]}
			settingId={settingId}
			status={<SettingFailure message={allow.failureMessage} />}
			onRestore={allow.readOnly ? undefined : allow.restore}
		>
			<Switch
				aria-label={label}
				checked={allow.value}
				disabled={allow.readOnly}
				onCheckedChange={allow.set}
			/>
		</SettingsRow>
	);
}
