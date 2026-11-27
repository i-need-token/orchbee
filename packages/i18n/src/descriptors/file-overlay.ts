import { defineMessages } from 'react-intl';

import { enUSFileOverlayMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSFileOverlayMessages);

/** The file overlay: the frame over the terminal area where files open in tabs. */
export const fileOverlayMessages = defineMessages({
	expand: message(
		'fileOverlay.actions.expand',
		'Label of the button at the end of the file overlay’s tab row that widens the overlay to the whole window.',
	),
	label: message(
		'fileOverlay.label',
		'Accessible name of the file overlay, the frame over the terminal area where files open in tabs.',
	),
	loadFailed: message(
		'fileOverlay.loadFailed',
		'Error card shown when the file overlay could not be loaded, so the file opened does not show.',
	),
	restore: message(
		'fileOverlay.actions.restore',
		'Label of the button at the end of the file overlay’s tab row that returns the widened overlay to its size before.',
	),
	retry: message(
		'fileOverlay.actions.retry',
		'Button on the error card that tries loading the file overlay again.',
	),
	toggleFull: message(
		'fileOverlay.actions.toggleFull',
		'Action that widens the file overlay to the whole window, or returns it to its size before.',
	),
});
