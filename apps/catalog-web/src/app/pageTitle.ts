import type { TFunction } from 'i18next';

const titleSeparator = ' · ';
const shortIdLength = 8;

const joinTitle = (...parts: string[]) => parts.join(titleSeparator);
const shortId = (id: string) => id.slice(0, shortIdLength);

type Segments = (string | undefined)[];

const agentsTitle = (t: TFunction, [conversation]: Segments) => {
  if (conversation === 'new') return t('agents.newAgentConversation');
  return conversation
    ? joinTitle(t('agents.conversation'), shortId(conversation))
    : t('agents.agentConversations');
};

const entitiesTitle = (t: TFunction, [entityId, subpage]: Segments) => {
  if (entityId === 'new') return t('entities.createEntity');
  const entity = `${t('workspace.entity')} ${entityId ? shortId(entityId) : ''}`;
  const subpageKeys: Record<string, string> = {
    changes: 'entities.changes',
    migrate: 'entities.upgradeEntity',
  };
  const subpageKey = subpage ? subpageKeys[subpage] : undefined;
  return subpageKey ? joinTitle(t(subpageKey), entity) : entity;
};

const blueprintsTitle = (t: TFunction, [blueprintId, subpage]: Segments) => {
  if (blueprintId === 'new') return t('blueprints.newBlueprint');
  if (!blueprintId) return t('navigation.blueprints');
  return subpage === 'revisions'
    ? t('blueprints.newBlueprintRevision')
    : joinTitle(t('navigation.blueprints'), shortId(blueprintId));
};

const workflowsTitle = (t: TFunction, [workflowId, subpage]: Segments) => {
  if (workflowId === 'new') return t('workflows.newWorkflow');
  if (!workflowId) return t('navigation.workflows');
  return subpage === 'revisions'
    ? t('workflows.newRevision')
    : joinTitle(t('navigation.workflows'), shortId(workflowId));
};

const extensionManagementPageKeys: Record<string, string> = {
  installed: 'extensions.installed',
  layout: 'extensions.layout',
  marketplace: 'extensions.marketplace',
  sideload: 'extensions.uploadTitle',
};

const extensionsManagementTitle = (t: TFunction, [page]: Segments) => {
  if (!page) return t('navigation.extensions');
  const key = extensionManagementPageKeys[page];
  return key ? t(key) : joinTitle(t('navigation.extensions'), page);
};

const rulesSectionKeys: Record<string, string> = {
  findings: 'app.findings',
  runs: 'app.runs',
};

const rulesTitle = (t: TFunction, [section]: Segments) => {
  const key = section ? rulesSectionKeys[section] : undefined;
  return key ? joinTitle(t('navigation.rules'), t(key)) : t('navigation.rules');
};

const simpleManagementPageKeys: Record<string, string> = {
  'audit-log': 'navigation.auditLog',
  'background-processing': 'navigation.backgroundProcessing',
  'data-health': 'navigation.dataHealth',
  exports: 'navigation.exports',
  lexicon: 'navigation.lexicon',
  'system-health': 'navigation.systemHealth',
};

const managementTitle = (t: TFunction, [group, ...rest]: Segments) => {
  const [item] = rest;
  switch (group) {
    case undefined:
      return t('navigation.dashboard');
    case 'blueprints':
      return blueprintsTitle(t, rest);
    case 'workflows':
      return workflowsTitle(t, rest);
    case 'extensions':
      return extensionsManagementTitle(t, rest);
    case 'workspace':
      return item
        ? joinTitle(t(`workspace.${item}`), t('navigation.workspaceManagement'))
        : t('navigation.workspaceManagement');
    case 'rules':
      return rulesTitle(t, rest);
    case 'contexts':
      return item === 'new'
        ? t('contexts.createContext')
        : t('navigation.contexts');
    case 'reusable-attributes':
      return item && item !== 'new'
        ? joinTitle(t('navigation.reusableAttributes'), shortId(item))
        : t('navigation.reusableAttributes');
    default: {
      const key = simpleManagementPageKeys[group];
      return key ? t(key) : t('errors.notFoundTitle');
    }
  }
};

const sectionTitle = (t: TFunction, [section, ...rest]: Segments) => {
  const [group] = rest;
  switch (section) {
    case undefined:
      return t('navigation.entityExplorer');
    case 'login':
      return t('auth.signIn');
    case 'password-reset':
      return t('auth.resetPassword');
    case 'onboarding':
      return t('navigation.workspaceManagement');
    case 'invitations':
      return t('workspace.invitations');
    case 'profile':
      if (group === 'extension-runs') return t('profile.extensionRuns');
      if (group !== 'personal-access-tokens') return t('navigation.profile');
      return rest[1] === 'new'
        ? t('profile.createToken')
        : t('profile.tokenList');
    case 'agents':
      return agentsTitle(t, rest);
    case 'inbox':
      return t('navigation.inbox');
    case 'entities':
      return entitiesTitle(t, rest);
    case 'extensions':
      return rest[1] && group
        ? joinTitle(t('navigation.apps'), group)
        : t('navigation.apps');
    case 'manage':
      return managementTitle(t, rest);
    default:
      return t('errors.notFoundTitle');
  }
};

// Match the most specific routes first. Resource names are filled in by detail pages
// after their data loads; these labels also serve as useful loading/error fallbacks.
export const pageTitle = (pathname: string, t: TFunction): string =>
  joinTitle(
    sectionTitle(t, pathname.split('/').filter(Boolean)),
    t('app.attricat'),
  );
