import type { ReactElement, ReactNode } from 'react';

import type { TuiLine, TuiSegment } from '../../../tui-grid';
import { joinClassNames } from '../../joinClassNames';
import styles from './TuiScreen.module.css';

export type TuiScreenSize = 'hero' | 'compact' | 'mirror' | 'board' | 'orchestrate';

// A live-rendered Orchbee terminal interface. The page script repaints the cells with
// renderTuiLines; `children` are page controls laid over the cells in cell units.
export function TuiScreen({
	lines,
	size,
	label,
	role = 'img',
	hook,
	show = 'always',
	children,
}: {
	lines: readonly TuiLine[];
	size: TuiScreenSize;
	label: string;
	role?: 'img' | 'group';
	hook?: string;
	show?: 'always' | 'wide' | 'narrow';
	children?: ReactNode;
}): ReactElement {
	return (
		<div
			className={joinClassNames(styles['screen'], styles[`show-${show}`])}
			role={role}
			aria-label={label}
			data-tui={hook}
		>
			<div className={joinClassNames(styles['cells'], styles[size])}>
				<div data-tui-lines=''>
					{lines.map((line, row) => (
						<div className={styles['line']} key={row}>
							{line.map((segment, index) => (
								<Cell key={index} segment={segment} />
							))}
						</div>
					))}
				</div>
				{children}
			</div>
		</div>
	);
}

function Cell({ segment }: { segment: TuiSegment }): ReactElement {
	const className = joinClassNames(
		styles[`tone-${segment.tone}`],
		styles[`surface-${segment.surface}`],
		segment.shape !== undefined && styles['shape'],
		segment.shape !== undefined && styles[`shape-${segment.shape}`],
		segment.motion !== undefined && styles[`motion-${segment.motion}`],
	);
	return segment.bold ? (
		<b className={className}>{segment.text}</b>
	) : (
		<span className={className}>{segment.text}</span>
	);
}
