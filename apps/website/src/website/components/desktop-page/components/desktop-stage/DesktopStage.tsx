import { useOrchbeeIntl, websiteDesktopMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { PageStage } from '../../../page-stage/PageStage';
import { SessionDesktopApplication } from '../../../session-desktop-application/SessionDesktopApplication';
import { SessionScene } from '../../../session-scene/SessionScene';
import { StateLegend } from '../../../state-legend/StateLegend';

/**
 * The scripted Session in the desktop application under the page head, with the caption and the
 * legend of Agent states under it.
 */
export function DesktopStage(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<SessionScene>
			<PageStage
				caption={t(messages.stageCaption, richText)}
				aside={<StateLegend states={['working', 'attention', 'done', 'idle']} />}
			>
				<SessionDesktopApplication />
			</PageStage>
		</SessionScene>
	);
}
