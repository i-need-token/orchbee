import type { ReactElement } from 'react';

import { CopyButton } from '../copy-button/CopyButton';
import styles from './CommandBlock.module.css';

export type CommandLine =
	| { readonly kind: 'comment'; readonly text: string }
	| { readonly kind: 'command'; readonly text: string; readonly copyable?: boolean };

// Commands are shown as typed in a shell, so they stay in English in both languages; only the
// comments between them are translated.
export function CommandBlock({
	title,
	lines,
}: {
	title?: string;
	lines: readonly CommandLine[];
}): ReactElement {
	return (
		<div className={styles['block']} data-theme='dark'>
			{title === undefined ? null : <p className={styles['title']}>{title}</p>}
			<div className={styles['lines']}>
				{lines.map((line) =>
					line.kind === 'comment' ? (
						<p className={styles['comment']} key={`${line.kind}:${line.text}`}>
							# {line.text}
						</p>
					) : (
						<p className={styles['command']} key={`${line.kind}:${line.text}`}>
							<span className={styles['prompt']} aria-hidden='true'>
								$
							</span>
							<code>{line.text}</code>
							{line.copyable === true ? <CopyButton text={line.text} /> : null}
						</p>
					),
				)}
			</div>
		</div>
	);
}
