import {
	Button,
	Kbd,
	Popover,
	PopoverClose,
	PopoverContent,
	PopoverDescription,
	PopoverHeader,
	PopoverTitle,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchConfirmationAnchor } from '../../types/workbench-confirmation';
import { useWorkbenchConfirmationPopover } from './hooks/useWorkbenchConfirmationPopover';
import styles from './WorkbenchConfirmationPopover.module.css';

interface WorkbenchConfirmationPopoverProps {
	readonly anchor: WorkbenchConfirmationAnchor;
	readonly cancelLabel: string;
	readonly confirmLabel: string;
	readonly description: string;
	readonly onCancel: () => void;
	readonly onConfirm: () => void;
	readonly title: string;
}

export function WorkbenchConfirmationPopover({
	anchor,
	cancelLabel,
	confirmLabel,
	description,
	onCancel,
	onConfirm,
	title,
}: WorkbenchConfirmationPopoverProps): React.ReactNode {
	const view = useWorkbenchConfirmationPopover(anchor);

	return (
		<Popover
			modal='trap-focus'
			open
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
		>
			<PopoverContent
				align='start'
				anchor={view.virtualAnchor}
				data-anchor-x={anchor.x}
				data-anchor-y={anchor.y}
				data-slot='workbench-confirmation-popover'
				initialFocus={view.confirmButtonRef}
				side='right'
			>
				<PopoverHeader>
					<PopoverTitle>{title}</PopoverTitle>
					<PopoverDescription>{description}</PopoverDescription>
				</PopoverHeader>
				<div className={styles['footer']}>
					<PopoverClose render={<Button size='sm' variant='outline' />}>
						{cancelLabel}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Esc
						</Kbd>
					</PopoverClose>
					<Button ref={view.confirmButtonRef} size='sm' variant='destructive' onClick={onConfirm}>
						{confirmLabel}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Enter
						</Kbd>
					</Button>
				</div>
			</PopoverContent>
		</Popover>
	);
}
