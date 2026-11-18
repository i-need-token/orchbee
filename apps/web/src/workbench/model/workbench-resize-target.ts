/**
 * Shared pointer hit-target sizes for splitter handles (fine `17px` = `1px` visual hairline +
 * `8px` `--ob-ref-space-2` expansion on each side so CSS cursor hover and pointer capture agree).
 */
export const WORKBENCH_RESIZE_TARGET_SIZE_PX = {
	coarse: 24,
	fine: 17,
} as const;
