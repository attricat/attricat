import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { Attribute, BlueprintWithAttributes } from '../api';
import { getDefaultValues } from '../api';
import { getFormAssists, runFormAssist } from '../../extensions/api';
import {
  extensionOutlets,
  formAssistDebounceMs,
  formAssistOutcomes,
} from '../../extensions/constants';
import { extensionQueryKeys } from '../../extensions/queryKeys';
import type { FormAssist } from '../../extensions/schemas';
import { scalarValueForField, valueForField } from '../attributeValues';
import { attributeValueTypes } from '../valueTypes';
import {
  applySuggestions,
  defaultFromDependents,
  suggestionSources,
  withoutEdited,
  type FieldProposal,
  type FormSuggestions,
  type SuggestionSource,
} from '../formSuggestions';
import { relationshipIdsForField } from '../recordForm';
import { useRecordLabels } from '../useRecordLabels';

type Fields = Readonly<Record<string, string>>;

/**
 * Values the unsaved form filled in for the user, and the fields the user has
 * edited, which nothing fills again.
 */
export const useFormSuggestions = ({
  getFields,
  setFields,
}: {
  getFields: () => Fields;
  setFields: (fields: Record<string, string>) => void;
}) => {
  const [suggestions, setSuggestions] = useState<FormSuggestions>({});
  const current = useRef(suggestions);
  const edited = useRef(new Set<string>());
  const update = (next: FormSuggestions) => {
    current.current = next;
    setSuggestions(next);
  };
  return {
    suggestions,
    /** Fields the user has edited themselves. */
    edited: () => edited.current as ReadonlySet<string>,
    /** Applies `proposals` for the fields in `scope`; see `applySuggestions`. */
    offer: (
      source: SuggestionSource,
      scope: readonly string[],
      proposals: readonly FieldProposal[],
    ) => {
      const result = applySuggestions({
        edited: edited.current,
        fields: getFields(),
        proposals,
        scope,
        source,
        suggestions: current.current,
      });
      if (!result) return;
      update(result.suggestions);
      setFields(result.fields);
    },
    /** The user changed `code`, so it keeps whatever they entered. */
    markEdited: (code: string) => {
      edited.current.add(code);
      update(withoutEdited(current.current, [code]));
    },
    /** Forgets every suggestion and edit, for the next record. */
    reset: () => {
      edited.current = new Set();
      update({});
    },
  };
};

/**
 * Previews the blueprint's `default_from` values while a new record is
 * entered: choosing a relationship target fills the fields that default from
 * it, as saving would. The save applies the same rule on the server.
 */
export const useDefaultFromSuggestions = ({
  blueprint,
  contextId,
  enabled,
  getFields,
  offer,
}: {
  blueprint: BlueprintWithAttributes | undefined;
  contextId: string | null;
  enabled: boolean;
  getFields: () => Fields;
  offer: ReturnType<typeof useFormSuggestions>['offer'];
}) => {
  const pending = useRef(new Map<string, AbortController>());
  /** Call after the user changes relationship `attribute` to `value`. */
  return (attribute: Attribute, value: string) => {
    if (!enabled || !blueprint) return;
    const scope = defaultFromDependents(
      blueprint.attributes,
      attribute.code,
    ).map((dependent) => dependent.code);
    if (scope.length === 0) return;
    pending.current.get(attribute.code)?.abort();
    // The previous target's values stop applying now, not when the new
    // preview answers: saving in between, or after a failed preview, would
    // otherwise store them for the new target. The save applies the rule.
    offer(suggestionSources.defaultFrom, scope, []);
    const [target, ...others] = relationshipIdsForField(value);
    if (!target || others.length > 0) return;
    const controller = new AbortController();
    pending.current.set(attribute.code, controller);
    getDefaultValues(
      {
        blueprintId: blueprint.blueprint.id,
        blueprintVersion: blueprint.blueprint.version,
        contextId,
        relationship: attribute.code,
        targetRecordId: target,
      },
      controller.signal,
    )
      .then((response) => {
        // A later choice replaced this one.
        if (
          controller.signal.aborted ||
          relationshipIdsForField(getFields()[attribute.code] ?? '').join() !==
            target
        )
          return;
        offer(
          suggestionSources.defaultFrom,
          scope,
          response.values.map((item) => ({
            attributeCode: item.attribute_code,
            value: item.value,
            recordId: target,
          })),
        );
      })
      // The save still applies the defaults; the form just shows none.
      .catch(() => undefined)
      .finally(() => {
        if (pending.current.get(attribute.code) === controller)
          pending.current.delete(attribute.code);
      });
  };
};

/** An assist's key among the form's assists. */
const assistKey = (assist: FormAssist) => `${assist.extension_id}:${assist.id}`;

/**
 * Runs the blueprint's extension form assists while a new record is entered:
 * a change to a watched field calls the assist once the user pauses, and its
 * proposals fill the proposed fields the user has not edited. Changing a
 * watched field withdraws the assist's earlier proposals; an answer to values
 * that changed meanwhile, a failure and a timeout propose nothing. Only the
 * form calls assists; saving does not.
 */
export const useFormAssistSuggestions = ({
  blueprint,
  contextId,
  enabled,
  getFields,
  suggestions,
}: {
  blueprint: BlueprintWithAttributes | undefined;
  contextId: string | null;
  enabled: boolean;
  getFields: () => Fields;
  suggestions: Pick<ReturnType<typeof useFormSuggestions>, 'edited' | 'offer'>;
}) => {
  const { t } = useTranslation();
  const scope = blueprint && {
    blueprintId: blueprint.blueprint.id,
    blueprintVersion: blueprint.blueprint.version,
  };
  const { data } = useQuery({
    queryKey: extensionQueryKeys.formAssists(
      scope ?? { blueprintId: '', blueprintVersion: 0 },
    ),
    queryFn: ({ signal }) => getFormAssists(scope!, signal),
    enabled: enabled && scope !== undefined,
  });
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const pending = useRef(new Map<string, AbortController>());
  useEffect(() => {
    const scheduled = timers.current;
    const running = pending.current;
    return () => {
      scheduled.forEach(clearTimeout);
      running.forEach((controller) => controller.abort());
    };
  }, []);
  const attributes = new Map(
    (blueprint?.attributes ?? []).map((attribute) => [
      attribute.code,
      attribute,
    ]),
  );
  /** The watched fields' values, to tell whether an answer is stale. */
  const watched = (assist: FormAssist) =>
    JSON.stringify(assist.watch.map((code) => getFields()[code] ?? ''));
  const run = (assist: FormAssist) => {
    if (!blueprint) return;
    const key = assistKey(assist);
    pending.current.get(key)?.abort();
    const controller = new AbortController();
    pending.current.set(key, controller);
    const fields = getFields();
    const values: Record<string, unknown> = {};
    const relationships: Record<string, string[]> = {};
    for (const code of new Set([...assist.watch, ...assist.proposes])) {
      const attribute = attributes.get(code);
      if (!attribute) continue;
      if (attribute.value_type === attributeValueTypes.relationship)
        relationships[code] = relationshipIdsForField(fields[code] ?? '');
      else {
        const scalar = scalarValueForField(attribute, fields[code] ?? '');
        if (scalar) values[code] = scalar.value;
      }
    }
    const edited = suggestions.edited();
    const asked = watched(assist);
    runFormAssist(
      {
        extension_id: assist.extension_id,
        id: assist.id,
        blueprint_id: blueprint.blueprint.id,
        blueprint_version: blueprint.blueprint.version,
        record_id: null,
        context_id: contextId,
        values,
        relationships,
        edited: assist.proposes.filter((code) => edited.has(code)),
      },
      controller.signal,
    )
      .then((answer) => {
        if (
          controller.signal.aborted ||
          answer.outcome !== formAssistOutcomes.answered ||
          watched(assist) !== asked
        )
          return;
        suggestions.offer(
          suggestionSources.assist,
          assist.proposes,
          answer.proposals.flatMap((proposal) => {
            const attribute = attributes.get(proposal.attribute_code);
            // The host checked the value; the form checks it as typed too.
            return attribute &&
              scalarValueForField(attribute, valueForField(proposal.value))
              ? [
                  {
                    attributeCode: proposal.attribute_code,
                    value: proposal.value,
                    note:
                      proposal.note ??
                      t('records.suggestedByNamedExtension', {
                        extension: assist.extension_name,
                      }),
                  },
                ]
              : [];
          }),
        );
      })
      // An assist never stands in the way of entering the record.
      .catch(() => undefined)
      .finally(() => {
        if (pending.current.get(key) === controller)
          pending.current.delete(key);
      });
  };
  /** Call after the user changes field `code`. */
  return (code: string) => {
    if (!enabled) return;
    const watching = (data?.assists ?? []).filter(
      (assist) =>
        assist.outlet === extensionOutlets.recordForm &&
        assist.watch.includes(code),
    );
    for (const assist of watching) {
      const key = assistKey(assist);
      // Proposals for the old values no longer hold, even if the next call
      // fails or times out, so they are withdrawn at once.
      suggestions.offer(suggestionSources.assist, assist.proposes, []);
      clearTimeout(timers.current.get(key));
      timers.current.set(
        key,
        setTimeout(() => {
          timers.current.delete(key);
          run(assist);
        }, formAssistDebounceMs),
      );
    }
  };
};

/** The helper text of each suggested field: where its value came from. */
export const useSuggestionNotes = (
  suggestions: FormSuggestions,
): Readonly<Record<string, string>> => {
  const { t } = useTranslation();
  const recordIds = Object.values(suggestions).flatMap((suggestion) =>
    suggestion.recordId ? [suggestion.recordId] : [],
  );
  const labels = useRecordLabels(recordIds);
  return useMemo(
    () =>
      Object.fromEntries(
        Object.entries(suggestions).map(([code, suggestion]) => [
          code,
          suggestion.source === suggestionSources.assist
            ? (suggestion.note ?? t('records.suggestedByExtension'))
            : t('records.suggestedFromRecord', {
                record:
                  (suggestion.recordId && labels.get(suggestion.recordId)) ||
                  t('records.relatedRecord'),
              }),
        ]),
      ),
    [labels, suggestions, t],
  );
};
