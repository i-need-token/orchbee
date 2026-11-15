import type { ReactElement, ReactNode } from 'react';

import styles from './CommandGroups.module.css';

/** A titled group of commands, each beside what it does. */
export interface CommandGroup {
	readonly id: string;
	readonly title: ReactNode;
	readonly rows: readonly {
		/** The command as typed; never translated. */
		readonly command: string;
		readonly label: ReactNode;
	}[];
}

/**
 * Command reference tables, laid out as KeyGroups lays out keys: groups in two columns (one on
 * narrow screens), each a ruled list of commands with what they do. The page gives the content;
 * the tables rise into view as one.
 */
export function CommandGroups({ groups }: { groups: readonly CommandGroup[] }): ReactElement {
	return (
		<div className={styles['groups']} data-reveal=''>
			{groups.map((group) => (
				<section key={group.id}>
					<h3 className={styles['title']}>{group.title}</h3>
					{group.rows.map((row) => (
						<div key={row.command} className={styles['row']}>
							<b className={styles['command']}>{row.command}</b>
							<span>{row.label}</span>
						</div>
					))}
				</section>
			))}
		</div>
	);
}
