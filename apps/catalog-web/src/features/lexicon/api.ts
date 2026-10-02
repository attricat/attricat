import { request } from '../../api/request';
import { LEXICON_ENTRIES_PATH, LEXICON_LANGUAGE_PARAM } from './constants';
import { lexiconEntriesSchema } from './schemas';

export const listLexiconEntries = (language: string) =>
  request(
    `${LEXICON_ENTRIES_PATH}?${new URLSearchParams({ [LEXICON_LANGUAGE_PARAM]: language })}`,
    lexiconEntriesSchema,
  );
