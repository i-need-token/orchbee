import {
	useOrchbeeIntl,
	websiteDesktopMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { Highlights } from '../../../highlights/Highlights';

// The desktop application's features, each with the shortcut that reaches it on Windows
// (apps/web/src/workbench/utils/workbench-shortcuts.ts and
// apps/web/src/projected-terminal-keyboard-shortcuts/projected-terminal-keyboard-shortcuts.ts).
const HIGHLIGHTS = [
	{ id: 'tabs', title: messages.highlightTabsTitle, body: messages.highlightTabsBody },
	{
		id: 'sidebar',
		title: messages.highlightSidebarTitle,
		body: messages.highlightSidebarBody,
		keys: 'Ctrl+Shift+B',
	},
	{
		id: 'search',
		title: messages.highlightSearchTitle,
		body: messages.highlightSearchBody,
		keys: 'Ctrl+Shift+K',
	},
	{
		id: 'find',
		title: messages.highlightFindTitle,
		body: messages.highlightFindBody,
		keys: 'Ctrl+Shift+F',
	},
	{
		id: 'quickSelect',
		title: messages.highlightQuickSelectTitle,
		body: messages.highlightQuickSelectBody,
		keys: 'Ctrl+Shift+O',
	},
	{
		id: 'agents',
		title: messages.highlightAgentsTitle,
		body: messages.highlightAgentsBody,
		keys: 'Ctrl+Shift+G',
	},
	{
		id: 'settings',
		title: messages.highlightSettingsTitle,
		body: messages.highlightSettingsBody,
		keys: 'Ctrl+,',
	},
	{
		id: 'appearance',
		title: messages.highlightAppearanceTitle,
		body: messages.highlightAppearanceBody,
	},
	{ id: 'input', title: messages.highlightInputTitle, body: messages.highlightInputBody },
] as const;

export function DesktopHighlights(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailSection
			kicker={t(websiteShellMessages.detailHighlights)}
			title={t(messages.highlightsTitle)}
		>
			<Highlights
				items={HIGHLIGHTS.map((item) => ({
					id: item.id,
					title: t(item.title),
					body: t(item.body, richText),
					...('keys' in item ? { keys: item.keys } : {}),
				}))}
			/>
		</DetailSection>
	);
}
