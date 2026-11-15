import { useOrchbeeIntl, websiteTerminalMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { PageStage } from '../../../page-stage/PageStage';
import { SessionScene } from '../../../session-scene/SessionScene';
import { SessionTerminalInterface } from '../../../session-terminal-interface/SessionTerminalInterface';
import { StateLegend } from '../../../state-legend/StateLegend';

// The scripted Session in the terminal interface under the page head, with its caption and the
// legend of Agent states.
export function TerminalStage(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<SessionScene>
			<PageStage
				caption={t(messages.stageCaption, richText)}
				aside={<StateLegend states={['working', 'attention', 'done', 'idle']} />}
			>
				<SessionTerminalInterface />
			</PageStage>
		</SessionScene>
	);
}
