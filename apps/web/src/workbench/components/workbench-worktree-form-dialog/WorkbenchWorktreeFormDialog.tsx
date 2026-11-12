import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	Dialog,
	DialogContent,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import { WorktreeReferenceSchema } from '@orchbee/workbench-client-protocol';
import { useId, useRef, useState, type ReactNode, type RefObject } from 'react';

import type { WorkbenchWorktreeFormAnswer } from '../../types/workbench-worktree-form';
import { ResourceDialogForm } from '../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../resource-dialog-header/ResourceDialogHeader';
import styles from './WorkbenchWorktreeFormDialog.module.css';

const MAXIMUM_REFERENCE_BYTES = 1024;
const utf8 = new TextEncoder();

type FieldName = 'base' | 'branch';
type Refusal = 'invalid' | 'too-long';

interface WorkbenchWorktreeFormDialogProps {
	/** The Branch field's starting text: blank, or the branch Start Agent proposes. */
	readonly branch: string;
	readonly finalFocus?: (() => HTMLElement | null) | undefined;
	/** Cancel, Escape, the close button and the backdrop. */
	readonly onCancel: () => void;
	readonly onSubmit: (answer: WorkbenchWorktreeFormAnswer) => void;
}

/** Why a field's text cannot be sent, by the TUI's rule; blank takes the field's default. */
function refusal(text: string): Refusal | null {
	if (text === '') return null;
	if (utf8.encode(text).length > MAXIMUM_REFERENCE_BYTES) return 'too-long';
	return WorktreeReferenceSchema.safeParse(text).success ? null : 'invalid';
}

/**
 * The TUI's New Worktree form: Branch, blank for a generated name, and Base, blank for HEAD. Text is
 * kept exactly as typed, a refused field says why when it is left or the form is submitted, and
 * Create sends only a form with no refused field.
 */
export function WorkbenchWorktreeFormDialog({
	branch: initialBranch,
	finalFocus,
	onCancel,
	onSubmit,
}: WorkbenchWorktreeFormDialogProps): ReactNode {
	const intl = useOrchbeeIntl();
	const formId = useId();
	const branchId = useId();
	const baseId = useId();
	// Not `autoFocus`: the menu this opens from returns focus as it closes, after that has run.
	const branchInput = useRef<HTMLInputElement>(null);
	const baseInput = useRef<HTMLInputElement>(null);
	const [branch, setBranch] = useState(initialBranch);
	const [base, setBase] = useState('');
	const [validated, setValidated] = useState<ReadonlySet<FieldName>>(new Set());
	const validate = (field: FieldName) =>
		setValidated((current) => (current.has(field) ? current : new Set([...current, field])));
	const errors = { base: refusal(base), branch: refusal(branch) };
	const message = (field: FieldName, cause: Refusal): string => {
		if (cause === 'too-long') return intl.formatMessage(workbenchMessages.worktreeFormTooLong);
		return intl.formatMessage(
			field === 'branch'
				? workbenchMessages.worktreeFormBranchInvalid
				: workbenchMessages.worktreeFormBaseInvalid,
		);
	};

	const field = (
		name: FieldName,
		{
			help,
			id,
			input,
			label,
			placeholder,
			setValue,
			value,
		}: {
			readonly help?: string | undefined;
			readonly id: string;
			readonly input: RefObject<HTMLInputElement | null>;
			readonly label: string;
			readonly placeholder?: string | undefined;
			readonly setValue: (value: string) => void;
			readonly value: string;
		},
	): ReactNode => {
		const error = validated.has(name) ? errors[name] : null;
		const descriptionId = `${id}-description`;
		let description: ReactNode = null;
		if (error !== null) {
			description = (
				<p className={styles['error']} id={descriptionId} role='alert'>
					{message(name, error)}
				</p>
			);
		} else if (help !== undefined) {
			description = <FieldDescription id={descriptionId}>{help}</FieldDescription>;
		}
		return (
			<Field data-invalid={error !== null || undefined}>
				<FieldLabel htmlFor={id}>{label}</FieldLabel>
				<Input
					aria-describedby={description === null ? undefined : descriptionId}
					aria-invalid={error !== null || undefined}
					autoCapitalize='off'
					autoComplete='off'
					autoCorrect='off'
					data-code-like='true'
					id={id}
					placeholder={placeholder}
					ref={input}
					spellCheck={false}
					value={value}
					onBlur={() => validate(name)}
					onChange={(event) => setValue(event.currentTarget.value)}
				/>
				{description}
			</Field>
		);
	};

	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
			open
		>
			<DialogContent
				closeLabel={intl.formatMessage(workbenchMessages.resourceClose)}
				data-slot='workbench-worktree-form-dialog'
				finalFocus={finalFocus}
				initialFocus={branchInput}
			>
				<ResourceDialogHeader title={intl.formatMessage(workbenchMessages.worktreeFormTitle)} />
				<ResourceDialogForm
					id={formId}
					onSubmit={() => {
						if (errors.branch === null && errors.base === null) {
							onSubmit({ base: base === '' ? null : base, branch: branch === '' ? null : branch });
							return;
						}
						setValidated(new Set<FieldName>(['base', 'branch']));
						(errors.branch === null ? baseInput : branchInput).current?.focus();
					}}
					submitDisabled={false}
				>
					{field('branch', {
						help: intl.formatMessage(workbenchMessages.worktreeFormBranchHelp),
						id: branchId,
						input: branchInput,
						label: intl.formatMessage(workbenchMessages.worktreeFormBranch),
						setValue: setBranch,
						value: branch,
					})}
					{field('base', {
						id: baseId,
						input: baseInput,
						label: intl.formatMessage(workbenchMessages.worktreeFormBase),
						placeholder: 'HEAD',
						setValue: setBase,
						value: base,
					})}
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelLabel={intl.formatMessage(workbenchMessages.resourceCancel)}
					formId={formId}
					onCancel={onCancel}
					submitDisabled={false}
					submitLabel={intl.formatMessage(workbenchMessages.resourceCreate)}
				/>
			</DialogContent>
		</Dialog>
	);
}
