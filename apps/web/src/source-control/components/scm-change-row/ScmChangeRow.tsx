import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { SCM_STATUS_FACTS, type ScmResourceStatus } from '../../model/scm-resource';
import styles from './ScmChangeRow.module.css';

interface ScmChangeRowProps {
	/** The row's text: a group's name, a folder's compact path, or a file's name. */
	readonly label: string;
	/** A file's folder in list view; empty for none. */
	readonly description: string;
	/** The row's hover text, a folder's or file's path; empty for none. */
	readonly title: string;
	/** A group's number of changes; null on other rows. */
	readonly count: string | null;
	/** A file's status, with its hover text; null on other rows. */
	readonly status: ScmResourceStatus | null;
	readonly statusText: string;
	/** Whether the row holds the tree's focus, which shows its buttons as a hover does. */
	readonly focused: boolean;
	/** The row's inline buttons. */
	readonly actions: ReactNode;
}

/**
 * One Changes row below a repository: a group with its count, a folder in tree view, or a file with
 * its folder in list view and its status letter, colored as VS Code colors only the letter. Its
 * buttons take the room before the count or letter while the row is hovered or focused.
 */
export function ScmChangeRow({
	actions,
	count,
	description,
	focused,
	label,
	status,
	statusText,
	title,
}: ScmChangeRowProps): ReactNode {
	const facts = status === null ? null : SCM_STATUS_FACTS[status];
	return (
		<span
			className={styles['root']}
			data-focused={focused ? '' : undefined}
			title={title === '' ? undefined : title}
		>
			<span
				className={styles['name']}
				data-struck-through={facts?.struckThrough === true ? '' : undefined}
			>
				{label}
			</span>
			{description === '' ? null : <span className={styles['description']}>{description}</span>}
			<span className={styles['actions']}>{actions}</span>
			{count === null ? null : <span className={styles['count']}>{count}</span>}
			{facts === null ? null : (
				<Tooltip>
					<TooltipTrigger
						render={
							<span
								aria-label={statusText}
								className={styles['letter']}
								data-git-color={facts.color}
								role='img'
							/>
						}
					>
						{facts.letter}
					</TooltipTrigger>
					<TooltipContent side='bottom'>{statusText}</TooltipContent>
				</Tooltip>
			)}
		</span>
	);
}
