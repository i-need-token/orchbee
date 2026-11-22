import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Button,
	Checkbox,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
	Input,
	ProductIcon,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { TerminalLaunchProfileControl } from '../../../terminal-launch-profile-control';
import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsChildPage } from '../settings-child-page/SettingsChildPage';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SETTINGS_LIST_FILTER_THRESHOLD } from '../../constants/settings-list-filter';
import { SettingsListFilter } from '../settings-list-filter/SettingsListFilter';
import { SettingsListFilterEmpty } from '../settings-list-filter-empty/SettingsListFilterEmpty';
import { SettingsRow } from '../settings-row/SettingsRow';
import { SettingsSelect } from '../settings-select/SettingsSelect';
import { useTerminalLaunchProfileSettings } from './hooks/useTerminalLaunchProfileSettings';
import styles from './TerminalLaunchProfileSettings.module.css';

interface TerminalLaunchProfileSettingsProps {
	readonly client: TerminalLaunchProfileControl;
	readonly query: string;
}

/**
 * Launch profiles as one list card: each profile on its own line with its default state and an
 * Edit action that opens its editor on a child page; Done or Back saves and returns to the list.
 */
export function TerminalLaunchProfileSettings({
	client,
	query,
}: TerminalLaunchProfileSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const view = useTerminalLaunchProfileSettings(client);
	const [editingId, setEditingId] = React.useState<string | null>(null);
	const [adding, setAdding] = React.useState(false);
	const [filter, setFilter] = React.useState('');
	const searching = query.trim() !== '';
	const filterVisible = !searching && view.profiles.length > SETTINGS_LIST_FILTER_THRESHOLD;
	const profiles = filterVisible
		? view.profiles.filter((profile) =>
				matchesSettingsQuery(filter, [profile.label, profile.detail ?? ''], intl.locale),
			)
		: view.profiles;
	// The editor follows the selected profile, so a deleted or reset profile closes its page.
	const edited = searching
		? undefined
		: view.profiles.find((profile) => profile.id === editingId && profile.selected);
	const done = async (): Promise<void> => {
		if (await view.saveAction.onClick()) setEditingId(null);
	};
	const back = (): void => {
		if (view.saveAction.disabled) setEditingId(null);
		else void done();
	};

	return (
		<SettingsGroup
			action={
				view.resetAction.kind === 'confirm' ? (
					<span className={styles['resetConfirm']}>
						<Button
							disabled={view.resetAction.disabled}
							size='xs'
							type='button'
							variant='ghost'
							onClick={view.resetAction.onCancel}
						>
							{view.resetAction.cancelLabel}
						</Button>
						<Button
							data-slot='terminal-launch-profile-reset'
							disabled={view.resetAction.disabled}
							size='xs'
							type='button'
							variant='destructive'
							onClick={view.resetAction.onConfirm}
						>
							{view.resetAction.confirmLabel}
						</Button>
					</span>
				) : undefined
			}
			description={intl.formatMessage(terminalSettingsMessages.pageGroupLaunchProfilesDescription)}
			effect='new-terminals'
			label={intl.formatMessage(terminalSettingsMessages.pageGroupLaunchProfiles)}
			modified={view.modified}
			onRestore={view.resetAction.kind === 'request' ? view.resetAction.onRequest : undefined}
		>
			{searching ? null : <SettingFailure message={view.warning} />}
			{filterVisible ? (
				<SettingsListFilter
					placeholder={intl.formatMessage(terminalSettingsMessages.pageFilterProfiles)}
					shown={profiles.length}
					total={view.profiles.length}
					value={filter}
					onChange={setFilter}
				/>
			) : null}
			{filterVisible && profiles.length === 0 ? <SettingsListFilterEmpty /> : null}
			{profiles.map((profile) => (
				<SettingsRow
					key={profile.id}
					detail={profile.detail}
					label={profile.label}
					query={query}
					settingId={`profiles.${profile.id}`}
				>
					{profile.isDefault ? (
						<span className={styles['defaultLabel']}>{profile.defaultLabel}</span>
					) : (
						<Button
							data-slot='terminal-launch-profile-make-default'
							disabled={view.makeDefaultAction.disabled && profile.selected}
							size='sm'
							type='button'
							variant='link'
							onClick={profile.onMakeDefault}
						>
							{intl.formatMessage(terminalSettingsMessages.pageProfilesUseAsDefault)}
						</Button>
					)}
					<Button
						aria-label={`${intl.formatMessage(terminalSettingsMessages.pageProfilesEdit)}: ${profile.label}`}
						data-slot='terminal-launch-profile-option'
						size='sm'
						type='button'
						variant='outline'
						onClick={() => {
							profile.onSelect();
							setEditingId(profile.id);
						}}
					>
						{intl.formatMessage(terminalSettingsMessages.pageProfilesEdit)}
					</Button>
				</SettingsRow>
			))}
			{searching ? null : (
				<AddProfileRow
					adding={adding}
					view={view}
					onAddingChange={setAdding}
					onSubmitted={() => setEditingId(null)}
				/>
			)}
			{edited === undefined ? null : (
				<SettingsChildPage title={edited.label} onBack={back}>
					<ProfileEditor view={view} onDone={done} />
				</SettingsChildPage>
			)}
		</SettingsGroup>
	);
}

interface AddProfileRowProps {
	readonly adding: boolean;
	readonly onAddingChange: (adding: boolean) => void;
	readonly onSubmitted: () => void;
	readonly view: ReturnType<typeof useTerminalLaunchProfileSettings>;
}

/**
 * The card's last line: an Add profile action that lists the shells installed on the Server's
 * machine, then Custom…, which opens a name box in its place; with no shell found it opens the name
 * box at once.
 */
function AddProfileRow({
	adding,
	onAddingChange,
	onSubmitted,
	view,
}: AddProfileRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	if (!adding) {
		const label = (
			<>
				<ProductIcon name='add' />
				{intl.formatMessage(terminalSettingsMessages.pageProfilesAdd)}
			</>
		);
		if (view.addProfile.shells.length === 0) {
			return (
				<button
					className={styles['addRow']}
					data-settings-row
					disabled={view.addProfile.buttonDisabled}
					type='button'
					onClick={() => onAddingChange(true)}
				>
					{label}
				</button>
			);
		}
		return (
			<DropdownMenu>
				<DropdownMenuTrigger
					className={styles['addRow']}
					data-settings-row
					disabled={view.addProfile.buttonDisabled}
				>
					{label}
				</DropdownMenuTrigger>
				<DropdownMenuContent>
					{view.addProfile.shells.map((shell) => (
						<DropdownMenuItem
							key={shell.key}
							onClick={() => {
								shell.onSelect();
								onSubmitted();
							}}
						>
							<ProductIcon name='terminal' size='menu-leading' />
							{shell.label}
						</DropdownMenuItem>
					))}
					<DropdownMenuSeparator />
					<DropdownMenuItem onClick={() => onAddingChange(true)}>
						<ProductIcon name='edit' size='menu-leading' />
						{view.addProfile.customLabel}
					</DropdownMenuItem>
				</DropdownMenuContent>
			</DropdownMenu>
		);
	}
	return (
		<form
			className={styles['addProfile']}
			onSubmit={(event) => {
				view.addProfile.onSubmit(event);
				onAddingChange(false);
				onSubmitted();
			}}
		>
			<Input
				ref={focusOnMount}
				aria-label={view.addProfile.inputLabel}
				autoComplete='off'
				className={styles['addInput']}
				disabled={view.addProfile.inputDisabled}
				placeholder={view.addProfile.inputPlaceholder}
				required
				value={view.addProfile.inputValue}
				onChange={view.addProfile.onInputChange}
				onKeyDown={(event) => {
					if (event.key !== 'Escape') return;
					event.preventDefault();
					onAddingChange(false);
				}}
			/>
			<Button
				data-slot='terminal-launch-profile-add'
				disabled={view.addProfile.buttonDisabled}
				size='sm'
				type='submit'
			>
				{view.addProfile.buttonLabel}
			</Button>
		</form>
	);
}

/** The add-profile name box opens on request, so its name is the next thing typed. */
function focusOnMount(element: HTMLInputElement | null): void {
	element?.focus();
}

function ProfileEditor({
	onDone,
	view,
}: {
	readonly onDone: () => Promise<void>;
	readonly view: ReturnType<typeof useTerminalLaunchProfileSettings>;
}): React.ReactNode {
	const intl = useOrchbeeIntl();
	const { commandEditor, deleteAction, nameEditor } = view;
	return (
		<form
			className={styles['editor']}
			data-slot='terminal-launch-profile-editor'
			onSubmit={(event) => {
				event.preventDefault();
				if (!view.saveAction.disabled) void onDone();
			}}
		>
			<label className={styles['field']} htmlFor='terminal-profile-name'>
				<span className={styles['fieldLabel']}>{nameEditor.label}</span>
				<Input
					autoComplete='off'
					disabled={nameEditor.disabled}
					id='terminal-profile-name'
					placeholder={nameEditor.placeholder}
					required={nameEditor.required}
					value={nameEditor.value}
					onChange={nameEditor.onChange}
				/>
			</label>
			<label className={styles['field']} htmlFor='terminal-profile-command'>
				<span className={styles['fieldLabel']}>{commandEditor.label}</span>
				<SettingsSelect
					disabled={commandEditor.disabled}
					id='terminal-profile-command'
					options={commandEditor.options}
					value={commandEditor.kind}
					onValueChange={commandEditor.onKindChange}
				/>
			</label>
			{commandEditor.kind === 'system-shell' ? (
				<div className={styles['toggleField']}>
					<Checkbox
						aria-describedby='terminal-profile-login-description'
						checked={commandEditor.login}
						disabled={commandEditor.loginDisabled}
						id='terminal-profile-login'
						onCheckedChange={commandEditor.onLoginChange}
					/>
					<div>
						<label className={styles['fieldLabel']} htmlFor='terminal-profile-login'>
							{commandEditor.loginLabel}
						</label>
						<p className={styles['fieldDescription']} id='terminal-profile-login-description'>
							{commandEditor.description}
						</p>
					</div>
				</div>
			) : (
				<>
					<label className={styles['field']} htmlFor='terminal-profile-executable'>
						<span className={styles['fieldLabel']}>{commandEditor.executableLabel}</span>
						<Input
							aria-describedby='terminal-profile-executable-description'
							autoCapitalize='none'
							autoComplete='off'
							autoCorrect='off'
							className={styles['codeInput']}
							disabled={commandEditor.disabled}
							id='terminal-profile-executable'
							placeholder={commandEditor.executablePlaceholder}
							required
							spellCheck={false}
							value={commandEditor.executableValue}
							onChange={commandEditor.onExecutableChange}
						/>
						<span
							className={styles['fieldDescription']}
							id='terminal-profile-executable-description'
						>
							{commandEditor.executableDescription}
						</span>
					</label>
					<div
						aria-describedby='terminal-profile-arguments-description'
						aria-labelledby='terminal-profile-arguments-label'
						className={styles['field']}
						role='group'
					>
						<div className={styles['argumentHeader']}>
							<span className={styles['fieldLabel']} id='terminal-profile-arguments-label'>
								{commandEditor.argumentsLabel}
							</span>
							<Button
								data-slot='terminal-launch-profile-add-argument'
								disabled={commandEditor.addArgumentDisabled}
								size='sm'
								type='button'
								variant='ghost'
								onClick={commandEditor.onAddArgument}
							>
								<ProductIcon name='add' />
								{commandEditor.addArgumentLabel}
							</Button>
						</div>
						<span
							className={styles['fieldDescription']}
							id='terminal-profile-arguments-description'
						>
							{commandEditor.argumentsDescription}
						</span>
						{commandEditor.arguments.map((argument) => (
							<div className={styles['argument']} key={argument.key}>
								<Input
									aria-label={argument.inputLabel}
									autoCapitalize='none'
									autoComplete='off'
									autoCorrect='off'
									className={styles['codeInput']}
									disabled={argument.disabled}
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
				</>
			)}
			<div className={styles['actions']}>
				<div className={styles['deleteActions']}>
					{deleteAction.kind === 'confirm' ? (
						<>
							<Button
								disabled={deleteAction.disabled}
								size='sm'
								type='button'
								variant='destructive'
								onClick={deleteAction.onConfirm}
							>
								{deleteAction.confirmLabel}
							</Button>
							<Button
								disabled={deleteAction.disabled}
								size='sm'
								type='button'
								variant='ghost'
								onClick={deleteAction.onCancel}
							>
								{deleteAction.cancelLabel}
							</Button>
						</>
					) : (
						<Button
							className={styles['deleteRequest']}
							disabled={deleteAction.disabled}
							size='sm'
							type='button'
							variant='ghost'
							onClick={deleteAction.onRequest}
						>
							<ProductIcon name='trash' />
							{deleteAction.label}
						</Button>
					)}
				</div>
				<Button
					data-slot='terminal-launch-profile-save'
					disabled={view.saveAction.disabled}
					size='sm'
					type='submit'
				>
					{intl.formatMessage(terminalSettingsMessages.pageProfilesDone)}
				</Button>
			</div>
		</form>
	);
}
