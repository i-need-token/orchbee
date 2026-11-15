import { useOrchbeeIntl, websiteTerminalMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { PanelPlay } from '../../../panel-play/PanelPlay';
import { PanelPlayTerminalInterface } from '../../../panel-play-terminal-interface/PanelPlayTerminalInterface';

// The section where the visitor splits, focuses, resizes and closes Panels with the terminal
// interface's keys.
export function TerminalPanels(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<PanelPlay
			client='terminal'
			kicker={t(messages.panelsKicker)}
			title={t(messages.panelsTitle)}
			lead={t(messages.panelsLead, richText)}
			window={<PanelPlayTerminalInterface />}
		/>
	);
}
