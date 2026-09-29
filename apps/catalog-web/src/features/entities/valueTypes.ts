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

export const attributeValueKinds = {
  scalar: 'scalar',
  relationship: 'relationship',
  file: 'file',
} as const;
