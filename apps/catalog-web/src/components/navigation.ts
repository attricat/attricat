import {
  AgentIcon,
  AuditLogIcon,
  BlueprintIcon,
  ContextIcon,
  DataHealthIcon,
  ExplorerIcon,
  ExtensionIcon,
  ProfileIcon,
  WorkflowIcon,
  WorkspaceIcon,
} from './system-icons';

type NavigationCapabilities = {
  audit_read?: boolean;
  extensions_read?: boolean;
  members_manage?: boolean;
  roles_manage?: boolean;
  tokens_manage?: boolean;
  workflows_read?: boolean;
};

export const navigationRoutes = {
  agents: '/agents',
  auditLog: '/manage/audit-log',
  blueprints: '/manage/blueprints',
  contexts: '/manage/contexts',
  dataHealth: '/manage/data-health',
  explore: '/',
  extensions: '/manage/extensions',
  manage: '/manage',
  profile: '/profile',
  workflows: '/manage/workflows',
  workspace: '/manage/workspace/members',
} as const;

export const primaryNavigationItems = [
  {
    icon: ExplorerIcon,
    labelKey: 'navigation.entityExplorer',
    to: navigationRoutes.explore,
  },
  {
    icon: AgentIcon,
    labelKey: 'navigation.agents',
    to: navigationRoutes.agents,
  },
] as const;

export const managementNavigationItems = [
  {
    descriptionKey: 'management.blueprintsDescription',
    icon: BlueprintIcon,
    labelKey: 'navigation.blueprints',
    to: navigationRoutes.blueprints,
  },
  {
    descriptionKey: 'management.contextsDescription',
    icon: ContextIcon,
    labelKey: 'navigation.contexts',
    to: navigationRoutes.contexts,
  },
  {
    descriptionKey: 'management.dataHealthDescription',
    icon: DataHealthIcon,
    labelKey: 'navigation.dataHealth',
    to: navigationRoutes.dataHealth,
  },
  {
    descriptionKey: 'management.workspaceDescription',
    icon: WorkspaceIcon,
    labelKey: 'navigation.workspaceManagement',
    to: navigationRoutes.workspace,
  },
  {
    descriptionKey: 'management.governanceDescription',
    icon: AuditLogIcon,
    labelKey: 'navigation.auditLog',
    to: navigationRoutes.auditLog,
  },
  {
    descriptionKey: 'management.extensionsDescription',
    icon: ExtensionIcon,
    labelKey: 'navigation.extensions',
    to: navigationRoutes.extensions,
  },
  {
    descriptionKey: 'management.workflowsDescription',
    icon: WorkflowIcon,
    labelKey: 'navigation.workflows',
    to: navigationRoutes.workflows,
  },
] as const;

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
        capabilities?.tokens_manage,
      );
    }
    if (item.to === navigationRoutes.auditLog) return capabilities?.audit_read;
    if (item.to === navigationRoutes.extensions)
      return capabilities?.extensions_read;
    if (item.to === navigationRoutes.workflows)
      return capabilities?.workflows_read;
    return true;
  });
