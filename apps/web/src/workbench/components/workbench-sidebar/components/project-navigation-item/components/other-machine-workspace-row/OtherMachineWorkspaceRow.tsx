import type { ReactNode } from 'react';

import type { WorkbenchMachineMark } from '../../../../../../types/workbench-other-machines';
import { SidebarTruncationTooltip } from '../../../sidebar-truncation-tooltip/SidebarTruncationTooltip';
import { WorkspaceMachineMark } from '../workspace-machine-mark/WorkspaceMachineMark';
import styles from './OtherMachineWorkspaceRow.module.css';

const NAME_TOOLTIP_OFFSET = 4;

/**
 * A Workspace of a machine whose Session is not on screen: its name and, for another computer, the
 * machine's mark. Choosing it shows that Workspace; its menus, drags and Agents stay with the
 * Session on screen, as the TUI's rows keep them. A machine that is not connected keeps the rows it
 * listed when last read, muted, with its state word beneath, as the TUI's two-line rows do.
 */
export function OtherMachineWorkspaceRow({
	mark,
	name,
	onOpen,
	word,
}: {
	readonly mark: WorkbenchMachineMark | null;
	readonly name: string;
	readonly onOpen: () => void;
	readonly word: string | null;
}): ReactNode {
	const label = [name, mark === null ? null : `@${mark.name}`, word]
		.filter((part) => part !== null)
		.join(' ');
	return (
		<li className={styles['workspace']}>
			{/* A press anywhere on the row, its name included, chooses it, as a Workspace row's does. */}
			<div
				className={styles['row']}
				data-not-connected={word === null ? undefined : ''}
				onClick={onOpen}
			>
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
				{word === null ? null : <span className={styles['word']}>{word}</span>}
			</div>
		</li>
	);
}
