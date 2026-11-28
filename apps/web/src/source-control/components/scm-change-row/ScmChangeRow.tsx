import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import {
	SCM_STATUS_FACTS,
	type ScmGroupId,
	type ScmResourceStatus,
} from '../../model/scm-resource';
import type { ScmTreeItem } from '../../model/scm-tree';
import type { ScmViewMode } from '../../model/scm-view-state';
import styles from './ScmChangeRow.module.css';

type ScmChangeItem = Extract<ScmTreeItem, { readonly kind: 'folder' | 'group' | 'resource' }>;

interface ScmChangeRowProps {
	readonly item: ScmChangeItem;
	/** List mode describes each file by its folder; tree mode nests it there instead. */
	readonly mode: ScmViewMode;
	readonly separator: '/' | '\\';
	/** A status's hover text. */
	readonly statusText: (status: ScmResourceStatus) => string;
}

const GROUP_MESSAGES = {
	index: sourceControlMessages.groupIndex,
	merge: sourceControlMessages.groupMerge,
	workingTree: sourceControlMessages.groupWorkingTree,
} as const satisfies Record<ScmGroupId, unknown>;

/** The folder holding a file, relative to its repository; empty for a file at the root. */
function relativeFolder(path: string, repository: string, separator: string): string {
	const prefix = repository.endsWith(separator) ? repository : `${repository}${separator}`;
	const relative = path.startsWith(prefix) ? path.slice(prefix.length) : path;
	const end = relative.lastIndexOf(separator);
	return end < 0 ? '' : relative.slice(0, end);
}

/**
 * One Changes row below a repository: a group with its count, a folder in tree view, or a file with
 * its folder in list view and its status letter, colored as VS Code colors only the letter.
 */
export function ScmChangeRow({ item, mode, separator, statusText }: ScmChangeRowProps): ReactNode {
	const intl = useOrchbeeIntl();
	switch (item.kind) {
		case 'group':
			return (
				<span className={styles['root']}>
					<span className={styles['name']}>{intl.formatMessage(GROUP_MESSAGES[item.group])}</span>
					<span className={styles['count']}>{intl.formatNumber(item.resources.length)}</span>
				</span>
			);
		case 'folder':
			return (
				<span className={styles['root']} title={item.path}>
					<span className={styles['name']}>{item.names.join(separator)}</span>
				</span>
			);
		case 'resource': {
			const { path, status } = item.resource;
			const facts = SCM_STATUS_FACTS[status];
			const folder = mode === 'list' ? relativeFolder(path, item.repository, separator) : '';
			const text = statusText(status);
			return (
				<span className={styles['root']} title={path}>
					<span
						className={styles['name']}
						data-struck-through={facts.struckThrough ? '' : undefined}
					>
						{item.name}
					</span>
					{folder === '' ? null : <span className={styles['description']}>{folder}</span>}
					<Tooltip>
						<TooltipTrigger
							render={
								<span
									aria-label={text}
									className={styles['letter']}
									data-git-color={facts.color}
									role='img'
								/>
							}
						>
							{facts.letter}
						</TooltipTrigger>
						<TooltipContent side='bottom'>{text}</TooltipContent>
					</Tooltip>
				</span>
			);
		}
	}
}
