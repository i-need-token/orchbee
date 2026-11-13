import type { ReactElement } from 'react';

export type IconName = 'dl' | 'gh' | 'copy' | 'ok' | 'snd' | 'win' | 'replay';

// One symbol from IconSprite at a fixed size, drawn in the current text color.
export function Icon({ name, size }: { name: IconName; size: number }): ReactElement {
	return (
		<svg width={size} height={size} aria-hidden='true'>
			<use href={`#i-${name}`} />
		</svg>
	);
}
