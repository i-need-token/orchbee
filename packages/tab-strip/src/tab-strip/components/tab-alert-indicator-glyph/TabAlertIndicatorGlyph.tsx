import type { ReactNode } from 'react';
import type { TabAlertState } from '../../../model';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';

export interface TabAlertIndicatorGlyphProps {
	state: TabAlertState;
}

export function TabAlertIndicatorGlyph({ state }: TabAlertIndicatorGlyphProps): ReactNode {
	if (state === 'audio-playing') return <TabStripIcon name='volume' />;
	if (state === 'audio-muting' || state === 'audio-muted') return <TabStripIcon name='muted' />;
	if (state === 'media-recording' || state === 'recording')
		return <TabStripIcon name='media-recording' />;
	if (state === 'audio-recording') return <TabStripIcon name='microphone' />;
	if (state === 'video-recording') return <TabStripIcon name='camera' />;
	if (state === 'desktop-capturing') return <TabStripIcon name='desktop-capture' />;
	if (state === 'tab-capturing' || state === 'capturing')
		return <TabStripIcon name='tab-capture' />;
	if (state === 'pip-playing') return <TabStripIcon name='picture-in-picture' />;
	if (state === 'bluetooth-connected' || state === 'bluetooth-scan-active')
		return <TabStripIcon name='bluetooth' />;
	if (state === 'usb-connected') return <TabStripIcon name='usb' />;
	if (state === 'hid-connected' || state === 'serial-connected')
		return <TabStripIcon name='device' />;
	if (state === 'vr-presenting-in-headset') return <TabStripIcon name='vr' />;
	return null;
}
