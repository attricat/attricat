import type { TFunction } from 'i18next';

// Match the most specific routes first. Resource names are filled in by detail pages
// after their data loads; these labels also serve as useful loading/error fallbacks.
export const pageTitle = (pathname: string, t: TFunction): string => {
  const parts = pathname.split('/').filter(Boolean);
  const [section, group, item, subpage] = parts;
  const label = (() => {
    if (!section) return t('navigation.entityExplorer');
    if (section === 'login') return t('auth.signIn');
    if (section === 'password-reset') return t('auth.resetPassword');
    if (section === 'onboarding') return t('navigation.workspaceManagement');
    if (section === 'invitations') return t('workspace.invitations');
    if (section === 'profile')
      return group === 'personal-access-tokens'
        ? t('profile.tokenList')
        : t('navigation.profile');
    if (section === 'agents')
      return group === 'new'
        ? t('agents.newAgentConversation')
        : group
          ? `${t('agents.conversation')} · ${group.slice(0, 8)}`
          : t('agents.agentConversations');
    if (section === 'entities') {
      if (group === 'new') return t('entities.createEntity');
      const entity = `${t('workspace.entity')} ${group?.slice(0, 8) ?? ''}`;
      return item === 'changes'
        ? `${t('entities.changes')} · ${entity}`
        : item === 'edit'
          ? `${t('entities.editEntity')} · ${entity}`
          : item === 'migrate'
            ? `${t('entities.upgradeEntity')} · ${entity}`
            : entity;
    }
    if (section === 'extensions')
      return item ? `${t('navigation.apps')} · ${group}` : t('navigation.apps');
    if (section !== 'manage') return t('errors.notFoundTitle');
    if (!group) return t('navigation.dashboard');
    if (group === 'blueprints')
      return item === 'new'
        ? t('blueprints.newBlueprint')
        : item
          ? subpage === 'revisions'
            ? t('blueprints.newBlueprintRevision')
            : `${t('navigation.blueprints')} · ${item.slice(0, 8)}`
          : t('navigation.blueprints');
    if (group === 'workflows')
      return item === 'new'
        ? t('workflows.newWorkflow')
        : item && subpage === 'revisions'
          ? t('workflows.newRevision')
          : item
            ? `${t('navigation.workflows')} · ${item.slice(0, 8)}`
            : t('navigation.workflows');
    if (group === 'extensions')
      return item === 'marketplace'
        ? t('extensions.marketplace')
        : item === 'installed'
          ? t('extensions.installed')
          : item === 'layout'
            ? t('extensions.layout')
            : item === 'sideload'
              ? t('extensions.uploadTitle')
              : item
                ? `${t('navigation.extensions')} · ${item}`
                : t('navigation.extensions');
    if (group === 'workspace')
      return item
        ? `${t(`workspace.${item}`)} · ${t('navigation.workspaceManagement')}`
        : t('navigation.workspaceManagement');
    if (group === 'rules')
      return item === 'findings'
        ? `${t('navigation.rules')} · ${t('app.findings')}`
        : item === 'runs'
          ? `${t('navigation.rules')} · ${t('app.runs')}`
          : t('navigation.rules');
    if (group === 'contexts')
      return item === 'new'
        ? t('contexts.createContext')
        : t('navigation.contexts');
    if (group === 'reusable-attributes')
      return item && item !== 'new'
        ? `${t('navigation.reusableAttributes')} · ${item.slice(0, 8)}`
        : t('navigation.reusableAttributes');
    const labels: Record<string, string> = {
      'audit-log': t('navigation.auditLog'),
      'background-processing': t('navigation.backgroundProcessing'),
      'data-health': t('navigation.dataHealth'),
      exports: t('navigation.exports'),
    };
    return labels[group] ?? t('errors.notFoundTitle');
  })();
  return `${label} · ${t('app.attricat')}`;
};
