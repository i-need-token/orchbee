import { remoteDirectoryBrowserMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Field, FieldLabel, Input, ProductIcon } from '@orchbee/ui/components';
import type { FormEvent, KeyboardEvent, ReactNode } from 'react';

import { REMOTE_DIRECTORY_NAVIGATION_FAILURE_MESSAGES } from './constants/remote-directory-browser-messages';
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
		navigationFailure: location.navigationFailure,
		onNavigate,
		onPathDraftChange,
		pathDraft,
	});

	return (
		<section
			aria-busy={busy}
			className={styles['root']}
			data-slot='remote-directory-browser'
			data-source-label={sourceLabel}
		>
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

			{viewController.showPathEditor ? (
				<form
					className={styles['pathForm']}
					onSubmit={(event: FormEvent<HTMLFormElement>) => {
						event.preventDefault();
						viewController.submitPath();
					}}
				>
					<Field className={styles['pathField']}>
						<FieldLabel htmlFor='remote-directory-browser-path'>
							{intl.formatMessage(remoteDirectoryBrowserMessages.path)}
						</FieldLabel>
						<Input
							autoFocus
							disabled={busy}
							id='remote-directory-browser-path'
							onChange={(event) => onPathDraftChange(event.currentTarget.value)}
							onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
								if (event.key !== 'Escape') return;
								event.preventDefault();
								viewController.resetPathEditor();
							}}
							value={pathDraft}
						/>
					</Field>
					<Button disabled={!canNavigate || busy} type='submit' variant='outline'>
						{intl.formatMessage(remoteDirectoryBrowserMessages.go)}
					</Button>
				</form>
			) : null}

			{location.navigationFailure === null ? null : (
				<p className={styles['navigationFailure']} role='alert'>
					{intl.formatMessage(
						REMOTE_DIRECTORY_NAVIGATION_FAILURE_MESSAGES[location.navigationFailure],
					)}
				</p>
			)}

			{busy ? (
				<p aria-live='polite' className={styles['loading']} role='status'>
					{intl.formatMessage(remoteDirectoryBrowserMessages.loading)}
				</p>
			) : null}

			<div aria-busy={busy} className={styles['listFrame']}>
				<div
					aria-label={intl.formatMessage(remoteDirectoryBrowserMessages.listLabel)}
					className={styles['directoryList']}
					role='list'
				>
					{location.directories.map((entry) => (
						<div key={entry.path} role='listitem'>
							<button
								className={styles['directoryRow']}
								disabled={busy}
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
				{location.directories.length === 0 ? (
					<p className={styles['emptyDirectory']}>
						{intl.formatMessage(remoteDirectoryBrowserMessages.empty)}
					</p>
				) : null}
			</div>

			{location.truncated ? (
				<p className={styles['truncated']}>
					{intl.formatMessage(remoteDirectoryBrowserMessages.truncated)}
				</p>
			) : null}

			<div className={styles['actions']}>
				<Button disabled={busy} type='button' variant='outline' onClick={onCancel}>
					{intl.formatMessage(remoteDirectoryBrowserMessages.cancel)}
				</Button>
				<Button disabled={busy} type='button' onClick={onSelect}>
					{intl.formatMessage(remoteDirectoryBrowserMessages.useFolder)}
				</Button>
			</div>
		</section>
	);
}
