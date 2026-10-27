import { SAMPLE_SYSTEM_TAG } from './constants';

/** The tags shown with a record; the sample marker shows as its badge instead. */
export const visibleSystemTags = (tags: readonly string[]) =>
  tags.filter((tag) => tag !== SAMPLE_SYSTEM_TAG);
