import type { ReactElement } from 'react';

import { joinClassNames } from '../../joinClassNames';
import { StatusDot } from '../status-dot/StatusDot';
import styles from './MirrorPrompt.module.css';

const MAX_PROMPT_LENGTH = 48;

// The prompt rows one client shows for claude-1: the line last sent, the working status while
// claude-1 answers, and a prompt with an invisible input over it. The page script fills the
// rows of every client from whichever input the visitor types into.
export function MirrorPrompt({
	label,
	variant,
}: {
	label: string;
	variant: 'desktop' | 'terminal' | 'phone';
}): ReactElement {
	return (
		<div className={styles[variant]}>
			<div className={styles['sent']} data-mirror-sent='' hidden>
				&gt; <span data-mirror-sent-text='' />
			</div>
			<div className={styles['line']} data-mirror-working='' hidden>
				<StatusDot state='working' place='text' />
				<span className={styles['strong']}> Working</span>
				{variant === 'phone' ? null : (
					<span className={styles['muted']}>{'  Esc to interrupt'}</span>
				)}
			</div>
			<div className={joinClassNames(styles['line'], styles['prompt'])}>
				<span className={styles['caret']}>&gt; </span>
				<span className={styles['view']}>
					<span className={styles['row']}>
						<span className={styles['typing']}>Run the tests again</span>
						<span data-mirror-draft='' />
						<i className={styles['cursor']} />
					</span>
				</span>
				<input
					className={styles['input']}
					type='text'
					aria-label={label}
					maxLength={MAX_PROMPT_LENGTH}
					spellCheck={false}
					autoComplete='off'
					data-mirror-input=''
				/>
			</div>
		</div>
	);
}
