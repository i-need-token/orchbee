import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import { Fragment, type ReactElement } from 'react';

import { pagePath } from '../../pages';
import {
	BUILD_GUIDE_URL,
	NPM_INSTALL_COMMAND,
	publishedPlatforms,
	type Release,
} from '../../release';
import { CommandChip } from '../command-chip/CommandChip';
import { LinkButton } from '../link-button/LinkButton';
import styles from './GetOrchbee.module.css';

// The way to get Orchbee that a call to action offers. Once a release is published it offers a download button
// for each platform the release published, in catalog order (activateGetOrchbee moves the visitor's platform
// first and marks it primary), then a link to each platform's other installers and a link to all installation
// options. The command line call to action offers the npm package instead. Before a release, every call to
// action leads to the source build.
export function GetOrchbee({
	locale,
	release,
	offer,
}: {
	locale: OrchbeeLocale;
	release: Release;
	offer: 'both' | 'installer' | 'npm';
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	if (!release.published) {
		return (
			<div className={styles['root']}>
				<div className={styles['actions']}>
					<LinkButton href={BUILD_GUIDE_URL}>{t(messages.getSource)}</LinkButton>
				</div>
			</div>
		);
	}
	const platforms = publishedPlatforms(release);
	const otherInstallers = platforms.flatMap(({ variants }) => variants.slice(1));
	const installHref = `${pagePath('home', locale)}#install`;

	return (
		<div className={styles['root']} data-get-orchbee=''>
			<div className={styles['actions']}>
				{offer === 'npm' ? (
					<CommandChip command={NPM_INSTALL_COMMAND} />
				) : (
					platforms.map(({ platform, variants }, index) => {
						const offered = variants[0];
						return offered === undefined ? null : (
							<LinkButton
								href={offered.url}
								icon={platform.icon}
								platform={platform.id}
								primary={index === 0}
								key={platform.id}
							>
								{t(messages[offered.cta])}
							</LinkButton>
						);
					})
				)}
			</div>
			<div className={styles['meta']}>
				{offer === 'npm'
					? null
					: otherInstallers.map((variant) => (
							<Fragment key={variant.download}>
								<a className={styles['link']} href={variant.url}>
									↓ {t(messages[variant.link ?? variant.cta])}
								</a>
								<span className={styles['dot']} aria-hidden='true'>
									·
								</span>
							</Fragment>
						))}
				<a className={styles['link']} href={installHref}>
					{t(messages.allInstallOptions)} (curl / npm) →
				</a>
			</div>
		</div>
	);
}
