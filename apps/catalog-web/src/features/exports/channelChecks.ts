/** Normalizes entered rule codes into the unique list the API accepts. */
export const requiredRuleCodesFromInput = (values: readonly string[]) => [
  ...new Set(values.map((value) => value.trim()).filter(Boolean)),
];
