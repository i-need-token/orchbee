import { useOrchbeeIntl, websiteCliMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { PageStage } from '../../../page-stage/PageStage';
import { SessionCommandLine } from '../../../session-command-line/SessionCommandLine';
import { SessionScene } from '../../../session-scene/SessionScene';

// The scripted Session seen from a console outside Orchbee, under the page head.
export function CliStage(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<SessionScene>
			<PageStage caption={t(messages.stageCaption, richText)}>
				<SessionCommandLine />
			</PageStage>
		</SessionScene>
	);
}
