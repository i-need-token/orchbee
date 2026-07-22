import { type ReactNode } from 'react';
import { type HoverCardThumbnailVisual } from '../../../../hovercard';

export function ThumbnailVisualView({
	visual,
	fadingOut = false,
}: {
	visual: Readonly<HoverCardThumbnailVisual<string>>;
	fadingOut?: boolean;
}): ReactNode {
	return (
		<div className='ct-hover-card__preview-layer' data-fading-out={fadingOut || undefined}>
			{visual.presentation === 'thumbnail' && visual.image ? (
				<img src={visual.image} alt='' />
			) : null}
			{visual.presentation === 'crashed' ? <span>This tab has crashed</span> : null}
			{visual.presentation === 'placeholder' ? <span>Preview unavailable</span> : null}
		</div>
	);
}
