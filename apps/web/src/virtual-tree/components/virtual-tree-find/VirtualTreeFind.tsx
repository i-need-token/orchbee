import {
	Button,
	Input,
	Kbd,
	ProductIcon,
	Toggle,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type * as React from 'react';

import type { TreeFind } from '../../find/TreeFind';
import { useVirtualTreeFind } from './hooks/useVirtualTreeFind';
import styles from './VirtualTreeFind.module.css';

interface VirtualTreeFindProps<K, T> {
	readonly find: TreeFind<K, T>;
	readonly platform: KeyboardShortcutPlatform;
}

/**
 * VS Code's tree find widget as a band above the rows while the find is open: the input with its
 * Filter and Fuzzy Match toggles, the close button, and under them, over the first rows, the message
 * that nothing matched or the provider's warning. A live region beside it announces each result
 * politely.
 */
export function VirtualTreeFind<K, T>({
	find,
	platform,
}: VirtualTreeFindProps<K, T>): React.ReactNode {
	const view = useVirtualTreeFind(find, platform);
	const { announcement, matchType, mode, open, value } = view.snapshot;
	return (
		<>
			{open ? (
				<div
					aria-label={view.label}
					className={styles['band']}
					data-slot='virtual-tree-find'
					onKeyDown={view.onKeyDown}
					role='search'
				>
					<div className={styles['field']}>
						<Input
							aria-describedby={view.messageText === null ? undefined : view.messageId}
							aria-label={view.inputLabel}
							className={styles['input']}
							onChange={view.onInputChange}
							onKeyDown={view.onInputKeyDown}
							placeholder={view.placeholder}
							ref={view.inputReference}
							spellCheck={false}
							value={value}
						/>
						<div className={styles['toggles']}>
							<Tooltip>
								<TooltipTrigger
									render={
										<Toggle
											aria-label={view.filterLabel}
											className={styles['toggle']}
											onPressedChange={(pressed, details) =>
												view.onFilterChange(pressed, details.event)
											}
											pressed={mode === 'filter'}
											size='sm'
											variant='default'
										/>
									}
								>
									<ProductIcon name='list-filter' />
								</TooltipTrigger>
								<TooltipContent side='bottom'>{view.filterLabel}</TooltipContent>
							</Tooltip>
							<Tooltip>
								<TooltipTrigger
									render={
										<Toggle
											aria-label={view.fuzzyMatchLabel}
											className={styles['toggle']}
											onPressedChange={(pressed, details) =>
												view.onFuzzyMatchChange(pressed, details.event)
											}
											pressed={matchType === 'fuzzy'}
											size='sm'
											variant='default'
										/>
									}
								>
									<ProductIcon name='search-fuzzy' />
								</TooltipTrigger>
								<TooltipContent side='bottom'>{view.fuzzyMatchLabel}</TooltipContent>
							</Tooltip>
						</div>
					</div>
					<Tooltip>
						<TooltipTrigger
							render={
								<Button
									aria-label={view.closeLabel}
									className={styles['close']}
									onClick={view.onClose}
									size='icon-xs'
									variant='ghost'
								/>
							}
						>
							<ProductIcon name='close' />
						</TooltipTrigger>
						<TooltipContent side='bottom'>
							{view.closeLabel}
							<Kbd>Esc</Kbd>
						</TooltipContent>
					</Tooltip>
					{view.messageText === null ? null : (
						<p className={styles['message']} id={view.messageId}>
							{view.messageText}
						</p>
					)}
				</div>
			) : null}
			<p aria-live='polite' className={styles['liveRegion']} role='status'>
				{announcement === null ? null : (
					<span key={announcement.serial}>{view.announcementText}</span>
				)}
			</p>
		</>
	);
}
