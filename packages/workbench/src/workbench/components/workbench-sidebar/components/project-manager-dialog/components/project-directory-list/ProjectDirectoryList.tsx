import {
	Button,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
	FieldDescription,
	Field,
	FieldTitle,
	Input,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchProject } from '../../../../../../../project-workspace';
import type { WorkbenchProjectNavigationMessages } from '../../../../types/workbench-sidebar-types';
import styles from './ProjectDirectoryList.module.css';

interface ProjectDirectoryListProps {
	readonly addingFolder: boolean;
	readonly canAddFolder: boolean;
	readonly closeFolderInput: () => void;
	readonly folderAlreadyAdded: boolean;
	readonly folderPath: string;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly nativeSelectionPending?: boolean | undefined;
	readonly onFolderPathKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onRemoveDirectory: (directoryId: string) => void;
	readonly onSetPrimaryDirectory: (directoryId: string) => void;
	readonly onSelectFolder?: (() => void) | undefined;
	readonly onSelectSshFolder?: (() => void) | undefined;
	readonly project: WorkbenchProject;
	readonly setFolderPath: (path: string) => void;
	readonly showFolderInput: () => void;
	readonly submitFolder: () => void;
}

export function ProjectDirectoryList({
	addingFolder,
	canAddFolder,
	closeFolderInput,
	folderAlreadyAdded,
	folderPath,
	messages,
	nativeSelectionPending = false,
	onFolderPathKeyDown,
	onRemoveDirectory,
	onSetPrimaryDirectory,
	onSelectFolder,
	onSelectSshFolder,
	project,
	setFolderPath,
	showFolderInput,
	submitFolder,
}: ProjectDirectoryListProps): React.ReactNode {
	const showPrimaryControls = project.directories.length > 1;
	const folderInputDescriptionId = `project-folder-add-description-${project.id}`;
	const addFolderControl =
		onSelectSshFolder === undefined ? (
			<Button
				className={styles['addButton']}
				disabled={nativeSelectionPending}
				type='button'
				variant='ghost'
				onClick={onSelectFolder ?? showFolderInput}
			>
				<ProductIcon name={onSelectFolder === undefined ? 'new-folder' : 'folder-opened'} />
				{messages.projectManageAddFolder}
			</Button>
		) : (
			<DropdownMenu>
				<DropdownMenuTrigger
					render={
						<Button
							aria-label={messages.projectManageAddFolder}
							className={styles['addButton']}
							disabled={nativeSelectionPending}
							type='button'
							variant='ghost'
						/>
					}
				>
					<ProductIcon name='new-folder' />
					{messages.projectManageAddFolder}
				</DropdownMenuTrigger>
				<DropdownMenuContent align='start' positionerClassName={styles['modalMenu']} side='bottom'>
					<DropdownMenuItem onClick={onSelectFolder ?? showFolderInput}>
						<ProductIcon
							name={onSelectFolder === undefined ? 'new-folder' : 'folder-opened'}
							size='menu-leading'
						/>
						{messages.projectManageAddLocalFolder}
					</DropdownMenuItem>
					<DropdownMenuItem onClick={onSelectSshFolder}>
						<ProductIcon name='terminal' size='menu-leading' />
						{messages.projectManageAddSshFolder}
					</DropdownMenuItem>
				</DropdownMenuContent>
			</DropdownMenu>
		);

	return (
		<Field>
			<FieldTitle>{messages.projectManageFolders}</FieldTitle>
			<ul className={styles['list']}>
				{project.directories.map((directory) => {
					const primary = project.primaryDirectoryId === directory.id;
					const remote = directory.placement.kind === 'environment';
					const removable = directory.kind === 'folder' && project.directories.length > 1;
					return (
						<li
							className={styles['directoryRow']}
							data-primary={primary || undefined}
							key={directory.id}
						>
							<span className={styles['directoryIdentity']}>
								<span className={styles['icon']}>
									<ProductIcon name='folder' />
								</span>
								<Tooltip>
									<TooltipTrigger render={<span className={styles['directoryName']} />}>
										{directory.name}
									</TooltipTrigger>
									<TooltipContent side='top'>{directory.path}</TooltipContent>
								</Tooltip>
								{remote && (
									<span
										aria-label={messages.projectManageSshFolderDescription}
										className={styles['remoteBadge']}
									>
										{messages.projectManageSshFolderBadge}
									</span>
								)}
							</span>
							<span className={styles['directoryActions']}>
								{showPrimaryControls &&
									(primary ? (
										<span className={styles['primaryBadge']}>
											{messages.projectManagePrimaryFolder}
										</span>
									) : (
										<Button
											className={styles['setPrimaryButton']}
											disabled={!directory.available}
											size='xs'
											type='button'
											variant='ghost'
											onClick={() => onSetPrimaryDirectory(directory.id)}
										>
											{messages.setProjectPrimaryFolder}
										</Button>
									))}
								<Button
									aria-label={`${messages.removeProjectFolder}: ${directory.name}`}
									disabled={!removable}
									size='icon-xs'
									type='button'
									variant='ghost'
									onClick={() => onRemoveDirectory(directory.id)}
								>
									<ProductIcon name='close' />
								</Button>
							</span>
						</li>
					);
				})}
				<li className={styles['addRow']} data-editing={addingFolder || undefined}>
					{addingFolder ? (
						<div className={styles['addEditor']}>
							<div className={styles['addControls']}>
								<Input
									aria-describedby={folderAlreadyAdded ? folderInputDescriptionId : undefined}
									aria-invalid={folderAlreadyAdded || undefined}
									aria-label={messages.projectCreatePath}
									autoFocus
									className={styles['addInput']}
									id={`project-folder-add-${project.id}`}
									placeholder={messages.projectCreatePathPlaceholder}
									value={folderPath}
									onChange={(event) => setFolderPath(event.currentTarget.value)}
									onKeyDown={onFolderPathKeyDown}
								/>
								<div className={styles['addActions']}>
									<Tooltip>
										<TooltipTrigger
											render={
												<Button
													aria-label={messages.cancel}
													className={styles['addActionButton']}
													size='icon-xs'
													type='button'
													variant='ghost'
													onClick={closeFolderInput}
												/>
											}
										>
											<ProductIcon name='close' />
										</TooltipTrigger>
										<TooltipContent side='top'>
											{messages.cancel}
											<Kbd aria-hidden='true'>Esc</Kbd>
										</TooltipContent>
									</Tooltip>
									<Tooltip>
										<TooltipTrigger
											render={
												<Button
													aria-label={messages.add}
													className={styles['addActionButton']}
													disabled={!canAddFolder}
													size='icon-xs'
													type='button'
													variant='ghost'
													onClick={submitFolder}
												/>
											}
										>
											<ProductIcon name='check' />
										</TooltipTrigger>
										<TooltipContent side='top'>
											{messages.add}
											<Kbd aria-hidden='true'>Enter</Kbd>
										</TooltipContent>
									</Tooltip>
								</div>
							</div>
							{folderAlreadyAdded && (
								<FieldDescription id={folderInputDescriptionId}>
									{messages.projectManageFolderAlreadyAdded}
								</FieldDescription>
							)}
						</div>
					) : (
						addFolderControl
					)}
				</li>
			</ul>
		</Field>
	);
}
