import { FileOverlayHost } from './components/file-overlay-host/FileOverlayHost';
import type { FileOverlayModule } from './file-overlay-contract';

/** The root of the overlay's chunk: nothing outside this module reaches its components statically. */
export const fileOverlayModule: FileOverlayModule = { FileOverlayHost };
