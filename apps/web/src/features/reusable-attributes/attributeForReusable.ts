import type { Attribute } from '../records/api';
import type { ReusableAttribute } from './api';

/** The record attribute a reusable attribute becomes once attached. */
export const attributeForReusable = (
  reusable: ReusableAttribute,
): Attribute => ({
  cardinality: reusable.cardinality,
  code: reusable.code,
  context_editable: reusable.context_editable,
  context_fallback: reusable.context_fallback,
  file_policy: null,
  name: reusable.name,
  readonly: reusable.readonly,
  tags: reusable.tags,
  target_blueprint_code: reusable.target_blueprint_code,
  target_cardinality: reusable.target_cardinality,
  value_schema:
    reusable.value_schema === null
      ? null
      : (reusable.value_schema as Attribute['value_schema']),
  value_type: reusable.value_type,
});
