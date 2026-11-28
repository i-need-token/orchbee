import {
	FileIcon,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import {
	VirtualTreeLabel,
	VirtualTreeSegments,
	type VirtualTreeRow,
	type VirtualTreeRowState,
} from '../../../virtual-tree';
import type { ExplorerGitDecorations } from '../../model/ExplorerGitDecorations';
import type { ExplorerItem } from '../../model/explorer-item';
import {
	useExplorerRowDecoration,
	type ExplorerRowDecoration,
} from './hooks/useExplorerRowDecoration';
import styles from './ExplorerRow.module.css';

interface ExplorerRowProps {
	/** The session's Git decorations; null while there are none to draw. */
	readonly decorations: ExplorerGitDecorations | null;
	readonly row: VirtualTreeRow<string, ExplorerItem>;
	/** The row's state as the tree draws it, which the names of a compact row take. */
	readonly state: VirtualTreeRowState;
}

interface ExplorerRowMarkProps {
	readonly badge: NonNullable<ExplorerRowDecoration['badge']>;
	readonly color: ExplorerRowDecoration['color'];
	/** What the mark means, as its name and tooltip say it. */
	readonly tooltip: string;
}

function ExplorerRowMark({ badge, color, tooltip }: ExplorerRowMarkProps): ReactNode {
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<span
						aria-label={tooltip}
						className={styles['mark']}
						data-descendants={badge.kind === 'descendants' ? '' : undefined}
						data-git-color={color ?? undefined}
						role='img'
					/>
				}
			>
				{badge.kind === 'letters' ? badge.text : <ProductIcon name='contains-emphasized' />}
			</TooltipTrigger>
			<TooltipContent side='bottom'>{tooltip}</TooltipContent>
		</Tooltip>
	);
}

/**
 * One Explorer row's label: a file's Seti icon, since Seti has no folder icons, the entry's name,
 * and VS Code's decorations. Git's color tints the name and the mark at the far end, which shows
 * the row's letters, Git's ahead of the Explorer's own ⤷ for a link or ? for an entry that is
 * neither a file nor a folder; a folder with changes below it shows a dot in their color instead.
 * A compact row names each of its folders, as VS Code's compact folders do, and carries the
 * decorations of the last. An entry `files.exclude` hides, which shows only on the way to the
 * overlay's active file, is deemphasized as VS Code's Explorer draws it, unless Git colors it. The
 * characters the tree's find matched are highlighted in the names.
 */
export function ExplorerRow({ decorations, row, state }: ExplorerRowProps): ReactNode {
	const item = row.element;
	const decoration = useExplorerRowDecoration(decorations, item);
	return (
		<span className={styles['root']}>
			{item.directory ? null : <FileIcon name={item.name} path={item.path} />}
			<span
				className={styles['name']}
				data-excluded={item.excluded ? '' : undefined}
				data-git-color={decoration?.color ?? undefined}
			>
				{state.compact === null ? (
					<VirtualTreeLabel highlights={state.highlights} text={item.name} />
				) : (
					<VirtualTreeSegments
						names={row.segments.map((segment) => segment.element.name)}
						state={state}
					/>
				)}
			</span>
			{decoration !== null && decoration.badge !== null ? (
				<ExplorerRowMark
					badge={decoration.badge}
					color={decoration.color}
					tooltip={decoration.tooltip}
				/>
			) : null}
		</span>
	);
}
