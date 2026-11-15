import type { ReactElement, ReactNode } from 'react';

import { KeyCaps } from '../key-caps/KeyCaps';
import styles from './KeyGroups.module.css';

/** A titled group of actions, each beside its keys. */
export interface KeyGroup {
	readonly id: string;
	readonly title: ReactNode;
	readonly rows: readonly {
		readonly id: string;
		readonly label: ReactNode;
		readonly keys: string;
	}[];
}

/**
 * Key reference tables: groups in two columns (one on narrow screens), each a ruled list of
 * actions with their keys, written as KeyCaps reads them. The page gives the content; the tables
 * rise into view as one.
 */
export function KeyGroups({ groups }: { groups: readonly KeyGroup[] }): ReactElement {
	return (
		<div className={styles['groups']} data-reveal=''>
			{groups.map((group) => (
				<section key={group.id}>
					<h4 className={styles['title']}>{group.title}</h4>
					{group.rows.map((row) => (
						<div key={row.id} className={styles['row']}>
							<span>{row.label}</span>
							<span className={styles['keys']}>
								<KeyCaps keys={row.keys} />
							</span>
						</div>
					))}
				</section>
			))}
		</div>
	);
}
