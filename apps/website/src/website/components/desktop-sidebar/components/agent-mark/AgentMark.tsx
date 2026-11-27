import { useId, type ReactElement } from 'react';

import type { AgentMarkDrawing } from './agent-mark-drawings';
import styles from './AgentMark.module.css';

function fillOf(paint: AgentMarkDrawing['paint'], gradientId: string): string {
	switch (paint.kind) {
		case 'text':
			return 'currentColor';
		case 'color':
			return paint.color;
		case 'gradient':
			return `url(#${gradientId})`;
	}
}

/** An Agent's product mark at the end of its desktop sidebar row, in the box the product gives it. */
export function AgentMark({ drawing }: { drawing: AgentMarkDrawing }): ReactElement {
	const gradientId = useId();
	const { paint } = drawing;
	return (
		<span className={styles['mark']}>
			<svg viewBox={drawing.viewBox} overflow='visible' aria-hidden='true'>
				<path
					d={drawing.path}
					fill={fillOf(paint, gradientId)}
					fillRule={drawing.evenOdd ? 'evenodd' : undefined}
				/>
				{paint.kind === 'gradient' ? (
					<defs>
						<linearGradient
							gradientUnits='userSpaceOnUse'
							id={gradientId}
							x1={paint.line[0]}
							y1={paint.line[1]}
							x2={paint.line[2]}
							y2={paint.line[3]}
						>
							{paint.stops.map(([offset, color]) => (
								<stop key={offset} offset={offset} stopColor={color} />
							))}
						</linearGradient>
					</defs>
				) : null}
			</svg>
		</span>
	);
}
