import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { Codicon } from '../codicon/Codicon';
import { StateGlyph } from '../state-glyph/StateGlyph';
import type { TuiSidebarWorkspace } from '../tui-sidebar/TuiSidebar';
import { AgentMark } from './components/agent-mark/AgentMark';
import { AGENT_MARK_DRAWINGS } from './components/agent-mark/agent-mark-drawings';
import styles from './DesktopSidebar.module.css';

/** A Workspace as the desktop sidebar lists it: the terminal interface's, with how far its branch
 * is ahead of and behind its upstream. */
export interface DesktopSidebarWorkspace extends TuiSidebarWorkspace {
	readonly ahead?: number;
	readonly behind?: number;
}

/**
 * The sidebar of the desktop application: the sidebar button, New workspace, one Project with its
 * Workspaces and their Agents, and the Settings gear alone in the footer, as a release build shows
 * it while no update is offered and the pointer is elsewhere: an open Project shows no chevron, and
 * each Workspace leads with its folder, open while its Agents show, one level above them. An Agent
 * row ends with its product's mark, or names the product where it has none. The New workspace label
 * is product chrome and follows the page language. It is a DesktopWindow's `sidebar`.
 */
export function DesktopSidebar({
	project,
	workspaces,
}: {
	project: string;
	workspaces: readonly DesktopSidebarWorkspace[];
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['sidebar']}>
			<div className={styles['top']}>
				<span className={styles['iconButton']}>
					<Codicon name='layout-sidebar-left' />
				</span>
			</div>
			<div className={styles['newWorkspace']}>
				<span className={styles['itemIcon']}>
					<Codicon name='add' />
				</span>
				<span className={styles['label']}>{t(workbenchMessages.sidebarNewWorkspace)}</span>
			</div>
			<div className={styles['tree']}>
				<div className={styles['project']}>
					<span className={styles['projectName']}>{project}</span>
				</div>
				<div className={styles['list']}>
					{workspaces.map((workspace) => (
						<WorkspaceRows key={workspace.name} workspace={workspace} />
					))}
				</div>
			</div>
			<div className={styles['footer']}>
				<span className={styles['iconButton']}>
					<Codicon name='settings-gear' />
				</span>
			</div>
		</div>
	);
}

function WorkspaceRows({ workspace }: { workspace: DesktopSidebarWorkspace }): ReactElement {
	const { ahead = 0, behind = 0 } = workspace;
	return (
		<div className={styles['workspace']}>
			<div className={workspace.selected ? styles['selectedRow'] : styles['row']}>
				<span className={styles['line1']}>
					<span className={styles['folder']}>
						<Codicon name={workspace.agents.length > 0 ? 'folder-opened' : 'folder'} />
					</span>
					<span className={styles['name']}>{workspace.name}</span>
				</span>
				<span className={styles['line2']}>
					<span className={styles['lineText']}>{workspace.branch}</span>
				</span>
				{ahead > 0 || behind > 0 ? (
					<span className={styles['endInfo']}>
						{ahead > 0 ? <span className={styles['ahead']}>↑{ahead}</span> : null}
						{behind > 0 ? <span className={styles['behind']}>↓{behind}</span> : null}
					</span>
				) : null}
			</div>
			{workspace.agents.length > 0 ? (
				<div className={styles['agents']}>
					{workspace.agents.map((agent) => (
						<AgentRow key={`${agent.provider} ${agent.title}`} agent={agent} />
					))}
				</div>
			) : null}
		</div>
	);
}

function AgentRow({ agent }: { agent: DesktopSidebarWorkspace['agents'][number] }): ReactElement {
	const mark = AGENT_MARK_DRAWINGS.get(agent.provider);
	return (
		<div
			className={styles['agent']}
			hidden={agent.shown === false}
			data-scene-show={agent.bind?.shown}
		>
			<span className={styles['glyph']}>
				<StateGlyph state={agent.state} bind={agent.bind?.state} product='regular' />
			</span>
			<span className={styles['text']}>
				<span className={styles['title']} data-scene-text={agent.bind?.title}>
					{agent.title}
				</span>
				{mark === undefined ? <span className={styles['provider']}>{agent.provider}</span> : null}
			</span>
			<span className={styles['end']}>
				<span
					className={styles['unread']}
					hidden={!agent.unread}
					data-scene-show={agent.bind?.unread}
				>
					<span className={styles['dot']} />
				</span>
				{mark === undefined ? null : <AgentMark drawing={mark} />}
			</span>
		</div>
	);
}
