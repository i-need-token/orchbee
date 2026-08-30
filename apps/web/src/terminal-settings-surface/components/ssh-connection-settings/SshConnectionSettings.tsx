import { sshConnectionManagementMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Button,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
	ProductIcon,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type { DesktopSshConnectionManagementControl } from '../../../ssh-connection-management';
import { useSshConnectionSettings } from './hooks/useSshConnectionSettings';
import styles from './SshConnectionSettings.module.css';

export interface SshConnectionSettingsProps {
	readonly control: DesktopSshConnectionManagementControl;
}

export function SshConnectionSettings({ control }: SshConnectionSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const view = useSshConnectionSettings(control);
	return (
		<div className={styles['root']} data-slot='ssh-connection-settings'>
			<aside className={styles['privacy']}>
				<span className={styles['privacyIcon']}>
					<ProductIcon name='terminal' />
				</span>
				<div>
					<h2 className={styles['privacyTitle']}>
						{intl.formatMessage(sshConnectionManagementMessages.privacyTitle)}
					</h2>
					<p className={styles['privacyDescription']}>
						{intl.formatMessage(sshConnectionManagementMessages.privacyDescription)}
					</p>
				</div>
			</aside>

			{view.registryWarning === null ? null : (
				<div className={styles['warning']} role='alert'>
					{view.registryWarning}
				</div>
			)}
			{view.failure === null ? null : (
				<div className={styles['failure']} role='alert'>
					<span>{view.failure}</span>
					<Button
						aria-label={intl.formatMessage(sshConnectionManagementMessages.actionDismissFailure)}
						size='icon-sm'
						type='button'
						variant='ghost'
						onClick={view.onDismissFailure}
					>
						<ProductIcon name='close' />
					</Button>
				</div>
			)}

			<section className={styles['section']} id='ssh-connections.add'>
				<div className={styles['sectionHeader']}>
					<div>
						<h2 className={styles['sectionTitle']}>
							{intl.formatMessage(sshConnectionManagementMessages.actionAdd)}
						</h2>
					</div>
				</div>
				<form className={styles['addForm']} onSubmit={view.onSubmit}>
					<Field>
						<FieldLabel className={styles['fieldLabel']} htmlFor='ssh-connection-destination'>
							{intl.formatMessage(sshConnectionManagementMessages.fieldDestination)}
						</FieldLabel>
						<div className={styles['addControls']}>
							<Input
								aria-describedby='ssh-connection-destination-description'
								autoCapitalize='none'
								autoComplete='off'
								autoCorrect='off'
								className={styles['destinationInput']}
								id='ssh-connection-destination'
								placeholder={intl.formatMessage(
									sshConnectionManagementMessages.fieldDestinationPlaceholder,
								)}
								required
								spellCheck={false}
								value={view.destination}
								onChange={view.onDestinationChange}
							/>
							<Button
								className={styles['formAction']}
								disabled={view.addActionDisabled}
								type='submit'
							>
								<ProductIcon name='add' />
								{view.addActionLabel}
							</Button>
						</div>
						<FieldDescription
							className={styles['fieldDescription']}
							id='ssh-connection-destination-description'
						>
							{intl.formatMessage(sshConnectionManagementMessages.fieldDestinationDescription)}
						</FieldDescription>
					</Field>
				</form>
			</section>

			<section className={styles['section']} id='ssh-connections.managed'>
				<div className={styles['sectionHeader']}>
					<div>
						<h2 className={styles['sectionTitle']}>
							{intl.formatMessage(sshConnectionManagementMessages.managedSection)}
						</h2>
						<p className={styles['sectionDescription']}>
							{intl.formatMessage(sshConnectionManagementMessages.managedDescription)}
						</p>
					</div>
				</div>
				<div className={styles['list']}>
					{view.managed.length === 0 ? (
						<div className={styles['empty']}>
							<span className={styles['emptyIcon']}>
								<ProductIcon name='terminal' />
							</span>
							<div>
								<strong className={styles['emptyTitle']}>
									{intl.formatMessage(sshConnectionManagementMessages.managedEmptyTitle)}
								</strong>
								<p className={styles['emptyDescription']}>
									{intl.formatMessage(sshConnectionManagementMessages.managedEmptyDescription)}
								</p>
							</div>
						</div>
					) : (
						view.managed.map((connection) => (
							<article
								className={styles['connection']}
								data-connection-state={connection.statusTone}
								key={connection.environmentId}
							>
								<span className={styles['connectionIcon']}>
									<ProductIcon name='terminal' />
								</span>
								<div className={styles['connectionCopy']}>
									<div className={styles['connectionHeading']}>
										<h3 className={styles['connectionName']}>{connection.name}</h3>
										<span className={styles['status']} data-tone={connection.statusTone}>
											<span aria-hidden='true' className={styles['statusDot']} />
											{connection.status}
										</span>
									</div>
									{connection.destination === connection.name ? null : (
										<code className={styles['destination']}>{connection.destination}</code>
									)}
									<p className={styles['metadata']}>
										<span>{connection.source}</span>
										<span aria-hidden='true'>·</span>
										<span>{connection.projectUsage}</span>
									</p>
								</div>
								<Button
									disabled={connection.actionDisabled}
									type='button'
									variant={connection.statusTone === 'positive' ? 'outline' : 'default'}
									onClick={connection.onAction}
								>
									{connection.actionLabel}
								</Button>
							</article>
						))
					)}
				</div>
			</section>

			<section className={styles['section']} id='ssh-connections.discovery'>
				<div className={styles['sectionHeader']}>
					<div>
						<h2 className={styles['sectionTitle']}>
							{intl.formatMessage(sshConnectionManagementMessages.suggestionsSection)}
						</h2>
						<p className={styles['sectionDescription']}>
							{intl.formatMessage(sshConnectionManagementMessages.suggestionsDescription)}
						</p>
					</div>
					<Button
						disabled={view.refreshDisabled}
						type='button'
						variant='outline'
						onClick={view.onRefresh}
					>
						{intl.formatMessage(sshConnectionManagementMessages.actionRefresh)}
					</Button>
				</div>
				{view.discoveryWarning === null ? null : (
					<p className={styles['discoveryWarning']} role='status'>
						{view.discoveryWarning}
					</p>
				)}
				<div className={styles['list']}>
					{view.suggestions.length === 0 ? (
						<div className={styles['empty']}>
							<span className={styles['emptyIcon']}>
								<ProductIcon name='terminal' />
							</span>
							<div>
								<strong className={styles['emptyTitle']}>
									{intl.formatMessage(sshConnectionManagementMessages.suggestionsEmptyTitle)}
								</strong>
								<p className={styles['emptyDescription']}>
									{intl.formatMessage(sshConnectionManagementMessages.suggestionsEmptyDescription)}
								</p>
							</div>
						</div>
					) : (
						view.suggestions.map((suggestion) => (
							<div className={styles['suggestion']} key={suggestion.destination}>
								<span className={styles['connectionIcon']}>
									<ProductIcon name='terminal' />
								</span>
								<div className={styles['connectionCopy']}>
									<strong className={styles['connectionName']}>{suggestion.destination}</strong>
									<p className={styles['metadata']}>{suggestion.source}</p>
								</div>
								<Button
									disabled={suggestion.actionDisabled}
									type='button'
									variant='outline'
									onClick={suggestion.onAdd}
								>
									<ProductIcon name='add' />
									{suggestion.actionLabel}
								</Button>
							</div>
						))
					)}
				</div>
			</section>
		</div>
	);
}
