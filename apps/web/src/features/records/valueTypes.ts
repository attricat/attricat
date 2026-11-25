export const attributeValueTypes = {
  string: 'string',
  number: 'number',
  integer: 'integer',
  boolean: 'boolean',
  date: 'date',
  datetime: 'datetime',
  time: 'time',
  json: 'json',
  file: 'file',
  relationship: 'relationship',
} as const;

/** Whether a value type holds numbers: `number` or `integer`. */
export const isNumericValueType = (valueType: string | undefined) =>
  valueType === attributeValueTypes.number ||
  valueType === attributeValueTypes.integer;

export const attributeValueKinds = {
  scalar: 'scalar',
  relationship: 'relationship',
  file: 'file',
} as const;
