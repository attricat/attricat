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

type NavigationCapabilities = {
  agents_run?: boolean;
  audit_read?: boolean;
  data_health_read?: boolean;
  extensions_read?: boolean;
  members_manage?: boolean;
  roles_manage?: boolean;
  workflows_read?: boolean;
  workspace_navigation_manage?: boolean;
  agents_manage?: boolean;
  rules_read?: boolean;
};

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
  workspace: '/manage/workspace/members',
} as const;

// Routes rendered outside the authenticated application shell.
export const publicRoutes = {
  login: '/login',
  onboarding: '/onboarding',
  passwordReset: '/password-reset',
} as const;

export const primaryNavigationItems = [
  {
    icon: ExplorerIcon,
    labelKey: 'navigation.recordExplorer',
    to: navigationRoutes.explore,
  },
  {
    icon: AgentIcon,
    labelKey: 'navigation.agents',
    to: navigationRoutes.agents,
  },
] as const;

// Shown in the navigation footer, above the documentation link.
export const inboxNavigationItem = {
  icon: InboxIcon,
  labelKey: 'navigation.inbox',
  to: navigationRoutes.inbox,
} as const;

// Items are listed by purpose; navigation labels each group.
export const managementNavigationItems = [
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
  },
  {
    descriptionKey: 'management.workflowsDescription',
    group: 'automation',
    icon: WorkflowIcon,
    labelKey: 'navigation.workflows',
    to: navigationRoutes.workflows,
  },
  {
    descriptionKey: 'management.extensionsDescription',
    group: 'automation',
    icon: ExtensionIcon,
    labelKey: 'navigation.extensions',
    to: navigationRoutes.extensions,
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
  },
  {
    descriptionKey: 'management.workspaceDescription',
    group: 'access',
    icon: WorkspaceIcon,
    labelKey: 'navigation.workspaceManagement',
    to: navigationRoutes.workspace,
  },
  {
    descriptionKey: 'management.governanceDescription',
    group: 'access',
    icon: AuditLogIcon,
    labelKey: 'navigation.auditLog',
    to: navigationRoutes.auditLog,
  },
  {
    descriptionKey: 'management.backgroundProcessingDescription',
    group: 'system',
    icon: BackgroundProcessingIcon,
    labelKey: 'navigation.backgroundProcessing',
    to: navigationRoutes.backgroundProcessing,
  },
  {
    descriptionKey: 'management.systemHealthDescription',
    group: 'system',
    icon: SystemHealthIcon,
    labelKey: 'navigation.systemHealth',
    to: navigationRoutes.systemHealth,
  },
] as const;

export const getVisiblePrimaryNavigationItems = (
  capabilities?: NavigationCapabilities,
) =>
  primaryNavigationItems.filter(
    (item) => item.to !== navigationRoutes.agents || capabilities?.agents_run,
  );

export const profileNavigationItem = {
  icon: ProfileIcon,
  labelKey: 'navigation.profile',
  to: navigationRoutes.profile,
} as const;

export const getVisibleManagementNavigationItems = (
  capabilities?: NavigationCapabilities,
) =>
  managementNavigationItems.filter((item) => {
    if (item.to === navigationRoutes.workspace) {
      return Boolean(
        capabilities?.members_manage ||
        capabilities?.roles_manage ||
        capabilities?.workspace_navigation_manage ||
        capabilities?.agents_manage,
      );
    }
    if (
      item.to === navigationRoutes.backgroundProcessing ||
      item.to === navigationRoutes.dataHealth ||
      item.to === navigationRoutes.systemHealth
    )
      return capabilities?.data_health_read;
    if (item.to === navigationRoutes.auditLog) return capabilities?.audit_read;
    if (item.to === navigationRoutes.extensions)
      return capabilities?.extensions_read;
    if (item.to === navigationRoutes.workflows)
      return capabilities?.workflows_read;
    if (item.to === navigationRoutes.rules) return capabilities?.rules_read;
    return true;
  });
