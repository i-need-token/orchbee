import {
	useOrchbeeIntl,
	websiteDesktopMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { offeredPlatforms, type OfferedPlatforms, type Release } from '../../../../release';
import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { KeyGroups } from '../../../key-groups/KeyGroups';

// The desktop application's default shortcuts, as bound in
// apps/web/src/workbench/utils/workbench-shortcuts.ts and, for Find and Quick Select,
// apps/web/src/projected-terminal-keyboard-shortcuts/projected-terminal-keyboard-shortcuts.ts:
// `keys` on Windows, `macosKeys` on macOS, written as the product's formatter shows each platform
// (Apple modifier glyphs in the order ⌃⌥⇧⌘, docs/frontend/keyboard-shortcuts.md).
const GROUPS = [
	{
		id: 'terminals',
		title: messages.keysTerminals,
		rows: [
			{
				id: 'newTerminal',
				label: messages.keysTerminalsNewTerminal,
				keys: 'Ctrl+Shift+T',
				macosKeys: '⌘T',
			},
			{
				id: 'newWorkspace',
				label: messages.keysTerminalsNewWorkspace,
				keys: 'Ctrl+Shift+N',
				macosKeys: '⇧⌘N',
			},
			{
				id: 'startAgent',
				label: messages.keysTerminalsStartAgent,
				keys: 'Ctrl+Shift+G',
				macosKeys: '⇧⌘G',
			},
			{ id: 'goToTab', label: messages.keysTerminalsGoToTab, keys: 'Ctrl+1–9', macosKeys: '⌘1–9' },
		],
	},
	{
		id: 'panels',
		title: messages.keysPanels,
		rows: [
			{
				id: 'splitRight',
				label: messages.keysPanelsSplitRight,
				keys: 'Ctrl+Shift+D',
				macosKeys: '⌘D',
			},
			{
				id: 'splitDown',
				label: messages.keysPanelsSplitDown,
				keys: 'Ctrl+Shift+E',
				macosKeys: '⇧⌘D',
			},
			{
				id: 'maximize',
				label: messages.keysPanelsMaximize,
				keys: 'Ctrl+Shift+Enter',
				macosKeys: '⇧⌘Enter',
			},
			{ id: 'equalize', label: messages.keysPanelsEqualize, keys: 'Ctrl+Alt+=', macosKeys: '⌃⌘=' },
			{
				id: 'focus',
				label: messages.keysPanelsFocus,
				keys: 'Ctrl+Alt+← ↓ ↑ →',
				macosKeys: '⌥⌘← ↓ ↑ →',
			},
			{
				id: 'resize',
				label: messages.keysPanelsResize,
				keys: 'Alt+Shift+← ↓ ↑ →',
				macosKeys: '⌃⌘← ↓ ↑ →',
			},
		],
	},
	{
		id: 'search',
		title: messages.keysSearch,
		rows: [
			{
				id: 'actions',
				label: messages.keysSearchActions,
				keys: 'Ctrl+Shift+P',
				macosKeys: '⇧⌘P',
			},
			{
				id: 'everything',
				label: messages.keysSearchEverything,
				keys: 'Ctrl+Shift+K',
				macosKeys: '⇧⌘F',
			},
			{ id: 'find', label: messages.keysSearchFind, keys: 'Ctrl+Shift+F', macosKeys: '⌘F' },
			{
				id: 'quickSelect',
				label: messages.keysSearchQuickSelect,
				keys: 'Ctrl+Shift+O',
				macosKeys: '⇧⌘O',
			},
		],
	},
	{
		id: 'window',
		title: messages.keysWindow,
		rows: [
			{ id: 'settings', label: messages.keysWindowSettings, keys: 'Ctrl+,', macosKeys: '⌘,' },
			{
				id: 'sidebar',
				label: messages.keysWindowSidebar,
				keys: 'Ctrl+Shift+B',
				macosKeys: '⌘B',
			},
		],
	},
] as const;

// The line under the heading and the tables' column heads, by the platforms besides Windows the
// release offers: with macOS, a column of its keys beside the Windows ones, which Linux uses too.
// Platform names are not translated.
const LEAD = {
	windows: messages.keysLead,
	macos: messages.keysLeadMacos,
	linux: messages.keysLeadLinux,
	all: messages.keysLeadMacos,
} as const satisfies Record<OfferedPlatforms, unknown>;
const PLATFORMS = {
	windows: undefined,
	macos: ['Windows', 'macOS'],
	linux: undefined,
	all: ['Windows · Linux', 'macOS'],
} as const satisfies Record<OfferedPlatforms, readonly [string, string] | undefined>;

/** The shortcut reference; while the release offers macOS, with the macOS keys beside Windows'. */
export function DesktopKeys({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const offered = offeredPlatforms(release);
	const platforms = PLATFORMS[offered];
	return (
		<DetailSection
			kicker={t(websiteShellMessages.detailReference)}
			title={t(messages.keysTitle)}
			lead={t(LEAD[offered], richText)}
		>
			<KeyGroups
				groups={GROUPS.map((group) => ({
					id: group.id,
					title: t(group.title),
					rows: group.rows.map((row) => ({
						id: row.id,
						label: t(row.label),
						keys: row.keys,
						...(platforms === undefined ? {} : { macosKeys: row.macosKeys }),
					})),
				}))}
				platforms={platforms}
			/>
		</DetailSection>
	);
}
