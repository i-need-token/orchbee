import type { Activation } from './activation';
import { cliPageActivations } from './components/cli-page/activations';
import { activateCopyButtons } from './components/copy-button/activateCopyButtons';
import { activateDownloadDialog } from './components/download-dialog/activateDownloadDialog';
import { desktopPageActivations } from './components/desktop-page/activations';
import { activateGetOrchbee } from './components/get-orchbee/activateGetOrchbee';
import { homePageActivations } from './components/home-page/activations';
import { activateLattice } from './components/lattice/activateLattice';
import { activateLanguageLinks } from './language-link/activateLanguageLinks';
import { activateNumberedMarks } from './components/numbered-mark/activateNumberedMarks';
import { activateReveal } from './components/page-shell/activateReveal';
import { activatePanelPlay } from './components/panel-play/activatePanelPlay';
import { activateSessionScene } from './components/session-scene/activateSessionScene';
import { activateSiteHeader } from './components/site-header/activateSiteHeader';
import { terminalPageActivations } from './components/terminal-page/activations';

// Everything the static pages need from script, in the order it runs. The page chrome comes first;
// each page owns its own list, so a section added to one page never edits this file.
export const activations: readonly Activation[] = [
	activateSiteHeader,
	activateDownloadDialog,
	activateLanguageLinks,
	activateLattice,
	activateReveal,
	activateCopyButtons,
	activateGetOrchbee,
	activateSessionScene,
	activatePanelPlay,
	activateNumberedMarks,
	...homePageActivations,
	...terminalPageActivations,
	...desktopPageActivations,
	...cliPageActivations,
];
