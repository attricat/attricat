import { useState } from 'react';
import { checkViolationError } from '../../../api/checkViolations';
import { useViolationText } from '../../../components/useViolationText';
import { violationFieldErrors } from '../checkViolations';

/**
 * Places a failed save's check violations on the fields they name. A field's
 * violation is cleared once the user edits that field after the failure;
 * violations that name no field are returned as `unplaced`.
 */
export const useServerFieldErrors = (
  error: Error | null | undefined,
  fieldCodes: readonly string[],
) => {
  const violationText = useViolationText();
  const checks = checkViolationError(error);
  const [editedAfterError, setEditedAfterError] = useState<{
    error: unknown;
    codes: readonly string[];
  }>({ error: undefined, codes: [] });
  const editedCodes =
    editedAfterError.error === error ? editedAfterError.codes : [];
  const violations = checks
    ? violationFieldErrors(checks.violations, fieldCodes, violationText)
    : undefined;
  const fieldErrors = Object.fromEntries(
    Object.entries(violations?.fieldErrors ?? {}).filter(
      ([code]) => !editedCodes.includes(code),
    ),
  );
  const markEdited = (code: string) => {
    if (!checks || editedCodes.includes(code)) return;
    setEditedAfterError((current) => ({
      error,
      codes: current.error === error ? [...current.codes, code] : [code],
    }));
  };
  return { fieldErrors, markEdited, unplaced: violations?.unplaced };
};
