import type { TFunction } from 'i18next';
import { defaultContextCode } from './constants';

/** The id of the default context among `contexts`, or `null` while unknown. */
export const findDefaultContextId = (
  contexts: readonly { code: string; id: string }[] | undefined,
) =>
  contexts?.find((context) => context.code === defaultContextCode)?.id ?? null;

/** A context's label: its code, or a translated name for the default one. */
export const contextLabel = (t: TFunction, code: string) =>
  code === defaultContextCode ? t('contexts.defaultContext') : code;
