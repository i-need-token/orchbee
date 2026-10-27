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

import { useTerminalTextViewer } from './hooks/useTerminalTextViewer';
import styles from './TerminalTextViewer.module.css';
import {
	MAX_TERMINAL_TEXT_VIEWER_QUERY_LENGTH,
	type TerminalTextViewerController,
	type TerminalTextViewerSource,
} from '../../terminal-text-viewer-controller';

export interface TerminalTextViewerProps {
	readonly controller: TerminalTextViewerController;
	readonly onCopy: (source: TerminalTextViewerSource) => void;
	readonly platform: KeyboardShortcutPlatform;
	readonly restoreFocus: () => void;
}

export function TerminalTextViewer(props: TerminalTextViewerProps): React.ReactNode {
	const view = useTerminalTextViewer(props);
	if (!view.snapshot.isOpen || view.snapshot.document === null) return null;
	const document = view.snapshot.document;
	const text = document.kind === 'text';

	return (
		<div className={styles['backdrop']} data-slot='terminal-text-viewer'>
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
					{text ? (
						<div className={styles['search']}>
							<Input
								aria-describedby={view.statusId}
								aria-label={view.searchLabel}
								className={styles['input']}
								maxLength={MAX_TERMINAL_TEXT_VIEWER_QUERY_LENGTH}
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
						{text ? (
							<Tooltip>
								<TooltipTrigger
									render={
										<Button
											aria-label={view.copyLabel}
											onClick={() => props.onCopy(document.source)}
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
					{text ? (
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
									{view.segments.map((segment, index) => {
										if (segment.kind === 'anchor') {
											return (
												<span
													aria-hidden='true'
													data-initial-scrollback-anchor='true'
													key={`anchor-${index}`}
												/>
											);
										}
										if (segment.kind === 'text') {
											return <React.Fragment key={`text-${index}`}>{segment.text}</React.Fragment>;
										}
										return (
											<mark
												className={styles['match']}
												data-active={segment.active}
												key={`match-${segment.matchIndex}-${index}`}
											>
												{segment.text}
											</mark>
										);
									})}
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
