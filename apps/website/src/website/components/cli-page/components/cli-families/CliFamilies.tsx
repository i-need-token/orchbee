import {
	useOrchbeeIntl,
	websiteCliMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { CommandGroups } from '../../../command-groups/CommandGroups';
import { DetailSection } from '../../../detail-section/DetailSection';
import { CLI_FAMILIES } from '../../cli-families';

// The command families in their groups, with the command line's own help in the page language.
export function CliFamilies(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailSection
			kicker={t(websiteShellMessages.detailReference)}
			title={t(messages.familiesTitle)}
			lead={t(messages.familiesLead, richText)}
		>
			<CommandGroups
				groups={CLI_FAMILIES.map((group) => ({
					id: group.id,
					title: t(group.title),
					rows: group.rows.map((row) => ({ command: row.command, label: t(row.help) })),
				}))}
			/>
		</DetailSection>
	);
}
