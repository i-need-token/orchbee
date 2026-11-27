import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './CodeEditorPlaceholder.module.css';

/** One button under the message, such as Open Anyway or Try Again. */
interface CodeEditorPlaceholderAction {
	readonly label: string;
	readonly run: () => void;
}

interface CodeEditorPlaceholderProps {
	readonly actions: readonly CodeEditorPlaceholderAction[];
	readonly message: string;
}

/**
 * VS Code's editor placeholder: why the editor shows no content, with the actions that can change
 * that. The message stays selectable, as VS Code's is, so it can be copied.
 */
export function CodeEditorPlaceholder({ actions, message }: CodeEditorPlaceholderProps): ReactNode {
	return (
		<div className={styles['root']} data-slot='code-editor-placeholder'>
			<div className={styles['content']}>
				<p className={styles['message']}>{message}</p>
				{actions.length === 0 ? null : (
					<div className={styles['actions']}>
						{actions.map((action) => (
							<Button key={action.label} onClick={action.run}>
								{action.label}
							</Button>
						))}
					</div>
				)}
			</div>
		</div>
	);
}
