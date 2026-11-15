import type { ReactElement, ReactNode } from 'react';

import { joinClassNames } from '../../joinClassNames';
import { Kicker } from '../kicker/Kicker';
import styles from './SectionHead.module.css';

// A section's label, heading and paragraph. `split` sets the paragraph beside the heading and
// reveals the block on scroll; `stack` sets it below, for a column that reveals as a whole.
export function SectionHead({
	kicker,
	title,
	lead,
	layout = 'split',
}: {
	kicker: ReactNode;
	title: ReactNode;
	lead: ReactNode;
	layout?: 'split' | 'stack';
}): ReactElement {
	return (
		<div
			className={joinClassNames(styles['head'], layout === 'split' && styles['split'])}
			data-reveal={layout === 'split' ? '' : undefined}
		>
			<div>
				<Kicker>{kicker}</Kicker>
				<h2 className={styles['title']}>{title}</h2>
			</div>
			<p className={styles['lead']}>{lead}</p>
		</div>
	);
}
