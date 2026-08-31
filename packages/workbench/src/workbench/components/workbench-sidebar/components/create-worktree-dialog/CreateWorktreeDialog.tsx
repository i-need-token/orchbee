import {
	Checkbox,
	Combobox,
	ComboboxContent,
	ComboboxEmpty,
	ComboboxInput,
	ComboboxItem,
	ComboboxList,
	Dialog,
	DialogContent,
	Field,
	FieldLabel,
	Input,
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type {
	WorkbenchProjectWorktreeDiscovery,
	WorkbenchProjectWorktreeFailureCode,
} from '../../../../types/workbench-project-worktree';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-header/ResourceDialogHeader';
import styles from './CreateWorktreeDialog.module.css';

interface CreateWorktreeDialogProps {
	readonly baseRef: string;
	readonly branchName: string;
	readonly createWorkspace: boolean;
	readonly discovery: WorkbenchProjectWorktreeDiscovery | null;
	readonly failure: WorkbenchProjectWorktreeFailureCode | null;
	readonly loading: boolean;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly onBaseRefChange: (value: string) => void;
	readonly onBranchNameChange: (value: string) => void;
	readonly onClose: () => void;
	readonly onCreateWorkspaceChange: (value: boolean) => void;
	readonly onRepositoryChange: (repositoryId: string) => void;
	readonly onSubmit: () => void;
	readonly repositoryId: string;
	readonly submitting: boolean;
}

export function CreateWorktreeDialog({
	baseRef,
	branchName,
	createWorkspace,
	discovery,
	failure,
	loading,
	messages,
	onBaseRefChange,
	onBranchNameChange,
	onClose,
	onCreateWorkspaceChange,
	onRepositoryChange,
	onSubmit,
	repositoryId,
	submitting,
}: CreateWorktreeDialogProps): React.ReactNode {
	const formId = 'workbench-project-worktree-create-form';
	const repository = discovery?.repositories.find(
		(candidate) => candidate.repositoryId === repositoryId,
	);
	const showRepositorySelection = (discovery?.repositories.length ?? 0) > 1;
	const selectedBaseRef = repository?.localBaseRefs.includes(baseRef) === true ? baseRef : null;
	const submitDisabled =
		loading ||
		submitting ||
		repository === undefined ||
		baseRef.trim().length === 0 ||
		branchName.trim().length === 0;
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open && !submitting) onClose();
			}}
			open
		>
			<DialogContent closeLabel={messages.close} data-slot='workbench-project-worktree-create'>
				<ResourceDialogHeader
					description={messages.projectWorktreeDescription}
					title={messages.projectWorktreeTitle}
				/>
				<ResourceDialogForm id={formId} onSubmit={onSubmit} submitDisabled={submitDisabled}>
					{loading ? <p className={styles['status']}>{messages.projectWorktreeLoading}</p> : null}
					{!loading && discovery?.repositories.length === 0 ? (
						<p className={styles['status']}>{messages.projectWorktreeNoRepositories}</p>
					) : null}
					{failure === null ? null : (
						<div className={styles['failure']} role='alert'>
							<span>{messages.projectWorktreeFailureTitle}</span>
							<span>{messages.projectWorktreeFailureMessages[failure]}</span>
						</div>
					)}
					{repository === undefined ? null : (
						<>
							<Field>
								<FieldLabel htmlFor='project-worktree-branch'>
									{messages.projectWorktreeBranch}
								</FieldLabel>
								<Input
									autoCapitalize='none'
									autoComplete='off'
									autoCorrect='off'
									autoFocus
									data-code-like='true'
									id='project-worktree-branch'
									required
									spellCheck={false}
									value={branchName}
									onChange={(event) => onBranchNameChange(event.currentTarget.value)}
								/>
							</Field>
							{showRepositorySelection ? (
								<Field>
									<FieldLabel htmlFor='project-worktree-repository'>
										{messages.projectWorktreeRepository}
									</FieldLabel>
									<Select
										value={repositoryId}
										onValueChange={(value) => {
											if (value !== null) onRepositoryChange(value);
										}}
									>
										<SelectTrigger className={styles['select']} id='project-worktree-repository'>
											<SelectValue />
										</SelectTrigger>
										<SelectContent
											align='start'
											alignItemWithTrigger={false}
											positionerClassName={styles['modalPopup']}
										>
											{discovery?.repositories.map((candidate) => (
												<SelectItem key={candidate.repositoryId} value={candidate.repositoryId}>
													{candidate.label}
												</SelectItem>
											))}
										</SelectContent>
									</Select>
								</Field>
							) : null}
							<Field>
								<FieldLabel htmlFor='project-worktree-base'>
									{messages.projectWorktreeBase}
								</FieldLabel>
								<Combobox
									autoComplete='off'
									autoHighlight
									inputValue={baseRef}
									items={repository.localBaseRefs}
									value={selectedBaseRef}
									onInputValueChange={onBaseRefChange}
									onValueChange={(value) => {
										if (value !== null) onBaseRefChange(value);
									}}
								>
									<ComboboxInput
										autoCapitalize='none'
										autoCorrect='off'
										className={styles['select']}
										data-code-like='true'
										id='project-worktree-base'
										required
										spellCheck={false}
									/>
									<ComboboxContent positionerClassName={styles['modalPopup']}>
										<ComboboxEmpty>{messages.projectWorktreeBaseNoMatches}</ComboboxEmpty>
										<ComboboxList>
											{(reference: string) => (
												<ComboboxItem key={reference} value={reference}>
													{reference}
												</ComboboxItem>
											)}
										</ComboboxList>
									</ComboboxContent>
								</Combobox>
							</Field>
							<dl className={styles['metadata']} data-slot='project-worktree-location'>
								<dt className={styles['metadataLabel']}>{messages.projectWorktreeLocation}</dt>
								<dd>
									<code className={styles['metadataValue']}>
										{messages.projectWorktreeLocationAutomatic(discovery?.managedRoot ?? '')}
									</code>
								</dd>
							</dl>
							<label className={styles['checkbox']}>
								<Checkbox
									checked={createWorkspace}
									onCheckedChange={(checked) => onCreateWorkspaceChange(checked === true)}
								/>
								<span>{messages.projectWorktreeCreateWorkspace}</span>
							</label>
						</>
					)}
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelDisabled={submitting}
					cancelLabel={messages.cancel}
					formId={formId}
					onCancel={onClose}
					showCancelShortcut={false}
					showSubmitShortcut={false}
					submitDisabled={submitDisabled}
					submitLabel={messages.create}
				/>
			</DialogContent>
		</Dialog>
	);
}
