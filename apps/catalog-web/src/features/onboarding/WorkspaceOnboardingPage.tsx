import {
  Box,
  Button,
  Link,
  Paper,
  Stack,
  Typography,
  useTheme,
} from '@mui/material';
import { ExternalLinkIcon } from 'lucide-react';
import { createElement } from 'react';
import { useTranslation } from 'react-i18next';
import { useDocumentationUrl } from '../../app/documentation';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { DocumentationIcon } from '../../components/systemIcons';
import {
  getVisibleOnboardingSteps,
  type OnboardingCapabilities,
} from './onboardingSteps';

const externalLinkProps = {
  rel: 'noopener noreferrer',
  target: '_blank',
} as const;

export const WorkspaceOnboardingPage = ({
  capabilities,
}: {
  capabilities?: OnboardingCapabilities;
}) => {
  const { t } = useTranslation();
  const theme = useTheme();
  const documentationUrl = useDocumentationUrl();
  const steps = getVisibleOnboardingSteps(capabilities);
  const documentationButton = (
    <Button
      {...externalLinkProps}
      endIcon={<ExternalLinkIcon />}
      href={documentationUrl('home')}
      startIcon={<DocumentationIcon />}
      sx={{ whiteSpace: 'nowrap' }}
      variant="outlined"
    >
      {t('onboarding.readDocumentation')}
    </Button>
  );

  if (!steps.length)
    return (
      <PageContainer maxWidth="md">
        <PageHeader
          actions={documentationButton}
          description={t('onboarding.emptyDescription')}
          title={t('onboarding.emptyTitle')}
        />
      </PageContainer>
    );

  return (
    <PageContainer>
      <PageHeader
        actions={documentationButton}
        description={t('onboarding.description')}
        eyebrow={t('onboarding.eyebrow')}
        title={t('onboarding.title')}
      />
      <Box
        aria-label={t('onboarding.stepsLabel')}
        component="ol"
        sx={{
          display: 'grid',
          gap: 2,
          gridTemplateColumns: { sm: 'repeat(2, minmax(0, 1fr))' },
          listStyle: 'none',
          mb: 0,
          mt: 3,
          p: 0,
        }}
      >
        {steps.map((step, index) => (
          <Paper component="li" key={step.to} sx={{ display: 'flex', p: 3 }}>
            <Stack spacing={1.5} sx={{ flexGrow: 1 }}>
              <Stack
                direction="row"
                spacing={1.5}
                sx={{ alignItems: 'center' }}
              >
                {createElement(step.icon, {
                  color: theme.palette.primary.main,
                })}
                <Typography color="text.secondary" variant="overline">
                  {t('onboarding.stepNumber', { number: index + 1 })}
                </Typography>
              </Stack>
              <Typography component="h2" variant="h6">
                {t(step.titleKey)}
              </Typography>
              <Typography color="text.secondary" sx={{ flexGrow: 1 }}>
                {t(step.descriptionKey)}
              </Typography>
              <Stack
                direction="row"
                spacing={2}
                sx={{ alignItems: 'center', flexWrap: 'wrap', rowGap: 1 }}
              >
                <RouterButton
                  to={step.to}
                  variant={index === 0 ? 'contained' : 'outlined'}
                >
                  {t(step.actionKey)}
                </RouterButton>
                <Link
                  {...externalLinkProps}
                  aria-label={t('onboarding.readGuideAbout', {
                    topic: t(step.titleKey),
                  })}
                  href={documentationUrl(step.documentationPage)}
                >
                  {t('onboarding.readGuide')}
                </Link>
              </Stack>
            </Stack>
          </Paper>
        ))}
      </Box>
    </PageContainer>
  );
};
