import {
	useOrchbeeIntl,
	websiteShellMessages,
	websiteTerminalMessages as messages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { KeyGroups } from '../../../key-groups/KeyGroups';

// The terminal interface's default keys after the prefix, Ctrl+b, as bound in
// apps/orchbee/crates/workbench-client/src/client_view/bindings.rs.
const GROUPS = [
	{
		id: 'panels',
		title: messages.keysPanels,
		rows: [
			{ id: 'splitRight', label: messages.keysPanelsSplitRight, keys: '|' },
			{ id: 'splitDown', label: messages.keysPanelsSplitDown, keys: '-' },
			{ id: 'splitLeft', label: messages.keysPanelsSplitLeft, keys: '\\' },
			{ id: 'splitUp', label: messages.keysPanelsSplitUp, keys: '_' },
			{ id: 'focus', label: messages.keysPanelsFocus, keys: '← ↓ ↑ → / h j k l' },
			{ id: 'resize', label: messages.keysPanelsResize, keys: 'Alt+← ↓ ↑ →' },
			{ id: 'maximize', label: messages.keysPanelsMaximize, keys: 'z' },
			{ id: 'move', label: messages.keysPanelsMove, keys: 'Shift+H J K L' },
			{ id: 'last', label: messages.keysPanelsLast, keys: ';' },
			{ id: 'close', label: messages.keysPanelsClose, keys: 'Shift+X' },
		],
	},
	{
		id: 'tabs',
		title: messages.keysTabs,
		rows: [
			{ id: 'newTab', label: messages.keysTabsNewTab, keys: 'c' },
			{ id: 'nextTab', label: messages.keysTabsNextTab, keys: 'n / p' },
			{ id: 'tabNumber', label: messages.keysTabsTabNumber, keys: '1–9' },
			{ id: 'moveTab', label: messages.keysTabsMoveTab, keys: '< / >' },
			{ id: 'renameTab', label: messages.keysTabsRenameTab, keys: 'F2' },
			{ id: 'closeTab', label: messages.keysTabsCloseTab, keys: 'x' },
			{ id: 'newWorkspace', label: messages.keysTabsNewWorkspace, keys: 'a' },
			{ id: 'renameWorkspace', label: messages.keysTabsRenameWorkspace, keys: 'r' },
			{ id: 'closeWorkspace', label: messages.keysTabsCloseWorkspace, keys: 'Shift+D' },
			{ id: 'navigator', label: messages.keysTabsNavigator, keys: 'w' },
		],
	},
	{
		id: 'agents',
		title: messages.keysAgents,
		rows: [
			{ id: 'startAgent', label: messages.keysAgentsStartAgent, keys: 'Shift+G' },
			{ id: 'nextAgent', label: messages.keysAgentsNextAgent, keys: '{ / }' },
			{ id: 'notification', label: messages.keysAgentsNotification, keys: 'o' },
			{ id: 'createWorktree', label: messages.keysAgentsCreateWorktree, keys: 'Shift+W' },
			{ id: 'openWorktree', label: messages.keysAgentsOpenWorktree, keys: 'Shift+O' },
		],
	},
	{
		id: 'interface',
		title: messages.keysInterface,
		rows: [
			{ id: 'actionMenu', label: messages.keysInterfaceActionMenu, keys: 'Space / ?' },
			{ id: 'settings', label: messages.keysInterfaceSettings, keys: ',' },
			{ id: 'sidebar', label: messages.keysInterfaceSidebar, keys: 'b' },
			{ id: 'focusSidebar', label: messages.keysInterfaceFocusSidebar, keys: 'Tab' },
			{ id: 'find', label: messages.keysInterfaceFind, keys: 'f' },
			{ id: 'copyMode', label: messages.keysInterfaceCopyMode, keys: 'Shift+U' },
			{ id: 'history', label: messages.keysInterfaceHistory, keys: 'PageUp' },
			{ id: 'mouse', label: messages.keysInterfaceMouse, keys: 'm' },
			{ id: 'sendPrefix', label: messages.keysInterfaceSendPrefix, keys: 'Ctrl+b' },
			{ id: 'leave', label: messages.keysInterfaceLeave, keys: 'q / d' },
		],
	},
] as const;

export function TerminalKeys(): ReactElement {
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
