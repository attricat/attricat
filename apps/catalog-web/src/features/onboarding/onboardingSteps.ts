import type { DocumentationPage } from '../../app/documentation';
import {
  BlueprintIcon,
  ContextIcon,
  ExtensionIcon,
  WorkspaceIcon,
} from '../../components/systemIcons';

export type OnboardingCapabilities = {
  blueprints_write?: boolean;
  extensions_read?: boolean;
  members_manage?: boolean;
};

type OnboardingStep = {
  actionKey: string;
  capability?: keyof OnboardingCapabilities;
  descriptionKey: string;
  documentationPage: DocumentationPage;
  icon: typeof BlueprintIcon;
  titleKey: string;
  to:
    | '/manage/blueprints/new'
    | '/manage/contexts'
    | '/manage/workspace/invitations'
    | '/manage/extensions';
};

export const onboardingSteps: readonly OnboardingStep[] = [
  {
    actionKey: 'onboarding.steps.blueprint.action',
    capability: 'blueprints_write',
    descriptionKey: 'onboarding.steps.blueprint.description',
    documentationPage: 'blueprints',
    icon: BlueprintIcon,
    titleKey: 'onboarding.steps.blueprint.title',
    to: '/manage/blueprints/new',
  },
  {
    actionKey: 'onboarding.steps.contexts.action',
    descriptionKey: 'onboarding.steps.contexts.description',
    documentationPage: 'contexts',
    icon: ContextIcon,
    titleKey: 'onboarding.steps.contexts.title',
    to: '/manage/contexts',
  },
  {
    actionKey: 'onboarding.steps.invite.action',
    capability: 'members_manage',
    descriptionKey: 'onboarding.steps.invite.description',
    documentationPage: 'workspaces',
    icon: WorkspaceIcon,
    titleKey: 'onboarding.steps.invite.title',
    to: '/manage/workspace/invitations',
  },
  {
    actionKey: 'onboarding.steps.extensions.action',
    capability: 'extensions_read',
    descriptionKey: 'onboarding.steps.extensions.description',
    documentationPage: 'extensions',
    icon: ExtensionIcon,
    titleKey: 'onboarding.steps.extensions.title',
    to: '/manage/extensions',
  },
];

export const canCreateBlueprints = (capabilities?: OnboardingCapabilities) =>
  capabilities?.blueprints_write === true;

export const getVisibleOnboardingSteps = (
  capabilities?: OnboardingCapabilities,
) =>
  canCreateBlueprints(capabilities)
    ? onboardingSteps.filter(
        (step) =>
          step.capability === undefined ||
          capabilities?.[step.capability] === true,
      )
    : [];
