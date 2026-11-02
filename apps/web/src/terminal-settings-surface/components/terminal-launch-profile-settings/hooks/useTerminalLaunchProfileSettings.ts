import {
	BUILT_IN_USER_LAUNCH_PROFILE_ID,
	USER_LAUNCH_PROFILE_LIMITS,
	UserLaunchProfileNameSchema,
	UserLaunchProfileSchema,
	type UserLaunchProfile,
} from '@orchbee/workbench-client-protocol';
import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { TerminalLaunchProfileControl } from '../../../../terminal-launch-profile-control';

interface ProfileDraft {
	readonly arguments: readonly string[];
	readonly executable: string;
	readonly kind: UserLaunchProfile['command']['kind'];
	readonly login: boolean;
	readonly name: string;
}

interface ProfileListItemView {
	readonly defaultLabel: string | null;
	readonly id: string;
	readonly label: string;
	readonly onSelect: () => void;
	readonly selected: boolean;
}

interface AddProfileView {
	readonly buttonDisabled: boolean;
	readonly buttonLabel: string;
	readonly inputDisabled: boolean;
	readonly inputLabel: string;
	readonly inputPlaceholder: string;
	readonly inputValue: string;
	readonly onInputChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onSubmit: React.FormEventHandler<HTMLFormElement>;
}

interface NameEditorView {
	readonly disabled: boolean;
	readonly label: string;
	readonly onChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly placeholder: string;
	readonly required: boolean;
	readonly value: string;
}

interface CommandOptionView {
	readonly label: string;
	readonly value: UserLaunchProfile['command']['kind'];
}

interface CommandEditorCommonView {
	readonly disabled: boolean;
	readonly label: string;
	readonly onKindChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly options: readonly CommandOptionView[];
}

interface SystemShellEditorView extends CommandEditorCommonView {
	readonly description: string;
	readonly kind: 'system-shell';
	readonly login: boolean;
	readonly loginDisabled: boolean;
	readonly loginLabel: string;
	readonly onLoginChange: (checked: boolean) => void;
}

interface ArgumentEditorView {
	readonly disabled: boolean;
	readonly inputLabel: string;
	readonly key: number;
	readonly onChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onRemove: () => void;
	readonly removeLabel: string;
	readonly value: string;
}

interface CustomExecutableEditorView extends CommandEditorCommonView {
	readonly addArgumentDisabled: boolean;
	readonly addArgumentLabel: string;
	readonly arguments: readonly ArgumentEditorView[];
	readonly argumentsDescription: string;
	readonly argumentsLabel: string;
	readonly executableDescription: string;
	readonly executableLabel: string;
	readonly executablePlaceholder: string;
	readonly executableValue: string;
	readonly kind: 'custom-executable';
	readonly onAddArgument: () => void;
	readonly onExecutableChange: React.ChangeEventHandler<HTMLInputElement>;
}

type CommandEditorView = CustomExecutableEditorView | SystemShellEditorView;

interface ButtonActionView {
	readonly disabled: boolean;
	readonly label: string;
	readonly onClick: () => void;
}

type ConfirmableActionView =
	| {
			readonly cancelLabel: string;
			readonly confirmLabel: string;
			readonly disabled: boolean;
			readonly kind: 'confirm';
			readonly onCancel: () => void;
			readonly onConfirm: () => void;
	  }
	| {
			readonly disabled: boolean;
			readonly kind: 'request';
			readonly label: string;
			readonly onRequest: () => void;
	  };

interface TerminalLaunchProfileSettingsView {
	readonly addProfile: AddProfileView;
	readonly commandEditor: CommandEditorView;
	readonly deleteAction: ConfirmableActionView;
	readonly makeDefaultAction: ButtonActionView;
	readonly nameEditor: NameEditorView;
	readonly notice: string;
	readonly profiles: readonly ProfileListItemView[];
	readonly resetAction: ConfirmableActionView;
	readonly saveAction: ButtonActionView;
	readonly sectionLabel: string;
	readonly warning: string | null;
}

export function useTerminalLaunchProfileSettings(
	client: TerminalLaunchProfileControl,
): TerminalLaunchProfileSettingsView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		client.subscribe,
		client.getSnapshot,
		client.getSnapshot,
	);
	const [selectedProfileId, setSelectedProfileId] = React.useState(
		snapshot.catalog.defaultProfileId,
	);
	const selectedProfile =
		snapshot.catalog.profiles.find((profile) => profile.id === selectedProfileId) ??
		snapshot.catalog.profiles[0];
	if (selectedProfile === undefined) {
		throw new Error('The terminal launch-profile catalog must contain one profile.');
	}
	const selectedProfileRevision = JSON.stringify(selectedProfile);
	const selectedProfileDraft = React.useMemo<ProfileDraft>(
		() =>
			selectedProfile.command.kind === 'system-shell'
				? {
						arguments: [],
						executable: '',
						kind: 'system-shell',
						login: selectedProfile.command.login,
						name: selectedProfile.name ?? '',
					}
				: {
						arguments: [...selectedProfile.command.arguments],
						executable: selectedProfile.command.executable,
						kind: 'custom-executable',
						login: false,
						name: selectedProfile.name ?? '',
					},
		[selectedProfile],
	);
	const [draft, setDraft] = React.useState<ProfileDraft>(selectedProfileDraft);
	const [draftSource, setDraftSource] = React.useState(snapshot.preferencesSource);
	const [newProfileName, setNewProfileName] = React.useState('');
	const [localError, setLocalError] = React.useState<string | null>(null);
	const [deleteConfirmation, setDeleteConfirmation] = React.useState(false);
	const [resetConfirmation, setResetConfirmation] = React.useState(false);
	const observedProfile = React.useRef({
		draft: selectedProfileDraft,
		id: selectedProfile.id,
		revision: selectedProfileRevision,
	});

	React.useEffect(() => {
		const previous = observedProfile.current;
		observedProfile.current = {
			draft: selectedProfileDraft,
			id: selectedProfile.id,
			revision: selectedProfileRevision,
		};
		if (selectedProfile.id !== selectedProfileId) setSelectedProfileId(selectedProfile.id);
		if (
			previous.id !== selectedProfile.id ||
			JSON.stringify(draft) === JSON.stringify(previous.draft)
		) {
			setDraft(selectedProfileDraft);
			setDraftSource(snapshot.preferencesSource);
			setDeleteConfirmation(false);
			setLocalError(null);
		}
	}, [
		selectedProfile,
		selectedProfileDraft,
		selectedProfileId,
		selectedProfileRevision,
		snapshot.preferencesSource,
		draft,
	]);

	const messages = {
		add: intl.formatMessage(terminalSettingsMessages.profilesAdd),
		addNameLabel: intl.formatMessage(terminalSettingsMessages.profilesAddNameLabel),
		addNamePlaceholder: intl.formatMessage(terminalSettingsMessages.profilesAddNamePlaceholder),
		argumentAdd: intl.formatMessage(terminalSettingsMessages.profilesArgumentAdd),
		argumentsDescription: intl.formatMessage(terminalSettingsMessages.profilesArgumentsDescription),
		argumentsLabel: intl.formatMessage(terminalSettingsMessages.profilesArgumentsLabel),
		cancel: intl.formatMessage(terminalSettingsMessages.profilesCancel),
		commandCustom: intl.formatMessage(terminalSettingsMessages.profilesCommandCustom),
		commandLabel: intl.formatMessage(terminalSettingsMessages.profilesCommandLabel),
		commandSystem: intl.formatMessage(terminalSettingsMessages.profilesCommandSystem),
		confirmDelete: intl.formatMessage(terminalSettingsMessages.profilesConfirmDelete),
		confirmReset: intl.formatMessage(terminalSettingsMessages.profilesConfirmReset),
		defaultBadge: intl.formatMessage(terminalSettingsMessages.profilesDefaultBadge),
		delete: intl.formatMessage(terminalSettingsMessages.profilesDelete),
		executableDescription: intl.formatMessage(
			terminalSettingsMessages.profilesExecutableDescription,
		),
		executableLabel: intl.formatMessage(terminalSettingsMessages.profilesExecutableLabel),
		executablePlaceholder: intl.formatMessage(
			terminalSettingsMessages.profilesExecutablePlaceholder,
		),
		failureBusy: intl.formatMessage(terminalSettingsMessages.profilesFailureBusy),
		failureCatalogFull: intl.formatMessage(terminalSettingsMessages.profilesFailureCatalogFull),
		failureControlUnavailable: intl.formatMessage(
			terminalSettingsMessages.profilesFailureControlUnavailable,
		),
		failureInvalid: intl.formatMessage(terminalSettingsMessages.profilesFailureInvalid),
		failureNotFound: intl.formatMessage(terminalSettingsMessages.profilesFailureNotFound),
		failureProtectedDelete: intl.formatMessage(
			terminalSettingsMessages.profilesFailureProtectedDelete,
		),
		failureUnsupported: intl.formatMessage(terminalSettingsMessages.profilesFailureUnsupported),
		failureWrite: intl.formatMessage(terminalSettingsMessages.profilesFailureWrite),
		futureTerminalsNotice: intl.formatMessage(
			terminalSettingsMessages.profilesFutureTerminalsNotice,
		),
		loginDescription: intl.formatMessage(terminalSettingsMessages.profilesLoginDescription),
		loginLabel: intl.formatMessage(terminalSettingsMessages.profilesLoginLabel),
		loginUnsupported: intl.formatMessage(terminalSettingsMessages.profilesLoginUnsupported),
		makeDefault: intl.formatMessage(terminalSettingsMessages.profilesMakeDefault),
		nameInvalid: intl.formatMessage(terminalSettingsMessages.profilesNameInvalid),
		nameLabel: intl.formatMessage(terminalSettingsMessages.profilesNameLabel),
		persistenceInvalid: intl.formatMessage(terminalSettingsMessages.profilesPersistenceInvalid),
		persistenceRead: intl.formatMessage(terminalSettingsMessages.profilesPersistenceRead),
		persistenceWrite: intl.formatMessage(terminalSettingsMessages.profilesPersistenceWrite),
		reset: intl.formatMessage(terminalSettingsMessages.profilesReset),
		save: intl.formatMessage(terminalSettingsMessages.profilesSave),
		section: intl.formatMessage(terminalSettingsMessages.profilesSection),
		systemShell: intl.formatMessage(terminalSettingsMessages.profilesSystemShell),
	};
	const profileLabel = (profile: UserLaunchProfile): string =>
		profile.name ??
		(profile.command.kind === 'system-shell' ? messages.systemShell : messages.commandCustom);
	let operationWarning: string | null = null;
	switch (snapshot.failureCode) {
		case 'source-changed':
			operationWarning = intl.formatMessage(terminalSettingsMessages.sharedConflict);
			break;
		case 'outcome-unknown':
			operationWarning = intl.formatMessage(terminalSettingsMessages.sharedUnknown);
			break;
		case 'busy':
			operationWarning = messages.failureBusy;
			break;
		case 'catalog-full':
			operationWarning = messages.failureCatalogFull;
			break;
		case 'control-unavailable':
			operationWarning = messages.failureControlUnavailable;
			break;
		case 'default-profile-delete':
		case 'last-profile-delete':
			operationWarning = messages.failureProtectedDelete;
			break;
		case 'profile-not-found':
			operationWarning = messages.failureNotFound;
			break;
		case 'unsupported-platform':
			operationWarning = messages.failureUnsupported;
			break;
		case 'write-failed':
			operationWarning = messages.failureWrite;
			break;
		case 'identity-allocation-failed':
		case 'invalid-profile':
			operationWarning = messages.failureInvalid;
			break;
	}
	let persistenceWarning: string | null = null;
	switch (snapshot.persistenceIssue) {
		case 'invalid-record':
			persistenceWarning = messages.persistenceInvalid;
			break;
		case 'read-failed':
			persistenceWarning = messages.persistenceRead;
			break;
		case 'write-failed':
			persistenceWarning = messages.persistenceWrite;
			break;
	}
	const disabled = snapshot.pending;
	const isDefault = selectedProfile.id === snapshot.catalog.defaultProfileId;
	const canDelete = snapshot.catalog.profiles.length > 1 && !isDefault;
	const addProfile = async (): Promise<void> => {
		const name = UserLaunchProfileNameSchema.safeParse(newProfileName.trim());
		if (!name.success) {
			setLocalError(messages.nameInvalid);
			return;
		}
		setLocalError(null);
		const previousIds = new Set(snapshot.catalog.profiles.map((profile) => profile.id));
		const result = await client.createProfile(name.data, selectedProfile.id);
		if (result.kind !== 'completed') return;
		const created = result.snapshot.catalog.profiles.find(
			(profile) => !previousIds.has(profile.id),
		);
		if (created !== undefined) setSelectedProfileId(created.id);
		setNewProfileName('');
	};
	const saveProfile = async (): Promise<void> => {
		const trimmedName = draft.name.trim();
		const name =
			selectedProfile.id === BUILT_IN_USER_LAUNCH_PROFILE_ID && trimmedName === ''
				? null
				: trimmedName;
		const candidate = UserLaunchProfileSchema.safeParse({
			command:
				draft.kind === 'system-shell'
					? { kind: 'system-shell', login: draft.login }
					: {
							arguments: draft.arguments,
							executable: draft.executable,
							kind: 'custom-executable',
						},
			id: selectedProfile.id,
			name,
		});
		if (!candidate.success) {
			setLocalError(name === '' ? messages.nameInvalid : messages.failureInvalid);
			return;
		}
		setLocalError(null);
		const result = await client.updateProfile(candidate.data, draftSource);
		if (result.kind === 'completed') setDraftSource(result.snapshot.preferencesSource);
	};
	const deleteProfile = async (): Promise<void> => {
		const result = await client.deleteProfile(selectedProfile.id);
		setDeleteConfirmation(false);
		if (result.kind === 'completed') {
			setSelectedProfileId(result.snapshot.catalog.defaultProfileId);
		}
	};
	const resetProfiles = async (): Promise<void> => {
		const result = await client.reset();
		setResetConfirmation(false);
		if (result.kind === 'completed') {
			setSelectedProfileId(result.snapshot.catalog.defaultProfileId);
		}
	};
	const commandCommon: CommandEditorCommonView = {
		disabled,
		label: messages.commandLabel,
		onKindChange: (event): void => {
			const kind = event.currentTarget.value;
			if (kind === 'system-shell' || kind === 'custom-executable') {
				setDraft((current) => ({ ...current, kind }));
			}
		},
		options: [
			{ label: messages.commandSystem, value: 'system-shell' },
			{ label: messages.commandCustom, value: 'custom-executable' },
		],
	};
	const commandEditor: CommandEditorView =
		draft.kind === 'system-shell'
			? {
					...commandCommon,
					description: snapshot.capabilities.loginShell
						? messages.loginDescription
						: messages.loginUnsupported,
					kind: 'system-shell',
					login: draft.login,
					loginDisabled: disabled || !snapshot.capabilities.loginShell,
					loginLabel: messages.loginLabel,
					onLoginChange: (checked): void => setDraft((current) => ({ ...current, login: checked })),
				}
			: {
					...commandCommon,
					addArgumentDisabled:
						disabled || draft.arguments.length >= USER_LAUNCH_PROFILE_LIMITS.maximumArgumentCount,
					addArgumentLabel: messages.argumentAdd,
					arguments: draft.arguments.map((argument, index) => ({
						disabled,
						inputLabel: intl.formatMessage(terminalSettingsMessages.profilesArgumentLabel, {
							number: index + 1,
						}),
						key: index,
						onChange: (event): void => {
							const nextArgument = event.currentTarget.value;
							setDraft((current) => ({
								...current,
								arguments: current.arguments.map((value, candidateIndex) =>
									candidateIndex === index ? nextArgument : value,
								),
							}));
						},
						onRemove: (): void =>
							setDraft((current) => ({
								...current,
								arguments: current.arguments.filter(
									(_value, candidateIndex) => candidateIndex !== index,
								),
							})),
						removeLabel: intl.formatMessage(terminalSettingsMessages.profilesArgumentRemove, {
							number: index + 1,
						}),
						value: argument,
					})),
					argumentsDescription: messages.argumentsDescription,
					argumentsLabel: messages.argumentsLabel,
					executableDescription: messages.executableDescription,
					executableLabel: messages.executableLabel,
					executablePlaceholder: messages.executablePlaceholder,
					executableValue: draft.executable,
					kind: 'custom-executable',
					onAddArgument: (): void =>
						setDraft((current) => ({
							...current,
							arguments: [...current.arguments, ''],
						})),
					onExecutableChange: (event): void => {
						const executable = event.currentTarget.value;
						setDraft((current) => ({ ...current, executable }));
					},
				};

	return {
		addProfile: {
			buttonDisabled:
				disabled ||
				snapshot.catalog.profiles.length >= USER_LAUNCH_PROFILE_LIMITS.maximumProfileCount,
			buttonLabel: messages.add,
			inputDisabled: disabled,
			inputLabel: messages.addNameLabel,
			inputPlaceholder: messages.addNamePlaceholder,
			inputValue: newProfileName,
			onInputChange: (event): void => setNewProfileName(event.currentTarget.value),
			onSubmit: (event): void => {
				event.preventDefault();
				void addProfile();
			},
		},
		commandEditor,
		deleteAction: deleteConfirmation
			? {
					cancelLabel: messages.cancel,
					confirmLabel: messages.confirmDelete,
					disabled,
					kind: 'confirm',
					onCancel: (): void => setDeleteConfirmation(false),
					onConfirm: (): void => void deleteProfile(),
				}
			: {
					disabled: disabled || !canDelete,
					kind: 'request',
					label: messages.delete,
					onRequest: (): void => setDeleteConfirmation(true),
				},
		makeDefaultAction: {
			disabled: disabled || isDefault,
			label: messages.makeDefault,
			onClick: (): void => void client.selectDefault(selectedProfile.id),
		},
		nameEditor: {
			disabled,
			label: messages.nameLabel,
			onChange: (event): void => {
				const name = event.currentTarget.value;
				setDraft((current) => ({ ...current, name }));
			},
			placeholder: profileLabel(selectedProfile),
			required: selectedProfile.id !== BUILT_IN_USER_LAUNCH_PROFILE_ID,
			value: draft.name,
		},
		notice: messages.futureTerminalsNotice,
		profiles: snapshot.catalog.profiles.map((profile) => ({
			defaultLabel: profile.id === snapshot.catalog.defaultProfileId ? messages.defaultBadge : null,
			id: profile.id,
			label: profileLabel(profile),
			onSelect: (): void => setSelectedProfileId(profile.id),
			selected: profile.id === selectedProfile.id,
		})),
		resetAction: resetConfirmation
			? {
					cancelLabel: messages.cancel,
					confirmLabel: messages.confirmReset,
					disabled,
					kind: 'confirm',
					onCancel: (): void => setResetConfirmation(false),
					onConfirm: (): void => void resetProfiles(),
				}
			: {
					disabled,
					kind: 'request',
					label: messages.reset,
					onRequest: (): void => setResetConfirmation(true),
				},
		saveAction: {
			disabled,
			label: messages.save,
			onClick: (): void => void saveProfile(),
		},
		sectionLabel: messages.section,
		warning: localError ?? persistenceWarning ?? operationWarning,
	};
}
