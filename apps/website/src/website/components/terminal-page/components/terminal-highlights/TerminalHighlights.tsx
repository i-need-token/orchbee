import {
	useOrchbeeIntl,
	websiteShellMessages,
	websiteTerminalMessages as messages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { Highlights } from '../../../highlights/Highlights';

// The terminal interface's features, each with the keys that reach it.
const HIGHLIGHTS = [
	{
		id: 'prefix',
		title: messages.highlightPrefixTitle,
		body: messages.highlightPrefixBody,
		keys: 'Ctrl+b Space',
	},
	{
		id: 'mouse',
		title: messages.highlightMouseTitle,
		body: messages.highlightMouseBody,
		keys: 'Ctrl+b m',
	},
	{
		id: 'copyMode',
		title: messages.highlightCopyModeTitle,
		body: messages.highlightCopyModeBody,
		keys: 'Ctrl+b Shift+U',
	},
	{
		id: 'find',
		title: messages.highlightFindTitle,
		body: messages.highlightFindBody,
		keys: 'Ctrl+b f',
	},
	{
		id: 'agents',
		title: messages.highlightAgentsTitle,
		body: messages.highlightAgentsBody,
		keys: 'Ctrl+b Shift+G',
	},
	{
		id: 'worktrees',
		title: messages.highlightWorktreesTitle,
		body: messages.highlightWorktreesBody,
		keys: 'Ctrl+b Shift+W',
	},
	{
		id: 'settings',
		title: messages.highlightSettingsTitle,
		body: messages.highlightSettingsBody,
		keys: 'Ctrl+b ,',
	},
	{
		id: 'leave',
		title: messages.highlightLeaveTitle,
		body: messages.highlightLeaveBody,
		keys: 'Ctrl+b q',
	},
	{
		id: 'size',
		title: messages.highlightSizeTitle,
		body: messages.highlightSizeBody,
		keys: 'Ctrl+b b',
	},
] as const;

export function TerminalHighlights(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailSection
			kicker={t(websiteShellMessages.detailHighlights)}
			title={t(messages.highlightsTitle)}
		>
			<Highlights
				items={HIGHLIGHTS.map(({ id, title, body, keys }) => ({
					id,
					title: t(title),
					body: t(body, richText),
					keys,
				}))}
			/>
		</DetailSection>
	);
}
