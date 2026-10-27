// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { RetryResultAlerts } from './RetryResultAlerts';
import type { useRetryExports } from './useRetryExports';

const jobId = '00000000-0000-4000-8000-000000000002';

const retried = {
  error: null,
  data: {
    record_ids: [],
    job_errors: [
      {
        job_id: jobId,
        code: 'export_job_resync_failed',
        message: 'blueprint is no longer published',
        details: { job_id: jobId, job_code: 'nightly' },
      },
    ],
  },
} as unknown as ReturnType<typeof useRetryExports>;

describe('RetryResultAlerts', () => {
  afterEach(() => i18n.changeLanguage('en'));

  it('translates the jobs a retry could not resync by their code', async () => {
    await i18n.changeLanguage('pl');
    render(<RetryResultAlerts retry={retried} />);
    expect(screen.getByRole('alert').textContent).toBe(
      'Niektóre zadania eksportu nie mogły ponownie zsynchronizować rekordów: Zadanie eksportu nightly nie mogło uruchomić ponownej synchronizacji.',
    );
  });
});
