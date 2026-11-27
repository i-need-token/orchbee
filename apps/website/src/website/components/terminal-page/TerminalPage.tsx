import {
	useOrchbeeIntl,
	websiteShellMessages as shellMessages,
	websiteTerminalMessages as messages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { Release } from '../../release';
import { GetOrchbee } from '../get-orchbee/GetOrchbee';
import { OtherClients } from '../other-clients/OtherClients';
import { PageHead } from '../page-head/PageHead';
import { TerminalHighlights } from './components/terminal-highlights/TerminalHighlights';
import { TerminalKeys } from './components/terminal-keys/TerminalKeys';
import { TerminalPanels } from './components/terminal-panels/TerminalPanels';
import { TerminalStage } from './components/terminal-stage/TerminalStage';
import { TerminalThemes } from './components/terminal-themes/TerminalThemes';

// The terminal interface page, section by section.
export function TerminalPage({
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
				address='orchbee.com/terminal'
				title={t(shellMessages.clientTerminal)}
				lead={t(messages.lead)}
				action={<GetOrchbee locale={locale} release={release} offer='npm' />}
			>
				<TerminalStage />
			</PageHead>
			<TerminalPanels />
			<TerminalHighlights />
			<TerminalThemes />
			<TerminalKeys />
			<OtherClients locale={locale} current='terminal' release={release} />
		</>
	);
}
