import {
	Badge,
	Button,
	Checkbox,
	Field,
	FieldDescription,
	FieldLabel,
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
						<Field>
							<FieldLabel className={styles['fieldLabel']} htmlFor='terminal-profile-add-name'>
								{view.addProfile.inputLabel}
							</FieldLabel>
							<Input
								autoComplete='off'
								disabled={view.addProfile.inputDisabled}
								id='terminal-profile-add-name'
								placeholder={view.addProfile.inputPlaceholder}
								required
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
						</Field>
					</form>
				</div>

				<form
					className={styles['editor']}
					onSubmit={(event: React.FormEvent<HTMLFormElement>) => {
						event.preventDefault();
						if (!view.saveAction.disabled) view.saveAction.onClick();
					}}
				>
					<Field className={styles['field']}>
						<FieldLabel className={styles['fieldLabel']} htmlFor='terminal-profile-name'>
							{view.nameEditor.label}
						</FieldLabel>
						<Input
							autoComplete='off'
							disabled={view.nameEditor.disabled}
							id='terminal-profile-name'
							placeholder={view.nameEditor.placeholder}
							required={view.nameEditor.required}
							value={view.nameEditor.value}
							onChange={view.nameEditor.onChange}
						/>
					</Field>
					<Field className={styles['field']} id='profiles.command'>
						<FieldLabel className={styles['fieldLabel']} htmlFor='terminal-profile-command'>
							{view.commandEditor.label}
						</FieldLabel>
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
					</Field>

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
							<Field className={styles['field']}>
								<FieldLabel className={styles['fieldLabel']} htmlFor='terminal-profile-executable'>
									{view.commandEditor.executableLabel}
								</FieldLabel>
								<Input
									aria-describedby='terminal-profile-executable-description'
									autoCapitalize='none'
									autoComplete='off'
									autoCorrect='off'
									className={styles['codeInput']}
									disabled={view.commandEditor.disabled}
									id='terminal-profile-executable'
									placeholder={view.commandEditor.executablePlaceholder}
									required
									spellCheck={false}
									value={view.commandEditor.executableValue}
									onChange={view.commandEditor.onExecutableChange}
								/>
								<FieldDescription
									className={styles['fieldDescription']}
									id='terminal-profile-executable-description'
								>
									{view.commandEditor.executableDescription}
								</FieldDescription>
							</Field>
							<Field className={styles['field']} id='profiles.arguments'>
								<div className={styles['argumentHeader']}>
									<span className={styles['fieldLabel']} id='terminal-profile-arguments-label'>
										{view.commandEditor.argumentsLabel}
									</span>
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
								<FieldDescription
									className={styles['fieldDescription']}
									id='terminal-profile-arguments-description'
								>
									{view.commandEditor.argumentsDescription}
								</FieldDescription>
								<div
									aria-describedby='terminal-profile-arguments-description'
									aria-labelledby='terminal-profile-arguments-label'
									className={styles['arguments']}
									role='group'
								>
									{view.commandEditor.arguments.map((argument) => (
										<div className={styles['argument']} key={argument.key}>
											<label
												className={styles['argumentLabel']}
												htmlFor={`terminal-profile-argument-${argument.key}`}
											>
												{argument.inputLabel}
											</label>
											<Input
												autoCapitalize='none'
												autoComplete='off'
												autoCorrect='off'
												className={styles['codeInput']}
												disabled={argument.disabled}
												id={`terminal-profile-argument-${argument.key}`}
												spellCheck={false}
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
							</Field>
						</>
					)}

					<div className={styles['actions']}>
						<Button
							data-slot='terminal-launch-profile-save'
							disabled={view.saveAction.disabled}
							type='submit'
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
				</form>
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
