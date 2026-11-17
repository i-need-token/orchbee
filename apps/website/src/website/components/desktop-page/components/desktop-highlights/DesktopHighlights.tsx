import {
	useOrchbeeIntl,
	websiteDesktopMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { offersMacos, type Release } from '../../../../release';
import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { Highlights } from '../../../highlights/Highlights';

// The desktop application's features, each with the shortcut that reaches it on Windows and on
// macOS (apps/web/src/workbench/utils/workbench-shortcuts.ts and
// apps/web/src/projected-terminal-keyboard-shortcuts/projected-terminal-keyboard-shortcuts.ts).
const HIGHLIGHTS = [
	{ id: 'tabs', title: messages.highlightTabsTitle, body: messages.highlightTabsBody },
	{
		id: 'sidebar',
		title: messages.highlightSidebarTitle,
		body: messages.highlightSidebarBody,
		keys: 'Ctrl+Shift+B',
		macosKeys: '⌘B',
	},
	{
		id: 'search',
		title: messages.highlightSearchTitle,
		body: messages.highlightSearchBody,
		macosBody: messages.highlightSearchBodyMacos,
		keys: 'Ctrl+Shift+K',
		macosKeys: '⇧⌘F',
	},
	{
		id: 'find',
		title: messages.highlightFindTitle,
		body: messages.highlightFindBody,
		keys: 'Ctrl+Shift+F',
		macosKeys: '⌘F',
	},
	{
		id: 'quickSelect',
		title: messages.highlightQuickSelectTitle,
		body: messages.highlightQuickSelectBody,
		keys: 'Ctrl+Shift+O',
		macosKeys: '⇧⌘O',
	},
	{
		id: 'agents',
		title: messages.highlightAgentsTitle,
		body: messages.highlightAgentsBody,
		keys: 'Ctrl+Shift+G',
		macosKeys: '⇧⌘G',
	},
	{
		id: 'settings',
		title: messages.highlightSettingsTitle,
		body: messages.highlightSettingsBody,
		keys: 'Ctrl+,',
		macosKeys: '⌘,',
	},
	{
		id: 'appearance',
		title: messages.highlightAppearanceTitle,
		body: messages.highlightAppearanceBody,
	},
	{ id: 'input', title: messages.highlightInputTitle, body: messages.highlightInputBody },
] as const;

/** The feature cards; while the release offers macOS, each key is followed by its macOS key. */
export function DesktopHighlights({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const withMacos = offersMacos(release);
	return (
		<DetailSection
			kicker={t(websiteShellMessages.detailHighlights)}
			title={t(messages.highlightsTitle)}
		>
			<Highlights
				items={HIGHLIGHTS.map((item) => ({
					id: item.id,
					title: t(item.title),
					body: t(withMacos && 'macosBody' in item ? item.macosBody : item.body, richText),
					...('keys' in item
						? { keys: withMacos ? `${item.keys} / ${item.macosKeys}` : item.keys }
						: {}),
				}))}
			/>
		</DetailSection>
	);
}
