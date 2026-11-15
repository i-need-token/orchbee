import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { pagePath } from '../../pages';
import { LinkButton } from '../link-button/LinkButton';
import { PageHead } from '../page-head/PageHead';

// The page served for an address the site does not have.
export function NotFoundPage({ locale }: { locale: OrchbeeLocale }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<PageHead
			address='404'
			title={t(messages.notFoundTitle)}
			lead={t(messages.notFoundBody)}
			action={<LinkButton href={pagePath('home', locale)}>{t(messages.notFoundHome)}</LinkButton>}
		/>
	);
}
