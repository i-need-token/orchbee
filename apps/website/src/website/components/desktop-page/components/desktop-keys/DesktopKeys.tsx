import {
	useOrchbeeIntl,
	websiteDesktopMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { KeyGroups } from '../../../key-groups/KeyGroups';

// The desktop application's default shortcuts on Windows, as bound in
// apps/web/src/workbench/utils/workbench-shortcuts.ts and, for Find and Quick Select,
// apps/web/src/projected-terminal-keyboard-shortcuts/projected-terminal-keyboard-shortcuts.ts.
const GROUPS = [
	{
		id: 'terminals',
		title: messages.keysTerminals,
		rows: [
			{ id: 'newTerminal', label: messages.keysTerminalsNewTerminal, keys: 'Ctrl+Shift+T' },
			{ id: 'newWorkspace', label: messages.keysTerminalsNewWorkspace, keys: 'Ctrl+Shift+N' },
			{ id: 'startAgent', label: messages.keysTerminalsStartAgent, keys: 'Ctrl+Shift+G' },
			{ id: 'goToTab', label: messages.keysTerminalsGoToTab, keys: 'Ctrl+1–9' },
		],
	},
	{
		id: 'panels',
		title: messages.keysPanels,
		rows: [
			{ id: 'splitRight', label: messages.keysPanelsSplitRight, keys: 'Ctrl+Shift+D' },
			{ id: 'splitDown', label: messages.keysPanelsSplitDown, keys: 'Ctrl+Shift+E' },
			{ id: 'maximize', label: messages.keysPanelsMaximize, keys: 'Ctrl+Shift+Enter' },
			{ id: 'equalize', label: messages.keysPanelsEqualize, keys: 'Ctrl+Alt+=' },
			{ id: 'focus', label: messages.keysPanelsFocus, keys: 'Ctrl+Alt+← ↓ ↑ →' },
			{ id: 'resize', label: messages.keysPanelsResize, keys: 'Alt+Shift+← ↓ ↑ →' },
		],
	},
	{
		id: 'search',
		title: messages.keysSearch,
		rows: [
			{ id: 'actions', label: messages.keysSearchActions, keys: 'Ctrl+Shift+P' },
			{ id: 'everything', label: messages.keysSearchEverything, keys: 'Ctrl+Shift+K' },
			{ id: 'find', label: messages.keysSearchFind, keys: 'Ctrl+Shift+F' },
			{ id: 'quickSelect', label: messages.keysSearchQuickSelect, keys: 'Ctrl+Shift+O' },
		],
	},
	{
		id: 'window',
		title: messages.keysWindow,
		rows: [
			{ id: 'settings', label: messages.keysWindowSettings, keys: 'Ctrl+,' },
			{ id: 'sidebar', label: messages.keysWindowSidebar, keys: 'Ctrl+Shift+B' },
		],
	},
] as const;

export function DesktopKeys(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailSection
			kicker={t(websiteShellMessages.detailReference)}
			title={t(messages.keysTitle)}
			lead={t(messages.keysLead, richText)}
		>
			<KeyGroups
				groups={GROUPS.map((group) => ({
					id: group.id,
					title: t(group.title),
					rows: group.rows.map((row) => ({ id: row.id, label: t(row.label), keys: row.keys })),
				}))}
			/>
		</DetailSection>
	);
}
