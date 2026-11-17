import {
	useOrchbeeIntl,
	websiteCliMessages as messages,
	websiteShellMessages as shellMessages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { Release } from '../../release';
import { richText } from '../../rich-text';
import { GetOrchbee } from '../get-orchbee/GetOrchbee';
import { OtherClients } from '../other-clients/OtherClients';
import { PageHead } from '../page-head/PageHead';
import { CliFamilies } from './components/cli-families/CliFamilies';
import { CliGrammar } from './components/cli-grammar/CliGrammar';
import { CliHighlights } from './components/cli-highlights/CliHighlights';
import { CliOutputs } from './components/cli-outputs/CliOutputs';
import { CliStage } from './components/cli-stage/CliStage';

// The command line page, section by section.
export function CliPage({
	locale,
	release,
}: {
	locale: OrchbeeLocale;
	release: Release;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<>
			<PageHead
				address='orchbee.com/cli'
				title={t(shellMessages.clientCli)}
				lead={t(messages.lead, richText)}
				action={<GetOrchbee locale={locale} release={release} offer='npm' />}
			>
				<CliStage />
			</PageHead>
			<CliGrammar />
			<CliOutputs />
			<CliHighlights />
			<CliFamilies />
			<OtherClients locale={locale} current='cli' release={release} />
		</>
	);
}
