import {
  AgentIcon,
  AuditLogIcon,
  BackgroundProcessingIcon,
  BlueprintIcon,
  ContextIcon,
  DataHealthIcon,
  ExplorerIcon,
  ExtensionIcon,
  ExportIcon,
  InboxIcon,
  LexiconIcon,
  ProfileIcon,
  ReusableAttributeIcon,
  RuleIcon,
  SystemHealthIcon,
  WorkflowIcon,
  WorkspaceIcon,
} from './systemIcons';
import { capabilities } from '../features/auth/capabilities';
import type { Capability } from '../features/auth/useCan';

type NavigationCapabilities = Partial<Record<Capability, boolean>>;

/**
 * An item with `anyOf` shows only to users granted at least one of those
 * capabilities; items without it always show.
 */
const isAllowed = (
  anyOf: readonly Capability[] | undefined,
  granted: NavigationCapabilities | undefined,
) => !anyOf || anyOf.some((capability) => granted?.[capability]);

export const navigationRoutes = {
  agents: '/agents',
  auditLog: '/manage/audit-log',
  blueprints: '/manage/blueprints',
  contexts: '/manage/contexts',
  dataHealth: '/manage/data-health',
  backgroundProcessing: '/manage/background-processing',
  explore: '/',
  inbox: '/inbox',
  lexicon: '/manage/lexicon',
  extensions: '/manage/extensions',
  extensionContributions: '/extensions',
  exports: '/manage/exports',
  manage: '/manage',
  profile: '/profile',
  rules: '/manage/rules',
  systemHealth: '/manage/system-health',
  reusableAttributes: '/manage/reusable-attributes',
  workflows: '/manage/workflows',
  workspace: '/manage/workspace',
} as const;

// Routes rendered outside the authenticated application shell.
export const publicRoutes = {
  login: '/login',
  onboarding: '/onboarding',
  passwordReset: '/password-reset',
} as const;

const primaryNavigationItems = [
  {
    icon: ExplorerIcon,
    labelKey: 'navigation.recordExplorer',
    to: navigationRoutes.explore,
  },
  {
    icon: AgentIcon,
    labelKey: 'navigation.agents',
    to: navigationRoutes.agents,
    anyOf: [capabilities.agentsRun],
  },
] as const;

// Shown in the navigation footer, above the documentation link.
export const inboxNavigationItem = {
  icon: InboxIcon,
  labelKey: 'navigation.inbox',
  to: navigationRoutes.inbox,
} as const;

// Items are listed by purpose; navigation labels each group.
const managementNavigationItems = [
  {
    descriptionKey: 'management.blueprintsDescription',
    group: 'dataModel',
    icon: BlueprintIcon,
    labelKey: 'navigation.blueprints',
    to: navigationRoutes.blueprints,
  },
  {
    descriptionKey: 'management.reusableAttributesDescription',
    group: 'dataModel',
    icon: ReusableAttributeIcon,
    labelKey: 'navigation.reusableAttributes',
    to: navigationRoutes.reusableAttributes,
  },
  {
    descriptionKey: 'management.contextsDescription',
    group: 'dataModel',
    icon: ContextIcon,
    labelKey: 'navigation.contexts',
    to: navigationRoutes.contexts,
  },
  {
    descriptionKey: 'management.lexiconDescription',
    group: 'dataModel',
    icon: LexiconIcon,
    labelKey: 'navigation.lexicon',
    to: navigationRoutes.lexicon,
  },
  {
    descriptionKey: 'management.rulesDescription',
    group: 'automation',
    icon: RuleIcon,
    labelKey: 'navigation.rules',
    to: navigationRoutes.rules,
    anyOf: [capabilities.rulesRead],
  },
  {
    descriptionKey: 'management.workflowsDescription',
    group: 'automation',
    icon: WorkflowIcon,
    labelKey: 'navigation.workflows',
    to: navigationRoutes.workflows,
    anyOf: [capabilities.workflowsRead],
  },
  {
    descriptionKey: 'management.extensionsDescription',
    group: 'automation',
    icon: ExtensionIcon,
    labelKey: 'navigation.extensions',
    to: navigationRoutes.extensions,
    anyOf: [capabilities.extensionsRead],
  },
  {
    descriptionKey: 'management.exportsDescription',
    group: 'dataOperations',
    icon: ExportIcon,
    labelKey: 'navigation.exports',
    to: navigationRoutes.exports,
  },
  {
    descriptionKey: 'management.dataHealthDescription',
    group: 'dataOperations',
    icon: DataHealthIcon,
    labelKey: 'navigation.dataHealth',
    to: navigationRoutes.dataHealth,
    anyOf: [capabilities.dataHealthRead],
  },
  {
    descriptionKey: 'management.workspaceDescription',
    group: 'access',
    icon: WorkspaceIcon,
    labelKey: 'navigation.workspaceManagement',
    to: navigationRoutes.workspace,
    anyOf: [
      capabilities.membersManage,
      capabilities.rolesManage,
      capabilities.workspaceNavigationManage,
      capabilities.agentsManage,
    ],
  },
  {
    descriptionKey: 'management.governanceDescription',
    group: 'access',
    icon: AuditLogIcon,
    labelKey: 'navigation.auditLog',
    to: navigationRoutes.auditLog,
    anyOf: [capabilities.auditRead],
  },
  {
    descriptionKey: 'management.backgroundProcessingDescription',
    group: 'system',
    icon: BackgroundProcessingIcon,
    labelKey: 'navigation.backgroundProcessing',
    to: navigationRoutes.backgroundProcessing,
    anyOf: [capabilities.dataHealthRead],
  },
  {
    descriptionKey: 'management.systemHealthDescription',
    group: 'system',
    icon: SystemHealthIcon,
    labelKey: 'navigation.systemHealth',
    to: navigationRoutes.systemHealth,
    anyOf: [capabilities.dataHealthRead],
  },
] as const;

export const getVisiblePrimaryNavigationItems = (
  granted?: NavigationCapabilities,
) =>
  primaryNavigationItems.filter((item) =>
    isAllowed('anyOf' in item ? item.anyOf : undefined, granted),
  );

export const profileNavigationItem = {
  icon: ProfileIcon,
  labelKey: 'navigation.profile',
  to: navigationRoutes.profile,
} as const;

export const getVisibleManagementNavigationItems = (
  granted?: NavigationCapabilities,
) =>
  managementNavigationItems.filter((item) =>
    isAllowed('anyOf' in item ? item.anyOf : undefined, granted),
  );
