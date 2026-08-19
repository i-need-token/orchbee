import {
	Badge,
	Button,
	Checkbox,
	Input,
	NativeSelect,
	NativeSelectOption,
	ProductIcon,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type { DesktopTerminalLaunchProfileControl } from '../../../terminal-launch-profile-control';
import { useTerminalLaunchProfileSettings } from './hooks/useTerminalLaunchProfileSettings';
import styles from './TerminalLaunchProfileSettings.module.css';

interface TerminalLaunchProfileSettingsProps {
	readonly client: DesktopTerminalLaunchProfileControl;
}

export function TerminalLaunchProfileSettings({
	client,
}: TerminalLaunchProfileSettingsProps): React.ReactNode {
	const view = useTerminalLaunchProfileSettings(client);

	return (
		<div className={styles['root']} data-slot='terminal-launch-profile-settings'>
			<p className={styles['notice']}>{view.notice}</p>
			{view.warning === null ? null : (
				<div className={styles['warning']} role='alert'>
					{view.warning}
				</div>
			)}
			<section aria-label={view.sectionLabel} className={styles['workspace']} id='profiles.catalog'>
				<div className={styles['catalog']}>
					<h2 className={styles['sectionTitle']}>{view.sectionLabel}</h2>
					<div className={styles['profileList']}>
						{view.profiles.map((profile) => (
							<button
								aria-pressed={profile.selected}
								className={styles['profile']}
								data-selected={profile.selected}
								data-slot='terminal-launch-profile-option'
								key={profile.id}
								type='button'
								onClick={profile.onSelect}
							>
								<span className={styles['profileName']}>{profile.label}</span>
								{profile.defaultLabel === null ? null : (
									<Badge variant='secondary'>{profile.defaultLabel}</Badge>
								)}
							</button>
						))}
					</div>
					<form className={styles['addProfile']} onSubmit={view.addProfile.onSubmit}>
						<Input
							aria-label={view.addProfile.inputLabel}
							disabled={view.addProfile.inputDisabled}
							placeholder={view.addProfile.inputPlaceholder}
							value={view.addProfile.inputValue}
							onChange={view.addProfile.onInputChange}
						/>
						<Button
							data-slot='terminal-launch-profile-add'
							disabled={view.addProfile.buttonDisabled}
							type='submit'
							variant='secondary'
						>
							<ProductIcon name='add' />
							{view.addProfile.buttonLabel}
						</Button>
					</form>
				</div>

				<div className={styles['editor']}>
					<div className={styles['field']}>
						<label className={styles['fieldLabel']} htmlFor='terminal-profile-name'>
							{view.nameEditor.label}
						</label>
						<Input
							disabled={view.nameEditor.disabled}
							id='terminal-profile-name'
							placeholder={view.nameEditor.placeholder}
							value={view.nameEditor.value}
							onChange={view.nameEditor.onChange}
						/>
					</div>
					<div className={styles['field']} id='profiles.command'>
						<label className={styles['fieldLabel']} htmlFor='terminal-profile-command'>
							{view.commandEditor.label}
						</label>
						<NativeSelect
							className={styles['wideControl']}
							disabled={view.commandEditor.disabled}
							id='terminal-profile-command'
							value={view.commandEditor.kind}
							onChange={view.commandEditor.onKindChange}
						>
							{view.commandEditor.options.map((option) => (
								<NativeSelectOption key={option.value} value={option.value}>
									{option.label}
								</NativeSelectOption>
							))}
						</NativeSelect>
					</div>

					{view.commandEditor.kind === 'system-shell' ? (
						<div className={styles['toggleField']}>
							<Checkbox
								aria-describedby='terminal-profile-login-description'
								checked={view.commandEditor.login}
								disabled={view.commandEditor.loginDisabled}
								id='terminal-profile-login'
								onCheckedChange={view.commandEditor.onLoginChange}
							/>
							<div>
								<label className={styles['fieldLabel']} htmlFor='terminal-profile-login'>
									{view.commandEditor.loginLabel}
								</label>
								<p className={styles['fieldDescription']} id='terminal-profile-login-description'>
									{view.commandEditor.description}
								</p>
							</div>
						</div>
					) : (
						<>
							<div className={styles['field']}>
								<label className={styles['fieldLabel']} htmlFor='terminal-profile-executable'>
									{view.commandEditor.executableLabel}
								</label>
								<Input
									disabled={view.commandEditor.disabled}
									id='terminal-profile-executable'
									placeholder={view.commandEditor.executablePlaceholder}
									value={view.commandEditor.executableValue}
									onChange={view.commandEditor.onExecutableChange}
								/>
								<p className={styles['fieldDescription']}>
									{view.commandEditor.executableDescription}
								</p>
							</div>
							<div className={styles['field']} id='profiles.arguments'>
								<div className={styles['argumentHeader']}>
									<span className={styles['fieldLabel']}>{view.commandEditor.argumentsLabel}</span>
									<Button
										data-slot='terminal-launch-profile-add-argument'
										disabled={view.commandEditor.addArgumentDisabled}
										size='sm'
										type='button'
										variant='outline'
										onClick={view.commandEditor.onAddArgument}
									>
										<ProductIcon name='add' />
										{view.commandEditor.addArgumentLabel}
									</Button>
								</div>
								<p className={styles['fieldDescription']}>
									{view.commandEditor.argumentsDescription}
								</p>
								<div className={styles['arguments']}>
									{view.commandEditor.arguments.map((argument) => (
										<div className={styles['argument']} key={argument.key}>
											<Input
												aria-label={argument.inputLabel}
												disabled={argument.disabled}
												value={argument.value}
												onChange={argument.onChange}
											/>
											<Button
												aria-label={argument.removeLabel}
												disabled={argument.disabled}
												size='icon-sm'
												type='button'
												variant='ghost'
												onClick={argument.onRemove}
											>
												<ProductIcon name='trash' />
											</Button>
										</div>
									))}
								</div>
							</div>
						</>
					)}

					<div className={styles['actions']}>
						<Button
							data-slot='terminal-launch-profile-save'
							disabled={view.saveAction.disabled}
							type='button'
							onClick={view.saveAction.onClick}
						>
							{view.saveAction.label}
						</Button>
						<Button
							data-slot='terminal-launch-profile-make-default'
							disabled={view.makeDefaultAction.disabled}
							type='button'
							variant='secondary'
							onClick={view.makeDefaultAction.onClick}
						>
							{view.makeDefaultAction.label}
						</Button>
						{view.deleteAction.kind === 'confirm' ? (
							<>
								<Button
									disabled={view.deleteAction.disabled}
									type='button'
									variant='destructive'
									onClick={view.deleteAction.onConfirm}
								>
									{view.deleteAction.confirmLabel}
								</Button>
								<Button
									disabled={view.deleteAction.disabled}
									type='button'
									variant='ghost'
									onClick={view.deleteAction.onCancel}
								>
									{view.deleteAction.cancelLabel}
								</Button>
							</>
						) : (
							<Button
								disabled={view.deleteAction.disabled}
								type='button'
								variant='ghost'
								onClick={view.deleteAction.onRequest}
							>
								<ProductIcon name='trash' />
								{view.deleteAction.label}
							</Button>
						)}
					</div>
				</div>
			</section>

			<div className={styles['reset']}>
				{view.resetAction.kind === 'confirm' ? (
					<>
						<Button
							disabled={view.resetAction.disabled}
							type='button'
							variant='destructive'
							onClick={view.resetAction.onConfirm}
						>
							{view.resetAction.confirmLabel}
						</Button>
						<Button
							disabled={view.resetAction.disabled}
							type='button'
							variant='ghost'
							onClick={view.resetAction.onCancel}
						>
							{view.resetAction.cancelLabel}
						</Button>
					</>
				) : (
					<Button
						data-slot='terminal-launch-profile-reset'
						disabled={view.resetAction.disabled}
						type='button'
						variant='outline'
						onClick={view.resetAction.onRequest}
					>
						{view.resetAction.label}
					</Button>
				)}
			</div>
		</div>
	);
}
