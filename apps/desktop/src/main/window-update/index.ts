export {
	DesktopWindowUpdate,
	type DesktopWindowUpdateFocus,
	type DesktopWindowUpdatePort,
} from './DesktopWindowUpdate';
export { readWindowPointer } from './readWindowPointer';
export { watchWindowPointer } from './watchWindowPointer';
export {
	locateWindowDesktopRoot,
	readFallbackFromVersion,
	resolvePosixReleaseStoreRoot,
	windowVersionResources,
} from './window-update-offer';
export {
	givesWayToReplacedVersion,
	readRelaunchPlace,
	relaunchWindow,
	takeRelaunchAnnouncement,
} from './window-relaunch';
export { discardSetupLaunch, takeSetupLaunch } from './setup-launch';
