import {
	useOrchbeeIntl,
	websiteHomeMessages as messages,
	websiteProductMessages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import {
	DesktopSidebar,
	type DesktopSidebarWorkspace,
} from '../../../../../desktop-sidebar/DesktopSidebar';
import { DesktopWindow } from '../../../../../desktop-window/DesktopWindow';
import { NumberedMark } from '../../../../../numbered-mark/NumberedMark';
import { CLAUDE_TITLE } from '../../../../../session-scene/session';
import { TuiSidebar } from '../../../../../tui-sidebar/TuiSidebar';
import { TuiWindow } from '../../../../../tui-window/TuiWindow';
import styles from './SidebarAnatomy.module.css';

// One sidebar, held still, that shows every state but failed, both unread and read Agents, and the
// current Agent among the rest.
const WORKSPACES: readonly DesktopSidebarWorkspace[] = [
	{
		name: 'shop-api',
		branch: 'main',
		selected: true,
		ahead: 2,
		agents: [
			{ title: CLAUDE_TITLE, provider: 'Claude Code', state: 'attention', unread: true },
			{
				title: 'Review the current changes',
				provider: 'Codex',
				state: 'working',
				unread: false,
				current: true,
			},
		],
	},
	{
		name: 'shop-web',
		branch: 'feature/cart',
		behind: 1,
		agents: [{ title: 'Update cart layout', provider: 'OpenCode', state: 'done', unread: true }],
	},
	{
		name: 'docs',
		branch: 'main',
		agents: [
			{ title: 'Draft install guide', provider: 'Gemini CLI', state: 'idle', unread: false },
		],
	},
];

// The numbered parts of the desktop sidebar, each with its note. The disc of each stands at the
// row it names; the Unread disc stands at the right end of the first Agent row.
const NOTES = [
	{ place: 'markProject', title: messages.agentsProjectTitle, body: messages.agentsProject },
	{ place: 'markWorkspace', title: messages.agentsWorkspaceTitle, body: messages.agentsWorkspace },
	{ place: 'markAgent', title: messages.agentsAgentTitle, body: messages.agentsAgent },
	{ place: 'markUnread', title: messages.agentsUnreadTitle, body: messages.agentsUnread },
] as const;

/**
 * The sidebar alone in the desktop application and in the terminal interface, showing the same
 * Workspaces and Agents, with four numbered parts of the desktop one explained beside them. A
 * part and its note light up together on hover, and on keyboard focus of the note.
 */
export function SidebarAnatomy(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['anatomy']} data-reveal='' data-marks=''>
			<div className={styles['pair']}>
				<figure className={styles['figure']}>
					<div className={styles['window']}>
						<DesktopWindow
							label={t(websiteProductMessages.desktopApplication)}
							size='side'
							sidebar={<DesktopSidebar project='shop' workspaces={WORKSPACES} />}
						>
							{null}
						</DesktopWindow>
					</div>
					{NOTES.map((note, index) => (
						<span key={note.place} className={styles[note.place]} data-mark={index + 1}>
							<NumberedMark number={index + 1} />
						</span>
					))}
					<figcaption className={styles['caption']}>
						{t(websiteShellMessages.clientDesktop)}
					</figcaption>
				</figure>
				<figure className={styles['figure']}>
					<div className={styles['window']}>
						<TuiWindow
							label={t(websiteProductMessages.terminalInterface)}
							size='side'
							sidebar={<TuiSidebar project='shop' workspaces={WORKSPACES} />}
						>
							{null}
						</TuiWindow>
					</div>
					<figcaption className={styles['caption']}>
						{t(websiteShellMessages.clientTerminal)}
					</figcaption>
				</figure>
			</div>
			<div>
				<p className={styles['lead']}>{t(messages.agentsSidebar)}</p>
				<ol className={styles['notes']}>
					{NOTES.map((note, index) => (
						<li key={note.place} className={styles['note']} data-mark={index + 1} data-mark-note=''>
							<NumberedMark number={index + 1} />
							<span>
								<b className={styles['noteTitle']}>{t(note.title)}</b>
								{t(note.body)}
							</span>
						</li>
					))}
				</ol>
			</div>
		</div>
	);
}
