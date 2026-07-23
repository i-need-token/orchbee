import styles from './ThumbnailVisualView.module.css';
import { type ReactNode } from 'react';
import { type HoverCardThumbnailVisual } from '../../model/tab-hover-card-thumbnail-runtime';
import { useTabStripMessages } from '../../../../../../hooks/useTabStripMessages';

export function ThumbnailVisualView({
	visual,
	fadingOut = false,
}: {
	visual: Readonly<HoverCardThumbnailVisual<string>>;
	fadingOut?: boolean;
}): ReactNode {
	const messages = useTabStripMessages();
	return (
		<div
			className={styles['ct-hover-card__preview-layer']}
			data-fading-out={fadingOut || undefined}
		>
			{visual.presentation === 'thumbnail' && visual.image ? (
				<img src={visual.image} alt='' />
			) : null}
			{visual.presentation === 'crashed' ? <span>{messages.tabCrashed}</span> : null}
			{visual.presentation === 'placeholder' ? <span>{messages.previewUnavailable}</span> : null}
		</div>
	);
}
