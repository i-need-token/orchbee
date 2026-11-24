import type { ReactNode } from 'react';

import type { WorkbenchMachineMark } from '../../../../../../types/workbench-other-machines';
import { SidebarTruncationTooltip } from '../../../sidebar-truncation-tooltip/SidebarTruncationTooltip';
import { WorkspaceMachineMark } from '../workspace-machine-mark/WorkspaceMachineMark';
import styles from './OtherMachineWorkspaceRow.module.css';

const NAME_TOOLTIP_OFFSET = 4;

/**
 * A Workspace of a machine whose Session is not on screen: its name and, for another computer, the
 * machine's mark. Choosing it shows that Workspace; its menus, drags and Agents stay with the
 * Session on screen, as the TUI's rows keep them.
 */
export function OtherMachineWorkspaceRow({
	mark,
	name,
	onOpen,
}: {
	readonly mark: WorkbenchMachineMark | null;
	readonly name: string;
	readonly onOpen: () => void;
}): ReactNode {
	const label = mark === null ? name : `${name} @${mark.name}`;
	return (
		<li className={styles['workspace']}>
			{/* A press anywhere on the row, its name included, chooses it, as a Workspace row's does. */}
			<div className={styles['row']} onClick={onOpen}>
				<button
					aria-label={label}
					className={styles['rowMain']}
					data-sidebar-nav='machine-workspace'
					data-slot='workbench-sidebar-machine-workspace'
					tabIndex={-1}
					type='button'
				/>
				<span className={styles['line']}>
					<SidebarTruncationTooltip
						content={label}
						render={<span className={styles['name']} />}
						sideOffset={NAME_TOOLTIP_OFFSET}
					>
						{name}
					</SidebarTruncationTooltip>
					{mark === null ? null : <WorkspaceMachineMark mark={mark} />}
				</span>
			</div>
		</li>
	);
}
