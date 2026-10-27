import i18n from 'i18next';
import { lexiconKeyText, lexiconText } from '../lexicon/lexicon';
import { NEEDS_ATTENTION_SYSTEM_TAG, SAMPLE_SYSTEM_TAG } from './constants';

/** The tags shown with a record; the sample marker shows as its badge instead. */
export const visibleSystemTags = (tags: readonly string[]) =>
  tags.filter((tag) => tag !== SAMPLE_SYSTEM_TAG);

/** App translations of the Core tags Attricat sets on records. */
const coreSystemTagLabelKeys: Readonly<Record<string, string>> = {
  [NEEDS_ATTENTION_SYSTEM_TAG]: 'records.coreSystemTags.needsAttention',
};

/**
 * A system tag for display. A tag with `{{}}` renders as lexicon text;
 * workflows and extensions rarely write those, so any other tag is itself
 * looked up as a lexicon key. A Core tag without a workspace entry uses the
 * app's own translation. Filters and links keep the stored tag.
 */
export const systemTagLabel = (tag: string) => {
  if (tag.includes('{{')) return lexiconText(tag);
  const label = lexiconKeyText(tag);
  const coreKey = Object.hasOwn(coreSystemTagLabelKeys, tag)
    ? coreSystemTagLabelKeys[tag]
    : undefined;
  return label === tag && coreKey ? i18n.t(coreKey) : label;
};
