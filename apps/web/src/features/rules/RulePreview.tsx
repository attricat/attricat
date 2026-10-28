import { Box, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { CompiledRule } from './api';
import { ruleTriggerTypes } from './constants';
import { FindingSeverityChip } from './FindingSeverityChip';
import type { RuleTrigger } from './schemas';
import { RuleDescriptionText } from './RuleCells';

const TriggerText = ({ trigger }: { trigger: RuleTrigger }) => {
  const { t } = useTranslation();
  switch (trigger.type) {
    case ruleTriggerTypes.manual:
      return t('rules.preview.triggers.manual');
    case ruleTriggerTypes.schedule:
      return t('rules.preview.triggers.schedule', { cron: trigger.cron });
    case ruleTriggerTypes.event:
      return t('rules.preview.triggers.event', { event: trigger.event_type });
    case ruleTriggerTypes.postImport:
      return t('rules.preview.triggers.postImport');
  }
};

/** What a rule definition that passed the save checks will do. */
export const RulePreview = ({ rule }: { rule: CompiledRule }) => {
  const { t } = useTranslation();
  const attribute = rule.predicate.attribute_code;
  // A predicate type added on the server before the app learns it keeps its code.
  const type = t(`rules.preview.predicateTypes.${rule.predicate.type}`, {
    defaultValue: rule.predicate.type,
  });
  const transitions = rule.enforcement?.transitions ?? [];
  return (
    <Stack spacing={2} sx={{ mt: 1 }}>
      <Box>
        <Stack
          direction="row"
          spacing={1}
          sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        >
          <Typography component="h3" variant="h6">
            {rule.name}
          </Typography>
          <FindingSeverityChip severity={rule.severity} />
        </Stack>
        <Typography color="text.secondary" variant="caption">
          {rule.code}
        </Typography>
        <RuleDescriptionText description={rule.description} />
      </Box>
      <Box>
        <Typography component="h4" variant="subtitle2">
          {t('rules.preview.checks')}
        </Typography>
        <Typography variant="body2">
          {typeof attribute === 'string'
            ? t('rules.preview.predicateOn', { attribute, type })
            : t('rules.preview.predicate', { type })}
        </Typography>
      </Box>
      <Box>
        <Typography component="h4" variant="subtitle2">
          {t('rules.preview.runs')}
        </Typography>
        <Box component="ul" sx={{ m: 0, pl: 3 }}>
          {rule.triggers.map((trigger, index) => (
            <Typography component="li" key={index} variant="body2">
              <TriggerText trigger={trigger} />
            </Typography>
          ))}
        </Box>
      </Box>
      <Box>
        <Typography component="h4" variant="subtitle2">
          {t('rules.preview.enforcement')}
        </Typography>
        {rule.enforcement ? (
          <Box component="ul" sx={{ m: 0, pl: 3 }}>
            {rule.enforcement.on_save && (
              <Typography component="li" variant="body2">
                {t('rules.preview.rejectsSaves')}
              </Typography>
            )}
            {transitions.map((transition, index) => (
              <Typography component="li" key={index} variant="body2">
                {transition.from
                  ? t('rules.preview.blocksTransition', {
                      attribute: transition.attribute_code,
                      from: transition.from,
                      to: transition.to,
                    })
                  : t('rules.preview.blocksTransitionInto', {
                      attribute: transition.attribute_code,
                      to: transition.to,
                    })}
              </Typography>
            ))}
          </Box>
        ) : (
          <Typography color="text.secondary" variant="body2">
            {t('rules.preview.reportsOnly')}
          </Typography>
        )}
      </Box>
    </Stack>
  );
};
