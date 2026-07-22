import {
	Button,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import styles from './WorkbenchTabTrailingAction.module.css';

interface WorkbenchTabTrailingActionProps {
	readonly closeLabel: string;
	readonly isPinned: boolean;
	readonly onClose: (() => void) | undefined;
	readonly onPointerCloseStart: React.PointerEventHandler<HTMLButtonElement>;
	readonly onUnpin: (() => void) | undefined;
	readonly tabId: string;
	readonly unpinLabel: string;
}

export function WorkbenchTabTrailingAction({
	closeLabel,
	isPinned,
	onClose,
	onPointerCloseStart,
	onUnpin,
	tabId,
	unpinLabel,
}: WorkbenchTabTrailingActionProps): React.ReactNode {
	if (isPinned) {
		return (
			<Tooltip>
				<TooltipTrigger
					render={
						<Button
							aria-label={unpinLabel}
							className={styles['pin']}
							data-workbench-tab-action='unpin'
							data-workbench-unpin-tab-id={tabId}
							disabled={onUnpin === undefined}
							size='icon-xs'
							type='button'
							variant='ghost'
							onClick={onUnpin}
						/>
					}
				>
					<ProductIcon name='pin' />
				</TooltipTrigger>
				<TooltipContent side='bottom'>{unpinLabel}</TooltipContent>
			</Tooltip>
		);
	}

	if (onClose === undefined) return null;
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<Button
						aria-label={closeLabel}
						className={styles['close']}
						data-workbench-tab-action='close'
						data-workbench-close-tab-id={tabId}
						size='icon-xs'
						type='button'
						variant='ghost'
						onClick={onClose}
						onPointerDown={onPointerCloseStart}
					/>
				}
			>
				<ProductIcon name='close' />
			</TooltipTrigger>
			<TooltipContent side='bottom'>{closeLabel}</TooltipContent>
		</Tooltip>
	);
}
