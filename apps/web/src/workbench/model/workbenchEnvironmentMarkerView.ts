import { type IntlShape, workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchEnvironmentMarker } from '../types/workbench-environment-marker';

/**
 * The sidebar marker's copy: the environment's word, the checkout name, and the tooltip that names
 * the build and its checkout in full, which only a build from a checkout has.
 */
export function workbenchEnvironmentMarkerView(
	marker: WorkbenchEnvironmentMarker,
	intl: IntlShape,
): {
	readonly checkout: string | null;
	readonly description: string | null;
	readonly label: string;
} {
	const development = marker.environment === 'development';
	const checkout = marker.checkoutName;
	return {
		checkout,
		description:
			checkout === null
				? null
				: intl.formatMessage(
						development
							? workbenchMessages.environmentDevelopmentCheckout
							: workbenchMessages.environmentTestCheckout,
						{ checkout },
					),
		label: intl.formatMessage(
			development ? workbenchMessages.environmentDevelopment : workbenchMessages.environmentTest,
		),
	};
}
