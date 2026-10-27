import { lexiconKeyText, lexiconText } from '../lexicon/lexicon';
import { SAMPLE_SYSTEM_TAG } from './constants';

/** The tags shown with a record; the sample marker shows as its badge instead. */
export const visibleSystemTags = (tags: readonly string[]) =>
  tags.filter((tag) => tag !== SAMPLE_SYSTEM_TAG);

/**
 * A system tag for display. A tag with `{{}}` renders as lexicon text;
 * workflows and extensions rarely write those, so any other tag is itself
 * looked up as a lexicon key. Filters and links keep the stored tag.
 */
export const systemTagLabel = (tag: string) =>
  tag.includes('{{') ? lexiconText(tag) : lexiconKeyText(tag);
