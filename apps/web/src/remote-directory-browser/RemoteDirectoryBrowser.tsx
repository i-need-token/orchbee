import { remoteDirectoryBrowserMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Checkbox, Input, Kbd, ProductIcon, Spinner } from '@orchbee/ui/components';
import { type FocusEvent, type FormEvent, type KeyboardEvent, type ReactNode } from 'react';

import {
	REMOTE_DIRECTORY_EMPTY_MESSAGES,
	REMOTE_DIRECTORY_NAVIGATION_FAILURE_MESSAGES,
} from './constants/remote-directory-browser-messages';
import { useRemoteDirectoryBrowser } from './hooks/useRemoteDirectoryBrowser';
import type { RemoteDirectoryBrowserProps } from './types/remote-directory-browser';
import styles from './RemoteDirectoryBrowser.module.css';

export function RemoteDirectoryBrowser({
	busy,
	canNavigate,
	location,
	onCancel,
	onNavigate,
	onPathDraftChange,
	onSelect,
	pathDraft,
	sourceLabel,
}: RemoteDirectoryBrowserProps): ReactNode {
	const intl = useOrchbeeIntl();
	const viewController = useRemoteDirectoryBrowser({
		busy,
		canNavigate,
		canonicalPath: location.canonicalPath,
		directories: location.directories,
		navigationFailure: location.navigationFailure,
		onNavigate,
		onPathDraftChange,
		pathDraft,
	});
	return (
		<section
			aria-busy={busy}
			autoFocus={!viewController.showPathEditor}
			className={styles['root']}
			data-slot='remote-directory-browser'
			data-source-label={sourceLabel}
			onKeyDown={(event: KeyboardEvent<HTMLElement>) => {
				if (
					event.key !== 'Enter' ||
					event.target !== event.currentTarget ||
					viewController.showPathEditor ||
					busy ||
					event.repeat ||
					event.altKey ||
					event.ctrlKey ||
					event.metaKey ||
					event.shiftKey ||
					event.nativeEvent.isComposing
				) {
					return;
				}
				event.preventDefault();
				onSelect();
			}}
			ref={viewController.rootReference}
			tabIndex={-1}
		>
			{viewController.showPathEditor ? (
				<form
					id={viewController.pathFormId}
					className={styles['pathForm']}
					onSubmit={(event: FormEvent<HTMLFormElement>) => {
						event.preventDefault();
						viewController.submitPath();
					}}
				>
					<Input
						aria-describedby={
							viewController.pathFilterState === 'partial'
								? viewController.pathGuidanceId
								: undefined
						}
						aria-label={intl.formatMessage(remoteDirectoryBrowserMessages.path)}
						autoCapitalize='none'
						autoComplete='off'
						autoCorrect='off'
						autoFocus
						className={styles['pathInput']}
						disabled={busy}
						id={viewController.pathInputId}
						onChange={(event) => onPathDraftChange(event.currentTarget.value)}
						onBlur={(event: FocusEvent<HTMLInputElement>) => {
							const nextTarget = event.relatedTarget;
							if (
								nextTarget instanceof HTMLElement &&
								(nextTarget.id === viewController.pathSubmitId ||
									nextTarget.closest('[data-remote-directory-navigation-target="true"]') !== null)
							) {
								return;
							}
							viewController.resetPathEditor();
						}}
						onFocus={(event: FocusEvent<HTMLInputElement>) => {
							const end = event.currentTarget.value.length;
							event.currentTarget.setSelectionRange(end, end);
						}}
						onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
							if (event.key !== 'Escape') return;
							event.preventDefault();
							event.stopPropagation();
							viewController.resetPathEditor();
						}}
						required
						spellCheck={false}
						value={pathDraft}
					/>
				</form>
			) : (
				<div className={styles['locationBar']}>
					<span className={styles['source']} title={sourceLabel}>
						<ProductIcon name='folder-opened' />
						<span>{sourceLabel}</span>
					</span>
					<nav
						aria-label={intl.formatMessage(remoteDirectoryBrowserMessages.locationLabel)}
						className={styles['breadcrumbs']}
					>
						{location.crumbs.map((crumb, index) => {
							const isCurrent = index === location.crumbs.length - 1;
							return (
								<span key={crumb.path} className={styles['breadcrumb']}>
									<ProductIcon name='chevron-right' />
									{isCurrent ? (
										<span aria-current='page' className={styles['currentCrumb']} title={crumb.path}>
											{crumb.label}
										</span>
									) : (
										<button
											className={styles['crumbButton']}
											disabled={busy}
											onClick={() => viewController.openPath(crumb.path)}
											title={crumb.path}
											type='button'
										>
											{crumb.label}
										</button>
									)}
								</span>
							);
						})}
					</nav>
					<Button
						aria-label={intl.formatMessage(remoteDirectoryBrowserMessages.editPath)}
						className={styles['editPath']}
						disabled={busy}
						onClick={viewController.openPathEditor}
						size='icon-sm'
						type='button'
						variant='ghost'
					>
						<ProductIcon name='edit' />
					</Button>
				</div>
			)}

			{location.navigationFailure === null ? null : (
				<p className={styles['navigationFailure']} role='alert'>
					{intl.formatMessage(
						REMOTE_DIRECTORY_NAVIGATION_FAILURE_MESSAGES[location.navigationFailure],
					)}
				</p>
			)}
			{busy ? (
				<p aria-live='polite' className={styles['loadingStatus']} role='status'>
					{intl.formatMessage(remoteDirectoryBrowserMessages.loading)}
				</p>
			) : null}

			<div aria-busy={busy} className={styles['listFrame']}>
				<div className={styles['listScroller']}>
					{viewController.pathFilterState === 'partial' ? (
						<p className={styles['filterGuidance']} id={viewController.pathGuidanceId}>
							{intl.formatMessage(remoteDirectoryBrowserMessages.filterPartial)}
						</p>
					) : null}
					<div
						aria-label={intl.formatMessage(remoteDirectoryBrowserMessages.listLabel)}
						className={styles['directoryList']}
						role='list'
					>
						{viewController.visibleDirectories.map((entry) => (
							<div key={entry.path} role='listitem'>
								<button
									className={styles['directoryRow']}
									data-remote-directory-navigation-target='true'
									disabled={busy}
									onMouseDown={(event) => {
										if (viewController.showPathEditor) event.preventDefault();
									}}
									onClick={() => viewController.openPath(entry.path)}
									title={entry.path}
									type='button'
								>
									<ProductIcon name='folder' />
									<span>{entry.name}</span>
									<ProductIcon name='chevron-right' />
								</button>
							</div>
						))}
					</div>
					{viewController.emptyState === null ? null : (
						<p className={styles['emptyDirectory']}>
							{intl.formatMessage(REMOTE_DIRECTORY_EMPTY_MESSAGES[viewController.emptyState])}
						</p>
					)}
				</div>
				{busy ? (
					<span
						aria-hidden='true'
						className={styles['loadingIndicator']}
						data-slot='remote-directory-loading-indicator'
					>
						<Spinner aria-hidden='true' className={styles['loadingSpinner']} />
					</span>
				) : null}
			</div>

			{location.truncated ? (
				<p className={styles['truncated']}>
					{intl.formatMessage(remoteDirectoryBrowserMessages.truncated)}
				</p>
			) : null}

			<div className={styles['footer']}>
				<div className={styles['footerOptions']}>
					{viewController.hasHiddenDirectories ? (
						<label
							className={styles['hiddenDirectoriesToggle']}
							htmlFor={viewController.hiddenDirectoriesVisibilityId}
						>
							<Checkbox
								checked={viewController.hiddenDirectoriesVisible}
								disabled={busy}
								id={viewController.hiddenDirectoriesVisibilityId}
								onCheckedChange={viewController.setHiddenDirectoriesVisible}
							/>
							<span>{intl.formatMessage(remoteDirectoryBrowserMessages.showHidden)}</span>
						</label>
					) : null}
				</div>
				<div className={styles['actions']}>
					<Button
						className={styles['action']}
						disabled={busy}
						type='button'
						variant='outline'
						onClick={onCancel}
					>
						{intl.formatMessage(remoteDirectoryBrowserMessages.cancel)}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Esc
						</Kbd>
					</Button>
					{viewController.showPathEditor ? (
						<Button
							className={styles['action']}
							disabled={!viewController.canSubmitPath || busy}
							form={viewController.pathFormId}
							id={viewController.pathSubmitId}
							type='submit'
						>
							{intl.formatMessage(remoteDirectoryBrowserMessages.go)}
							<Kbd aria-hidden='true' className={styles['shortcut']}>
								Enter
							</Kbd>
						</Button>
					) : (
						<Button className={styles['action']} disabled={busy} type='button' onClick={onSelect}>
							{intl.formatMessage(remoteDirectoryBrowserMessages.useFolder)}
							<Kbd aria-hidden='true' className={styles['shortcut']}>
								Enter
							</Kbd>
						</Button>
					)}
				</div>
			</div>
		</section>
	);
}
