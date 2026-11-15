import type { ReactElement, ReactNode } from 'react';

import { KeyCaps } from '../key-caps/KeyCaps';
import styles from './Highlights.module.css';

/** One highlight: a title, a short explanation, and optionally the keys that reach it. */
export interface Highlight {
	readonly id: string;
	readonly title: ReactNode;
	readonly body: ReactNode;
	/** Written as KeyCaps reads it. */
	readonly keys?: string;
}

/**
 * A client page's features in a ruled grid of three columns (one on narrow screens), each with
 * its title, explanation and keys. The page gives the content; the list rises into view as one.
 */
export function Highlights({ items }: { items: readonly Highlight[] }): ReactElement {
	return (
		<div className={styles['grid']} data-reveal=''>
			{items.map((item) => (
				<article key={item.id} className={styles['item']}>
					<h3 className={styles['title']}>{item.title}</h3>
					<p className={styles['body']}>{item.body}</p>
					{item.keys === undefined ? null : (
						<div className={styles['keys']}>
							<KeyCaps keys={item.keys} />
						</div>
					)}
				</article>
			))}
		</div>
	);
}
