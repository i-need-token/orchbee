import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { FilePathRules } from '../../../file-session';
import type { ScmWelcomeItem } from '../../model/scmWelcomeItems';
import styles from './ScmWelcome.module.css';

interface ScmWelcomeProps {
	readonly items: readonly ScmWelcomeItem[];
	readonly onInitialize: () => void;
	readonly onManageUnsafe: () => void;
	readonly onOpenParent: () => void;
	readonly onReopenClosed: () => void;
	/** The system of the machine that holds the directory, whose Git download the welcome offers. */
	readonly os: FilePathRules['os'] | null;
}

/** Git's download page for each system, as VS Code's welcome links them. */
const GIT_DOWNLOADS = {
	linux: 'https://git-scm.com/download/linux',
	macos: 'https://git-scm.com/download/mac',
	windows: 'https://git-scm.com/download/win',
} as const satisfies Record<FilePathRules['os'], string>;

const GIT_DOWNLOAD_MESSAGES = {
	linux: sourceControlMessages.welcomeMissingGitDownloadLinux,
	macos: sourceControlMessages.welcomeMissingGitDownloadMac,
	windows: sourceControlMessages.welcomeMissingGitDownloadWindows,
} as const satisfies Record<FilePathRules['os'], unknown>;

interface ScmWelcomeEntryProps {
	readonly item: ScmWelcomeItem;
	readonly props: ScmWelcomeProps;
}

function ScmWelcomeEntry({ item, props }: ScmWelcomeEntryProps): ReactNode {
	const intl = useOrchbeeIntl();
	const text = (message: string): ReactNode => <p className={styles['text']}>{message}</p>;
	const button = (label: string, onPress: () => void): ReactNode => (
		<Button className={styles['button']} type='button' onClick={onPress}>
			{label}
		</Button>
	);
	switch (item.kind) {
		case 'closed':
			return (
				<>
					{text(
						intl.formatMessage(sourceControlMessages.welcomeClosedRepositories, {
							count: item.count,
						}),
					)}
					{button(
						intl.formatMessage(sourceControlMessages.welcomeReopenClosed, { count: item.count }),
						props.onReopenClosed,
					)}
				</>
			);
		case 'missing-git': {
			const { os } = props;
			// A machine whose system is not known yet gets VS Code's words for a remote one.
			if (os === null)
				return text(intl.formatMessage(sourceControlMessages.welcomeMissingGitRemote));
			return (
				<>
					{os === 'linux'
						? text(intl.formatMessage(sourceControlMessages.welcomeMissingGitLinux))
						: null}
					<Button
						className={styles['button']}
						nativeButton={false}
						render={<a href={GIT_DOWNLOADS[os]} rel='noreferrer' target='_blank' />}
					>
						{intl.formatMessage(GIT_DOWNLOAD_MESSAGES[os])}
					</Button>
				</>
			);
		}
		case 'no-directory':
			return text(intl.formatMessage(sourceControlMessages.welcomeNoDirectory));
		case 'no-repository':
			return (
				<>
					{text(intl.formatMessage(sourceControlMessages.welcomeNoRepository))}
					{button(intl.formatMessage(sourceControlMessages.welcomeInitialize), props.onInitialize)}
				</>
			);
		case 'parent':
			return (
				<>
					{text(
						intl.formatMessage(sourceControlMessages.welcomeParentRepositories, {
							count: item.count,
						}),
					)}
					{button(
						intl.formatMessage(sourceControlMessages.welcomeOpenRepository),
						props.onOpenParent,
					)}
				</>
			);
		case 'scanning':
			return text(intl.formatMessage(sourceControlMessages.welcomeScanning));
		case 'unsafe':
			return (
				<>
					{text(
						intl.formatMessage(sourceControlMessages.welcomeUnsafeRepositories, {
							count: item.count,
						}),
					)}
					{button(
						intl.formatMessage(sourceControlMessages.welcomeManageUnsafe),
						props.onManageUnsafe,
					)}
				</>
			);
	}
}

/**
 * VS Code's Source Control welcome, shown while no repository is displayed: why, in its words, and
 * the button that changes it, one paragraph for each reason that holds.
 */
export function ScmWelcome(props: ScmWelcomeProps): ReactNode {
	return (
		<div className={styles['root']} data-slot='scm-welcome'>
			{props.items.map((item) => (
				<ScmWelcomeEntry key={item.kind} item={item} props={props} />
			))}
		</div>
	);
}
