import { parse } from 'smol-toml';
import { TOML_FILE_EXTENSION } from './constants';
import { lexiconFileSchema, type LexiconFile } from './schemas';

/** Reads an import file as JSON, or as TOML when its name ends in `.toml`. */
export const parseLexiconFile = (name: string, source: string): LexiconFile =>
  lexiconFileSchema.parse(
    name.toLowerCase().endsWith(TOML_FILE_EXTENSION)
      ? parse(source)
      : JSON.parse(source),
  );

/** Saves an export as `lexicon-<language>.json`. */
export const downloadLexiconFile = (file: LexiconFile) => {
  const url = URL.createObjectURL(
    new Blob([`${JSON.stringify(file, null, 2)}\n`], {
      type: 'application/json',
    }),
  );
  const link = document.createElement('a');
  link.href = url;
  link.download = `lexicon-${file.language}.json`;
  link.click();
  URL.revokeObjectURL(url);
};
