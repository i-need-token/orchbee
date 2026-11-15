import {
	useOrchbeeIntl,
	websiteCliMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { Highlights } from '../../../highlights/Highlights';

// What makes the command line fit for scripts and agents.
const HIGHLIGHTS = [
	{ id: 'handles', title: messages.highlightHandlesTitle, body: messages.highlightHandlesBody },
	{ id: 'formats', title: messages.highlightFormatsTitle, body: messages.highlightFormatsBody },
	{ id: 'waiting', title: messages.highlightWaitingTitle, body: messages.highlightWaitingBody },
	{ id: 'guide', title: messages.highlightGuideTitle, body: messages.highlightGuideBody },
	{
		id: 'completion',
		title: messages.highlightCompletionTitle,
		body: messages.highlightCompletionBody,
	},
	{
		id: 'exitCodes',
		title: messages.highlightExitCodesTitle,
		body: messages.highlightExitCodesBody,
	},
] as const;

export function CliHighlights(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailSection
			kicker={t(websiteShellMessages.detailHighlights)}
			title={t(messages.highlightsTitle)}
		>
			<Highlights
				items={HIGHLIGHTS.map(({ id, title, body }) => ({
					id,
					title: t(title),
					body: t(body, richText),
				}))}
			/>
		</DetailSection>
	);
}
