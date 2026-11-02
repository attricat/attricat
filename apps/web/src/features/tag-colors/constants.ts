import { millisecondsPerMinute } from '../../time/durations';
/** Colors change rarely and the editor refreshes them on save. */
export const tagColorsStaleTimeMs = 5 * millisecondsPerMinute;

/**
 * The most tag colors a workspace may set, as the API allows. A changed color
 * of a default tag counts too.
 */
export const MAX_TAG_COLORS = 200;

/** The diameter of a color swatch beside a tag or color name. */
export const tagColorDotSize = 10;
