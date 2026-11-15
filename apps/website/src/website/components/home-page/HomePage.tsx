import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { Release } from '../../release';
import { Agents } from './components/agents/Agents';
import { Clients } from './components/clients/Clients';
import { Collaboration } from './components/collaboration/Collaboration';
import { Hero } from './components/hero/Hero';
import { Install } from './components/install/Install';
import { Multiplexing } from './components/multiplexing/Multiplexing';
import { RemoteMachines } from './components/remote-machines/RemoteMachines';
import { Statement } from './components/statement/Statement';

// The home page, section by section. Each section is a private component under `components/`.
export function HomePage({
	locale,
	release,
}: {
	locale: OrchbeeLocale;
	release: Release;
}): ReactElement {
	return (
		<>
			<Hero release={release} />
			<Statement />
			<Multiplexing />
			<Collaboration />
			<Agents />
			<RemoteMachines />
			<Clients locale={locale} />
			<Install release={release} />
		</>
	);
}
