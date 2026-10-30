import { parseStrictYaml } from '../../components/yamlSyntax';
import { YAML_FILE_EXTENSIONS } from './constants';
import { lexiconFileSchema, type LexiconFile } from './schemas';

/** Throws on invalid YAML, as `JSON.parse` does on invalid JSON. */
const parseYaml = (source: string) => {
  const parsed = parseStrictYaml(source);
  if ('error' in parsed) throw new SyntaxError();
  return parsed.data;
};

/** Reads an import file as JSON, or as YAML when its name ends in `.yaml` or `.yml`. */
export const parseLexiconFile = (name: string, source: string): LexiconFile =>
  lexiconFileSchema.parse(
    YAML_FILE_EXTENSIONS.some((extension) =>
      name.toLowerCase().endsWith(extension),
    )
      ? parseYaml(source)
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
