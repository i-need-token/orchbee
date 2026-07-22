import type { CSSProperties, MouseEvent as ReactMouseEvent, ReactNode } from 'react';
import type { VerticalTabStripCollapseState } from '../../../../common';
import { getVerticalBottomContainerPresentation } from '../../../../vertical';
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
			className='ct-vertical-bottom-container'
			data-orientation={presentation.orientation}
			data-collapsed-styling={presentation.collapsedStyling || undefined}
			style={
				{
					'--ct-bottom-inset-block': `${presentation.insets.top}px`,
					'--ct-bottom-inset-inline': `${presentation.insets.left}px`,
				} satisfies CSSProperties &
					Record<'--ct-bottom-inset-block' | '--ct-bottom-inset-inline', string>
			}
		>
			<FlatEdgeButton
				className='ct-vertical-new-tab'
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
