import {
	useOrchbeeIntl,
	websiteDesktopMessages as messages,
	websiteShellMessages as shellMessages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { Release } from '../../release';
import { richText } from '../../rich-text';
import { GetOrchbee } from '../get-orchbee/GetOrchbee';
import { OtherClients } from '../other-clients/OtherClients';
import { PageHead } from '../page-head/PageHead';
import { DesktopBackground } from './components/desktop-background/DesktopBackground';
import { DesktopHighlights } from './components/desktop-highlights/DesktopHighlights';
import { DesktopKeys } from './components/desktop-keys/DesktopKeys';
import { DesktopPanels } from './components/desktop-panels/DesktopPanels';
import { DesktopRendering } from './components/desktop-rendering/DesktopRendering';
import { DesktopStage } from './components/desktop-stage/DesktopStage';

// The desktop application page, section by section.
export function DesktopPage({
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
				address='orchbee.com/desktop'
				title={t(shellMessages.clientDesktop)}
				lead={t(messages.lead)}
				action={<GetOrchbee release={release} offer='installer' />}
				note={release.published ? t(shellMessages.installerNote, richText) : undefined}
			>
				<DesktopStage />
			</PageHead>
			<DesktopPanels />
			<DesktopHighlights />
			<DesktopBackground />
			<DesktopRendering />
			<DesktopKeys />
			<OtherClients locale={locale} current='desktop' release={release} />
		</>
	);
}
