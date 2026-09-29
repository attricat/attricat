import { useMutation, useQuery } from '@tanstack/react-query';
import {
  attachReusableAttribute,
  attachReusableAttributeGroup,
  listReusableAttributeGroups,
  listReusableAttributes,
} from '../../reusable-attributes/api';
import { latestReusableAttributeRevisions } from '../../reusable-attributes/latestRevisions';
import { reusableAttributeQueryKeys } from '../../reusable-attributes/queryKeys';

/** Loads reusable attributes and groups and attaches them to an entity. */
export const useReusableAttributeAttachment = (
  entityId: string,
  onAttached: () => void,
) => {
  const reusableAttributes = useQuery({
    queryKey: reusableAttributeQueryKeys.definitions(),
    queryFn: ({ signal }) => listReusableAttributes(false, signal),
  });
  const reusableGroups = useQuery({
    queryKey: reusableAttributeQueryKeys.groups(),
    queryFn: ({ signal }) => listReusableAttributeGroups(signal),
  });
  const attach = useMutation({
    mutationFn: (revisionId: string) =>
      attachReusableAttribute(entityId, revisionId),
    onSuccess: onAttached,
  });
  const attachGroup = useMutation({
    mutationFn: (groupId: string) =>
      attachReusableAttributeGroup(entityId, groupId),
    onSuccess: onAttached,
  });
  const latestAttributes = latestReusableAttributeRevisions(
    reusableAttributes.data ?? [],
  ).sort(
    (first, second) =>
      first.namespace.localeCompare(second.namespace) ||
      first.code.localeCompare(second.code),
  );
  return {
    attach,
    attachGroup,
    attributes: latestAttributes,
    attributesUnavailable:
      reusableAttributes.isPending || reusableAttributes.isError,
    error:
      attach.error ??
      attachGroup.error ??
      reusableAttributes.error ??
      reusableGroups.error,
    groups: reusableGroups.data ?? [],
    groupsUnavailable: reusableGroups.isPending || reusableGroups.isError,
  };
};
