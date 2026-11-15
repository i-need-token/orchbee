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
				action={<GetOrchbee release={release} offer='npm' />}
			/>
			<OtherClients locale={locale} current='cli' release={release} />
		</>
	);
}
