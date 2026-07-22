import type { ReactNode } from 'react';
import {
	TabStripImplementation,
	type TabStripImplementationCrossWindowDragRuntime,
	type TabStripImplementationOptions,
} from './components/tab-strip-implementation/tab-strip-implementation';
export type TabStripCrossWindowDragRuntime = TabStripImplementationCrossWindowDragRuntime;
export type TabStripProps = TabStripImplementationOptions;
export function TabStrip(props: TabStripProps): ReactNode {
	return <TabStripImplementation {...props} />;
}
