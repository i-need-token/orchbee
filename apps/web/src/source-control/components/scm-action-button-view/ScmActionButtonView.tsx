import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	sourceControlMessages,
	useOrchbeeIntl,
	type IntlShape,
	type MessageDescriptor,
} from '@orchbee/i18n';
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuGroup,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
	type ProductIconName,
} from '@orchbee/ui/components';
import * as React from 'react';

import type {
	ScmActionButton,
	ScmActionButtonCommand,
	ScmActionButtonIcon,
	ScmActionButtonTitle,
	ScmActionButtonTooltip,
} from '../../model/scm-action-button';
import { scmSyncTooltipText } from '../../utils/scmSyncTooltipText';
import styles from './ScmActionButtonView.module.css';

const TITLE_MESSAGES = {
	commit: sourceControlMessages.buttonCommit,
	'commit-amend': sourceControlMessages.buttonCommitAmend,
	'commit-and-push': sourceControlMessages.buttonCommitAndPush,
	'commit-and-sync': sourceControlMessages.buttonCommitAndSync,
	continue: sourceControlMessages.buttonContinue,
	publish: sourceControlMessages.buttonPublish,
	sync: sourceControlMessages.buttonSync,
} as const satisfies Record<ScmActionButtonTitle['kind'], MessageDescriptor>;

function titleText(intl: IntlShape, title: ScmActionButtonTitle): string {
	return intl.formatMessage(TITLE_MESSAGES[title.kind]);
}

function tooltipText(intl: IntlShape, tooltip: ScmActionButtonTooltip): string {
	switch (tooltip.kind) {
		case 'commit':
			return tooltip.branch === null
				? intl.formatMessage(sourceControlMessages.buttonTooltipCommit)
				: intl.formatMessage(sourceControlMessages.buttonTooltipCommitOnBranch, {
						branch: tooltip.branch,
					});
		case 'publish':
			return intl.formatMessage(sourceControlMessages.buttonTooltipPublish, {
				branch: tooltip.branch,
			});
		case 'publishing':
			return intl.formatMessage(sourceControlMessages.buttonTooltipPublishing, {
				branch: tooltip.branch,
			});
		case 'sync':
			return scmSyncTooltipText(intl, tooltip.tooltip) ?? '';
		case 'commit-and-push':
			return intl.formatMessage(sourceControlMessages.buttonTooltipCommitAndPush);
		case 'commit-and-sync':
			return intl.formatMessage(sourceControlMessages.buttonTooltipCommitAndSync);
		case 'committing':
			return intl.formatMessage(sourceControlMessages.syncCommitting);
		case 'committing-and-pushing':
			return intl.formatMessage(sourceControlMessages.buttonTooltipCommittingAndPushing);
		case 'committing-and-syncing':
			return intl.formatMessage(sourceControlMessages.buttonTooltipCommittingAndSyncing);
		case 'continue-merge':
			return intl.formatMessage(sourceControlMessages.buttonTooltipContinueMerge);
		case 'continue-rebase':
			return intl.formatMessage(sourceControlMessages.buttonTooltipContinueRebase);
		case 'continuing-merge':
			return intl.formatMessage(sourceControlMessages.buttonTooltipContinuingMerge);
		case 'continuing-rebase':
			return intl.formatMessage(sourceControlMessages.buttonTooltipContinuingRebase);
		case 'syncing':
			return intl.formatMessage(sourceControlMessages.syncSyncing);
	}
}

/** The codicon a title starts with; a spinning sync draws `sync` and turns. */
function iconName(icon: ScmActionButtonIcon): ProductIconName {
	return icon === 'syncing' ? 'sync' : icon;
}

interface ScmActionButtonViewProps {
	readonly button: ScmActionButton;
	/** Gives what focuses the button while it shows, and null once it is gone. */
	readonly onFocusTarget: (focus: (() => void) | null) => void;
	readonly onRun: (command: ScmActionButtonCommand) => void;
}

/**
 * VS Code's SCM action button below a repository's commit input: the primary command across the
 * row, with its title after its icon, and the dropdown of its other commands beside it when it has
 * them. Each shows the command's tooltip, and the whole button disables at once.
 */
export function ScmActionButtonView({
	button,
	onFocusTarget,
	onRun,
}: ScmActionButtonViewProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [open, setOpen] = React.useState(false);
	const { enabled, primary, secondary } = button;
	const tooltip = tooltipText(intl, primary.tooltip);
	const moreActions = intl.formatMessage(sourceControlMessages.moreActions);
	const focusReference = useMemoizedFn((element: HTMLButtonElement | null) => {
		onFocusTarget(element === null ? null : () => element.focus());
	});
	const { title } = primary;
	return (
		<div
			className={styles['root']}
			data-disabled={enabled ? undefined : ''}
			data-slot='scm-action-button'
		>
			<Tooltip>
				<TooltipTrigger
					render={
						<button
							aria-label={tooltip}
							className={styles['primary']}
							disabled={!enabled}
							ref={focusReference}
							type='button'
							onClick={() => onRun(primary.command)}
						/>
					}
				>
					<span
						className={styles['icon']}
						data-spinning={primary.icon === 'syncing' ? '' : undefined}
					>
						<ProductIcon name={iconName(primary.icon)} size='section-action' />
					</span>
					<span className={styles['label']}>{titleText(intl, title)}</span>
					{title.kind === 'sync' && title.behind > 0 ? (
						<span className={styles['count']}>
							{intl.formatNumber(title.behind)}
							<ProductIcon name='arrow-down' size='section-action' />
						</span>
					) : null}
					{title.kind === 'sync' && title.ahead > 0 ? (
						<span className={styles['count']}>
							{intl.formatNumber(title.ahead)}
							<ProductIcon name='arrow-up' size='section-action' />
						</span>
					) : null}
				</TooltipTrigger>
				<TooltipContent side='bottom'>{tooltip}</TooltipContent>
			</Tooltip>
			{secondary.length === 0 ? null : (
				<>
					<span className={styles['separator']} />
					<DropdownMenu open={open} onOpenChange={setOpen}>
						<Tooltip>
							<TooltipTrigger
								render={
									<DropdownMenuTrigger
										render={
											<button
												aria-label={moreActions}
												className={styles['dropdown']}
												disabled={!enabled}
												type='button'
											/>
										}
									/>
								}
							>
								<ProductIcon name='chevron-down' size='section-action' />
							</TooltipTrigger>
							<TooltipContent side='bottom'>{moreActions}</TooltipContent>
						</Tooltip>
						{open ? (
							<DropdownMenuContent align='end'>
								{secondary.map((items, index) => (
									<React.Fragment key={items[0]?.title.kind ?? index}>
										{index > 0 ? <DropdownMenuSeparator /> : null}
										<DropdownMenuGroup>
											{items.map((item) => (
												<DropdownMenuItem key={item.title.kind} onClick={() => onRun(item.command)}>
													{titleText(intl, item.title)}
												</DropdownMenuItem>
											))}
										</DropdownMenuGroup>
									</React.Fragment>
								))}
							</DropdownMenuContent>
						) : null}
					</DropdownMenu>
				</>
			)}
		</div>
	);
}
