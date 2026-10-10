import i18n, { type BackendModule, type TOptions } from 'i18next';
import { useEffect, useSyncExternalStore } from 'react';
import { listLexiconEntries } from './api';
import { LEXICON_MANAGEMENT_NAMESPACE, LEXICON_NAMESPACE } from './constants';
import {
  isLexiconReference,
  parseLexiconText,
  type LexiconReference,
} from './references';
import {
  lexiconResourceKey,
  lexiconResources,
  pluralResourceKey,
} from './resources';

// Keys are English text that may contain `.` or `:`, and translations are
// shown literally, so `{{` in a translation is never interpolated.
const lookupOptions = {
  ns: LEXICON_NAMESPACE,
  keySeparator: false,
  nsSeparator: false,
  skipInterpolation: true,
} as const;

const translate = (reference: LexiconReference, count?: number) =>
  i18n.t(lexiconResourceKey(reference), {
    ...lookupOptions,
    defaultValue: reference.key,
    ...(count === undefined ? {} : { count }),
  } as TOptions);

/**
 * Renders attricat-defined text for the UI language. Each `{{key}}` resolves
 * from the lexicon for the user's language, then English, then the key
 * itself; other text is literal. Malformed text renders as authored.
 */
export const lexiconText = (text: string): string => {
  const parsed = parseLexiconText(text);
  if ('error' in parsed) return text;
  return parsed.segments
    .map((segment) =>
      isLexiconReference(segment) ? translate(segment) : segment.literal,
    )
    .join('');
};

/**
 * The plural noun for `count` when `text` is exactly one reference whose
 * lexicon entry has a form for that count in the user's language or English.
 * Otherwise `undefined`, so callers keep their generic count wording rather
 * than pairing a number with a singular label.
 */
export const lexiconCountNoun = (
  text: string,
  count: number,
): string | undefined => {
  const parsed = parseLexiconText(text);
  if ('error' in parsed || parsed.segments.length !== 1) return undefined;
  const [reference] = parsed.segments;
  if (!isLexiconReference(reference)) return undefined;
  const resourceKey = lexiconResourceKey(reference);
  const hasForm = i18n.languages.some(
    (language) =>
      i18n.getResource(
        language,
        LEXICON_NAMESPACE,
        pluralResourceKey(
          resourceKey,
          new Intl.PluralRules(language).select(count),
        ),
        lookupOptions,
      ) !== undefined,
  );
  return hasForm ? translate(reference, count) : undefined;
};

let lexiconRevision = 0;
const revisionListeners = new Set<() => void>();
const bumpLexiconRevision = () => {
  lexiconRevision += 1;
  revisionListeners.forEach((listener) => listener());
};
i18n.on('loaded', bumpLexiconRevision);
i18n.on('languageChanged', bumpLexiconRevision);

/**
 * Changes whenever resolved lexicon text may change. Add it to the
 * dependencies of memoized values derived from {@link lexiconText}.
 */
export const useLexiconRevision = () =>
  useSyncExternalStore(
    (listener) => {
      revisionListeners.add(listener);
      return () => revisionListeners.delete(listener);
    },
    () => lexiconRevision,
  );

/** Namespaces served by {@link lexiconBackend} rather than the app locales. */
export const isLexiconNamespace = (namespace: string) =>
  namespace === LEXICON_NAMESPACE || namespace === LEXICON_MANAGEMENT_NAMESPACE;

let lexiconWorkspaceId: string | null = null;

/**
 * Loads the lexicon namespace from the signed-in workspace, and the lexicon
 * management page's own strings. Each read
 * replaces the language's bundle so deleted entries disappear on reload.
 */
export const lexiconBackend: BackendModule = {
  type: 'backend',
  init: () => undefined,
  read: (language, namespace, callback) => {
    if (namespace === LEXICON_MANAGEMENT_NAMESPACE) {
      // Kept out of the startup bundle; only the management page uses them.
      import(`./locales/${language}.json`).then(
        (strings: { default: Record<string, unknown> }) =>
          callback(null, strings.default),
        () => callback(null, {}),
      );
      return;
    }
    const workspaceId = lexiconWorkspaceId;
    if (namespace !== LEXICON_NAMESPACE) {
      callback(null, {});
      return;
    }
    if (!workspaceId) {
      // Signed out: drop the previous workspace's translations.
      i18n.removeResourceBundle(language, namespace);
      callback(null, {});
      return;
    }
    listLexiconEntries(language).then(
      (entries) => {
        if (workspaceId !== lexiconWorkspaceId) {
          // A newer workspace load is in flight; keep the current bundle.
          callback(null, i18n.getResourceBundle(language, namespace) ?? {});
          return;
        }
        i18n.removeResourceBundle(language, namespace);
        callback(null, lexiconResources(entries));
      },
      // Missing translations fall back to keys, so a failed load must not
      // block rendering or trigger i18next's retries.
      () => callback(null, {}),
    );
  },
};

const loadLexicon = (workspaceId: string | null) => {
  if (workspaceId === lexiconWorkspaceId) return;
  lexiconWorkspaceId = workspaceId;
  void (i18n.options.ns?.includes(LEXICON_NAMESPACE)
    ? i18n.reloadResources(undefined, [LEXICON_NAMESPACE])
    : i18n.loadNamespaces(LEXICON_NAMESPACE));
};

/** Reloads translations after they change, so labels update in place. */
export const reloadLexicon = () =>
  i18n.reloadResources(undefined, [LEXICON_NAMESPACE]);

/** Keeps the lexicon namespace in step with the signed-in workspace. */
export const useWorkspaceLexicon = (workspaceId: string | undefined) => {
  useEffect(() => loadLexicon(workspaceId ?? null), [workspaceId]);
};
