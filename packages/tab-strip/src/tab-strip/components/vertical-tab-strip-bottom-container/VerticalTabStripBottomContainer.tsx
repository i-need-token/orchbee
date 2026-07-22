import styles from './VerticalTabStripBottomContainer.module.css';
import type { CSSProperties, MouseEvent as ReactMouseEvent, ReactNode } from 'react';
import type { VerticalTabStripCollapseState } from '../../../common';
import { getVerticalBottomContainerPresentation } from '../../../vertical';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
import { FlatEdgeButton } from '../flat-edge-button/FlatEdgeButton';

export interface VerticalTabStripBottomContainerProps {
	collapseState: VerticalTabStripCollapseState;
	onNewTab: () => void;
	onContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}

export function VerticalTabStripBottomContainer({
	collapseState,
	onNewTab,
	onContextMenu,
}: VerticalTabStripBottomContainerProps): ReactNode {
	const presentation = getVerticalBottomContainerPresentation(collapseState);
	return (
		<div
			className={styles['ct-vertical-bottom-container']}
			data-orientation={presentation.orientation}
			data-collapsed-styling={presentation.collapsedStyling || undefined}
			style={
				{
					'--ob-comp-tab-strip-bottom-inset-block': `${presentation.insets.top}px`,
					'--ob-comp-tab-strip-bottom-inset-inline': `${presentation.insets.left}px`,
				} satisfies CSSProperties &
					Record<
						'--ob-comp-tab-strip-bottom-inset-block' | '--ob-comp-tab-strip-bottom-inset-inline',
						string
					>
			}
		>
			<FlatEdgeButton
				variantClass='ct-vertical-new-tab'
				type='button'
				aria-label='New tab'
				icon={<TabStripIcon name='plus' />}
				label='New tab'
				expansionOrientation={presentation.orientation}
				onClick={onNewTab}
				onContextMenu={onContextMenu}
			/>
		</div>
	);
}
