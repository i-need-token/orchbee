import type { ReactElement, ReactNode } from 'react';

import { joinClassNames } from '../../joinClassNames';
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
 * Features in a ruled grid, each with its title, explanation and keys: three columns for a client
 * page's features, or four for a short row of them, which takes two columns on a medium screen.
 * Either takes one column on a narrow screen. The page gives the content; the list rises into view
 * as one.
 */
export function Highlights({
	items,
	columns = 3,
}: {
	items: readonly Highlight[];
	columns?: 3 | 4;
}): ReactElement {
	return (
		<div className={joinClassNames(styles['grid'], columns === 4 && styles['four'])} data-reveal=''>
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
