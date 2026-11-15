import type { ReactElement } from 'react';

import { joinClassNames } from '../../joinClassNames';
import { CopyButton } from '../copy-button/CopyButton';
import styles from './CommandChip.module.css';

// One command in a bordered chip, with its copy button. Commands are never translated.
export function CommandChip({
	command,
	tone = 'bright',
}: {
	command: string;
	/** `quiet` sets the command in the secondary text color, as the install rows do. */
	tone?: 'bright' | 'quiet';
}): ReactElement {
	return (
		<div className={joinClassNames(styles['chip'], tone === 'quiet' && styles['quiet'])}>
			<span className={styles['prompt']} aria-hidden='true'>
				&gt;
			</span>
			<code className={styles['command']}>{command}</code>
			<CopyButton text={command} />
		</div>
	);
}
