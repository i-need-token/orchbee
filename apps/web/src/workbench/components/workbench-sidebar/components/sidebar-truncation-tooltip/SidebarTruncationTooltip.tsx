import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import * as React from 'react';

interface SidebarTruncationTooltipProps {
	readonly children: React.ReactNode;
	/** The whole text, shown below the row. */
	readonly content: string;
	/** The element that truncates; its own content or a descendant may be the part cut short. */
	readonly render: React.ReactElement;
	/** Clears what the row shows below the text, such as a Workspace's second line. */
	readonly sideOffset?: number | undefined;
}

function isTruncated(element: HTMLElement | null): boolean {
	if (element === null) return false;
	return [element, ...element.querySelectorAll<HTMLElement>('*')].some(
		(node) => node.scrollWidth > node.clientWidth,
	);
}

/** A row's text names itself in a tooltip only while the row cuts it short. */
export function SidebarTruncationTooltip({
	children,
	content,
	render,
	sideOffset,
}: SidebarTruncationTooltipProps): React.ReactNode {
	const [open, setOpen] = React.useState(false);
	const triggerReference = React.useRef<HTMLButtonElement>(null);
	return (
		<Tooltip
			open={open}
			onOpenChange={(next) => setOpen(next && isTruncated(triggerReference.current))}
		>
			<TooltipTrigger ref={triggerReference} render={render}>
				{children}
			</TooltipTrigger>
			<TooltipContent align='start' side='bottom' sideOffset={sideOffset}>
				{content}
			</TooltipContent>
		</Tooltip>
	);
}
