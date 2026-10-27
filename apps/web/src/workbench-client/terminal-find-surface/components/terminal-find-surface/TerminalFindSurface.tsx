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
import type * as React from 'react';

import { useTerminalFindSurface } from './hooks/useTerminalFindSurface';
import styles from './TerminalFindSurface.module.css';
import type { TerminalFindController } from '../../TerminalFindController';

export interface TerminalFindSurfaceProps {
	readonly controller: TerminalFindController;
	readonly onNext: () => void;
	readonly onPrevious: () => void;
	readonly restoreFocus: () => void;
}

export function TerminalFindSurface(props: TerminalFindSurfaceProps): React.ReactNode {
	const view = useTerminalFindSurface(props);
	if (!view.snapshot.isOpen) return null;

	return (
		<section
			aria-label={view.findLabel}
			className={styles['root']}
			data-search-state={view.snapshot.searchState}
			onKeyDown={view.onSurfaceKeyDown}
			role='search'
		>
			<div className={styles['queryField']} data-slot='terminal-find-query'>
				<Input
					aria-describedby={view.statusId}
					aria-invalid={view.snapshot.searchErrorKind === 'invalid-query'}
					aria-label={view.findLabel}
					className={styles['input']}
					onChange={view.onInputChange}
					onKeyDown={view.onInputKeyDown}
					placeholder={view.placeholder}
					ref={view.inputReference}
					value={view.snapshot.query}
				/>
				<div className={styles['optionControls']} onKeyDown={view.onOptionControlsKeyDown}>
					<Tooltip>
						<TooltipTrigger
							render={
								<Toggle
									aria-label={view.caseSensitiveLabel}
									className={styles['option']}
									onPressedChange={(pressed, details) =>
										view.onCaseSensitiveChange(pressed, details.event)
									}
									pressed={view.snapshot.caseSensitive}
									size='sm'
									variant='default'
								/>
							}
						>
							Aa
						</TooltipTrigger>
						<TooltipContent side='bottom'>{view.caseSensitiveLabel}</TooltipContent>
					</Tooltip>
					<Tooltip>
						<TooltipTrigger
							render={
								<Toggle
									aria-label={view.wholeWordLabel}
									className={styles['option']}
									onPressedChange={(pressed, details) =>
										view.onWholeWordChange(pressed, details.event)
									}
									pressed={view.snapshot.wholeWord}
									size='sm'
									variant='default'
								/>
							}
						>
							W
						</TooltipTrigger>
						<TooltipContent side='bottom'>{view.wholeWordLabel}</TooltipContent>
					</Tooltip>
					<Tooltip>
						<TooltipTrigger
							render={
								<Toggle
									aria-label={view.regexLabel}
									className={styles['option']}
									onPressedChange={(pressed, details) => view.onRegexChange(pressed, details.event)}
									pressed={view.snapshot.regex}
									size='sm'
									variant='default'
								/>
							}
						>
							.*
						</TooltipTrigger>
						<TooltipContent side='bottom'>{view.regexLabel}</TooltipContent>
					</Tooltip>
				</div>
			</div>
			{view.compactStatusText === null ? null : (
				<span
					aria-hidden='true'
					className={styles['resultCount']}
					data-slot='terminal-find-result-count'
				>
					{view.compactStatusText}
				</span>
			)}
			<div className={styles['actions']}>
				<Tooltip>
					<TooltipTrigger
						render={
							<Button
								aria-label={view.previousLabel}
								className={styles['action']}
								disabled={view.snapshot.resultCount === 0}
								onClick={view.onPrevious}
								size='icon-xs'
								variant='ghost'
							/>
						}
					>
						<span className={styles['previousIcon']}>
							<ProductIcon name='arrow-down' />
						</span>
					</TooltipTrigger>
					<TooltipContent side='bottom'>
						{view.previousLabel}
						<Kbd>⇧↵</Kbd>
					</TooltipContent>
				</Tooltip>
				<Tooltip>
					<TooltipTrigger
						render={
							<Button
								aria-label={view.nextLabel}
								className={styles['action']}
								disabled={view.snapshot.resultCount === 0}
								onClick={view.onNext}
								size='icon-xs'
								variant='ghost'
							/>
						}
					>
						<ProductIcon name='arrow-down' />
					</TooltipTrigger>
					<TooltipContent side='bottom'>
						{view.nextLabel}
						<Kbd>↵</Kbd>
					</TooltipContent>
				</Tooltip>
				<Tooltip>
					<TooltipTrigger
						render={
							<Button
								aria-label={view.closeLabel}
								className={styles['action']}
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
			</div>
			{view.messageStatusText === null ? null : (
				<p
					aria-hidden='true'
					className={styles['message']}
					data-slot='terminal-find-message'
					data-tone={view.snapshot.searchState === 'error' ? 'danger' : 'muted'}
				>
					{view.messageStatusText}
				</p>
			)}
			<p aria-live='polite' className={styles['liveRegion']} id={view.statusId} role='status'>
				{view.statusText}
			</p>
		</section>
	);
}
