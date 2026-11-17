import type { ReactElement, ReactNode } from 'react';

import { KeyCaps } from '../key-caps/KeyCaps';
import styles from './KeyGroups.module.css';

/** A titled group of actions, each beside its keys, and beside its macOS keys in a paired table. */
export interface KeyGroup {
	readonly id: string;
	readonly title: ReactNode;
	readonly rows: readonly {
		readonly id: string;
		readonly label: ReactNode;
		readonly keys: string;
		readonly macosKeys?: string;
	}[];
}

/**
 * Key reference tables: groups in two columns (one on narrow screens), each a ruled list of
 * actions with their keys, written as KeyCaps reads them. With `platforms`, each group heads two
 * key columns with those names, the rows' `keys` under the first and their `macosKeys` under the
 * second. The page gives the content; the tables rise into view as one.
 */
export function KeyGroups({
	groups,
	platforms,
}: {
	groups: readonly KeyGroup[];
	platforms?: readonly [ReactNode, ReactNode] | undefined;
}): ReactElement {
	return (
		<div className={styles['groups']} data-reveal=''>
			{groups.map((group) =>
				platforms === undefined ? (
					<section key={group.id}>
						<h3 className={styles['title']}>{group.title}</h3>
						{group.rows.map((row) => (
							<div key={row.id} className={styles['row']}>
								<span>{row.label}</span>
								<span className={styles['keys']}>
									<KeyCaps keys={row.keys} />
								</span>
							</div>
						))}
					</section>
				) : (
					<section key={group.id} className={styles['paired']}>
						<div className={styles['head']}>
							<h3 className={styles['pairedTitle']}>{group.title}</h3>
							<span className={styles['platform']}>{platforms[0]}</span>
							<span className={styles['platform']}>{platforms[1]}</span>
						</div>
						{group.rows.map((row) => (
							<div key={row.id} className={styles['pairedRow']}>
								<span className={styles['pairedLabel']}>{row.label}</span>
								<span className={styles['pairedKeys']}>
									<KeyCaps keys={row.keys} />
								</span>
								<span className={styles['pairedKeys']}>
									<KeyCaps keys={row.macosKeys ?? row.keys} />
								</span>
							</div>
						))}
					</section>
				),
			)}
		</div>
	);
}
