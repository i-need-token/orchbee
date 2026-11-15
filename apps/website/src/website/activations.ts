import type { Activation } from './activation';
import { cliPageActivations } from './components/cli-page/activations';
import { activateCopyButtons } from './components/copy-button/activateCopyButtons';
import { desktopPageActivations } from './components/desktop-page/activations';
import { homePageActivations } from './components/home-page/activations';
import { activateLattice } from './components/lattice/activateLattice';
import { activateLanguageLinks } from './language-link/activateLanguageLinks';
import { activateNumberedMarks } from './components/numbered-marks/activateNumberedMarks';
import { activateReveal } from './components/page-shell/activateReveal';
import { activatePanelPlay } from './components/panel-play/activatePanelPlay';
import { activateSessionScene } from './components/session-scene/activateSessionScene';
import { activateSiteHeader } from './components/site-header/activateSiteHeader';
import { terminalPageActivations } from './components/terminal-page/activations';

// Everything the static pages need from script, in the order it runs. The page chrome comes first;
// each page owns its own list, so a section added to one page never edits this file.
export const activations: readonly Activation[] = [
	activateSiteHeader,
	activateLanguageLinks,
	activateLattice,
	activateReveal,
	activateCopyButtons,
	activateSessionScene,
	activatePanelPlay,
	activateNumberedMarks,
	...homePageActivations,
	...terminalPageActivations,
	...desktopPageActivations,
	...cliPageActivations,
];
