import type { ReactNode } from 'react';

import type { WorkbenchSidebarWorkspaceLocation } from '../../../../hooks/useWorkbenchSidebarWorkspaceLocations';
import styles from './WorkspaceLocationLine.module.css';

interface WorkspaceLocationLineProps {
	readonly line: NonNullable<WorkbenchSidebarWorkspaceLocation['line']>;
}

/** Splits a path before its final folder, keeping that separator with the folder. */
function splitFinalFolder(path: string): readonly [string, string] {
	const separator = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
	return separator <= 0 ? ['', path] : [path.slice(0, separator), path.slice(separator)];
}

/**
 * A Workspace's branch, cut at its end, or its folder, which is a path cut in the middle: the start
 * gives way first, so the final folder stays whole while it fits. The pointer shows the full text.
 */
export function WorkspaceLocationLine({ line }: WorkspaceLocationLineProps): ReactNode {
	if (line.kind === 'branch') {
		return (
			<span className={styles['line']} data-kind='branch' title={line.text}>
				{line.text}
			</span>
		);
	}
	const [start, finalFolder] = splitFinalFolder(line.text);
	return (
		<span className={styles['line']} data-kind='folder' title={line.text}>
			{start === '' ? null : <span className={styles['start']}>{start}</span>}
			<span className={styles['finalFolder']}>{finalFolder}</span>
		</span>
	);
}
