import { parseStrictYaml } from '../../components/yamlSyntax';
import { YAML_FILE_EXTENSIONS } from './constants';
import { lexiconFileSchema, type LexiconFile } from './schemas';
import { downloadUrl } from '../../components/downloadUrl';
import { formatJson } from '../../components/jsonTokens';

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
    new Blob([`${formatJson(file)}\n`], {
      type: 'application/json',
    }),
  );
  downloadUrl(url, `lexicon-${file.language}.json`);
  URL.revokeObjectURL(url);
};
