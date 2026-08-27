import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import {
	Button,
	Input,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import { useTerminalCommandOutputViewer } from './hooks/useTerminalCommandOutputViewer';
import styles from './TerminalCommandOutputViewer.module.css';
import {
	MAX_TERMINAL_COMMAND_OUTPUT_QUERY_LENGTH,
	type TerminalCommandOutputViewerController,
} from './model/terminal-command-output-viewer-controller';

export interface TerminalCommandOutputViewerProps {
	readonly controller: TerminalCommandOutputViewerController;
	readonly onCopy: () => void;
	readonly platform: KeyboardShortcutPlatform;
	readonly restoreFocus: () => void;
}

export function TerminalCommandOutputViewer(
	props: TerminalCommandOutputViewerProps,
): React.ReactNode {
	const view = useTerminalCommandOutputViewer(props);
	if (!view.snapshot.isOpen || view.snapshot.document === null) return null;
	const document = view.snapshot.document;
	const output = document.kind === 'output';

	return (
		<div className={styles['backdrop']} data-slot='terminal-command-output-viewer'>
			<section
				aria-labelledby={view.titleId}
				aria-modal='true'
				className={styles['root']}
				onKeyDownCapture={view.onKeyDown}
				ref={view.rootReference}
				role='dialog'
			>
				<header className={styles['header']}>
					<div className={styles['heading']}>
						<ProductIcon name='terminal' />
						<h2 className={styles['title']} id={view.titleId}>
							{view.title}
						</h2>
					</div>
					{output ? (
						<div className={styles['search']}>
							<Input
								aria-describedby={view.statusId}
								aria-label={view.searchLabel}
								className={styles['input']}
								maxLength={MAX_TERMINAL_COMMAND_OUTPUT_QUERY_LENGTH}
								onChange={view.onQueryChange}
								placeholder={view.searchLabel}
								ref={view.inputReference}
								value={view.snapshot.query}
							/>
							<span className={styles['resultCount']}>
								{view.snapshot.query === '' ? null : view.statusText}
							</span>
							<Tooltip>
								<TooltipTrigger
									render={
										<Button
											aria-label={view.previousLabel}
											className={styles['action']}
											disabled={view.snapshot.matches.length === 0}
											onClick={() => props.controller.moveMatch(-1)}
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
											disabled={view.snapshot.matches.length === 0}
											onClick={() => props.controller.moveMatch(1)}
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
						</div>
					) : null}
					<div className={styles['headerActions']}>
						{output ? (
							<Tooltip>
								<TooltipTrigger
									render={
										<Button
											aria-label={view.copyLabel}
											onClick={props.onCopy}
											size='icon-sm'
											variant='ghost'
										/>
									}
								>
									<ProductIcon name='copy' />
								</TooltipTrigger>
								<TooltipContent side='bottom'>{view.copyLabel}</TooltipContent>
							</Tooltip>
						) : null}
						<Tooltip>
							<TooltipTrigger
								render={
									<Button
										aria-label={view.closeLabel}
										onClick={view.onClose}
										size='icon-sm'
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
				</header>
				<p aria-live='polite' className={styles['liveRegion']} id={view.statusId} role='status'>
					{view.statusText}
				</p>
				<div className={styles['body']}>
					{output ? (
						<>
							{view.truncatedText === null ? null : (
								<p className={styles['notice']}>{view.truncatedText}</p>
							)}
							<div
								aria-label={view.contentLabel}
								className={styles['content']}
								ref={view.contentReference}
								role='region'
								tabIndex={0}
							>
								<pre className={styles['pre']}>
									{view.segments.map((segment, index) =>
										segment.kind === 'text' ? (
											<React.Fragment key={`text-${index}`}>{segment.text}</React.Fragment>
										) : (
											<mark
												className={styles['match']}
												data-active={segment.active}
												key={`match-${segment.matchIndex}`}
											>
												{segment.text}
											</mark>
										),
									)}
								</pre>
							</div>
						</>
					) : (
						<div className={styles['empty']} role='status'>
							<ProductIcon name='terminal' />
							<p>{view.emptyText}</p>
						</div>
					)}
				</div>
			</section>
		</div>
	);
}
