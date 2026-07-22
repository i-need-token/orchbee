import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
	ReactNode,
} from 'react';
import type { VerticalTabStripCollapseState } from '../../../../common';
import { getVerticalBottomContainerPresentation } from '../../../../vertical';
import { FlatEdgeButton } from '../flat-edge-button/FlatEdgeButton';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
export interface VerticalTabStripBottomContainerProps {
	collapseState: VerticalTabStripCollapseState;
	onNewTab: () => void;
	onNewTabFromClipboard?: () => void;
	onContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}
interface VerticalTabStripBottomContainerStyle extends CSSProperties {
	'--ct-bottom-inset-block': string;
	'--ct-bottom-inset-inline': string;
}
export function VerticalTabStripBottomContainer({
	collapseState,
	onNewTab,
	onNewTabFromClipboard,
	onContextMenu,
}: VerticalTabStripBottomContainerProps): ReactNode {
	const presentation = getVerticalBottomContainerPresentation(collapseState);
	const style: VerticalTabStripBottomContainerStyle = {
		'--ct-bottom-inset-block': `${presentation.insets.top}px`,
		'--ct-bottom-inset-inline': `${presentation.insets.left}px`,
	};
	return (
		<div
			className='ct-vertical-bottom-container'
			data-orientation={presentation.orientation}
			data-collapsed-styling={presentation.collapsedStyling || undefined}
			style={style}
		>
			<FlatEdgeButton
				className='ct-vertical-new-tab'
				type='button'
				aria-label='New tab'
				icon={<TabStripIcon name='plus' />}
				label='New tab'
				expansionOrientation={presentation.orientation}
				onClick={onNewTab}
				onPointerDown={(event: ReactPointerEvent<HTMLButtonElement>) => {
					if (event.button !== 1) return;
					event.preventDefault();
					event.stopPropagation();
					onNewTabFromClipboard?.();
				}}
				onAuxClick={(event: ReactMouseEvent<HTMLButtonElement>) => {
					if (event.button !== 1) return;
					event.preventDefault();
				}}
				onContextMenu={onContextMenu}
			/>
		</div>
	);
}
