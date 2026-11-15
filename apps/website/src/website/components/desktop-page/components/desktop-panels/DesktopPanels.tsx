import { useOrchbeeIntl, websiteDesktopMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { PanelPlay } from '../../../panel-play/PanelPlay';
import { PanelPlayDesktopApplication } from '../../../panel-play-desktop-application/PanelPlayDesktopApplication';

// The section where the visitor splits, focuses, resizes, maximizes and closes Panels with the
// desktop application's shortcuts, the window first.
export function DesktopPanels(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<PanelPlay
			client='desktop'
			kicker={t(messages.panelsKicker)}
			title={t(messages.panelsTitle)}
			lead={t(messages.panelsLead)}
			window={<PanelPlayDesktopApplication />}
			flip
		/>
	);
}
