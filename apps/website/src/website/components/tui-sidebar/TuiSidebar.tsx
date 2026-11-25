import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { StateGlyph, type AgentState } from '../state-glyph/StateGlyph';
import styles from './TuiSidebar.module.css';

/** An Agent row. `bind` names the scene values script writes into it; `shown` hides the row;
 * `current` marks the Agent in the selected Tab's focused Panel. */
interface TuiSidebarAgent {
	readonly title: string;
	readonly provider: string;
	readonly state: AgentState;
	readonly unread: boolean;
	readonly current?: boolean;
	readonly shown?: boolean;
	readonly bind?: {
		readonly state?: string;
		readonly unread?: string;
		readonly title?: string;
		readonly shown?: string;
	};
}

export interface TuiSidebarWorkspace {
	readonly name: string;
	readonly branch: string;
	readonly selected?: boolean;
	readonly agents: readonly TuiSidebarAgent[];
}

/**
 * The sidebar of the terminal interface: New workspace, one Project with its Workspaces and their
 * Agents, and Settings. The two actions are product chrome and follow the page language. An open
 * Project shows no disclosure at rest; a Workspace's disclosure leads its name in its Agents' state
 * column, so the name aligns with their titles, and the guide runs down the list edge.
 */
export function TuiSidebar({
	project,
	workspaces,
}: {
	project: string;
	workspaces: readonly TuiSidebarWorkspace[];
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['sidebar']}>
			<div className={styles['action']}>
				<span>[ {t(workbenchMessages.sidebarNewWorkspace)} ]</span>
				<span className={styles['dim']}>[«]</span>
			</div>
			<div className={styles['tree']}>
				<div className={styles['project']}>{project}</div>
				{workspaces.map((workspace) => (
					<WorkspaceRows key={workspace.name} workspace={workspace} />
				))}
			</div>
			<div className={styles['action']}>
				<span>[ {t(workbenchMessages.settings)} ]</span>
				<span className={styles['dim']}>[?]</span>
			</div>
		</div>
	);
}

function WorkspaceRows({ workspace }: { workspace: TuiSidebarWorkspace }): ReactElement {
	return (
		<>
			<div className={workspace.selected ? styles['selectedWorkspace'] : styles['workspace']}>
				<div className={styles['identity']}>
					<span className={styles['disclosure']}>{workspace.agents.length > 0 ? '▾' : ''}</span>
					<span>{workspace.name}</span>
				</div>
				<div className={styles['branch']}>{workspace.branch}</div>
			</div>
			{workspace.agents.map((agent) => (
				<div
					key={`${agent.provider} ${agent.title}`}
					className={styles['agent']}
					hidden={agent.shown === false}
					data-current={agent.current === true ? '' : undefined}
					data-scene-show={agent.bind?.shown}
				>
					<span className={styles['guide']}>{agent.current === true ? '┃' : '│'}</span>
					<StateGlyph
						state={agent.state}
						selected={agent.current === true}
						bind={agent.bind?.state}
					/>
					<span className={styles['title']} data-scene-text={agent.bind?.title}>
						{agent.title}
					</span>
					<span className={styles['provider']}>· {agent.provider}</span>
					<span
						className={styles['unread']}
						hidden={!agent.unread}
						data-scene-show={agent.bind?.unread}
					>
						•
					</span>
				</div>
			))}
		</>
	);
}
