import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';

import { ORCHBEE_TERMINAL_FONT_FAMILY } from './terminal-product-fonts';

/** Stable application-owned source observed by an existing projected-terminal attachment. */
export interface TerminalPresentationConfigurationSource {
	current(): TerminalPresentationConfiguration;
	subscribe(listener: (configuration: TerminalPresentationConfiguration) => void): () => void;
}

const productTerminalPresentationConfiguration: TerminalPresentationConfiguration = Object.freeze({
	...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	fontFamily: ORCHBEE_TERMINAL_FONT_FAMILY,
	fontSize: 13,
});

export const DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE: TerminalPresentationConfigurationSource =
	Object.freeze({
		current: () => productTerminalPresentationConfiguration,
		subscribe: () => () => undefined,
	});
