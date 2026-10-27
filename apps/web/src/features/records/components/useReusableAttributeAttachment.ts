import { useMutation, useQuery } from '@tanstack/react-query';
import {
  attachReusableAttribute,
  attachReusableAttributeGroup,
} from '../../reusable-attributes/api';
import { latestReusableAttributeRevisions } from '../../reusable-attributes/latestRevisions';
import {
  reusableAttributeListOptions,
  reusableAttributeGroupListOptions,
} from '../../reusable-attributes/queryOptions';

/** Loads reusable attributes and groups and attaches them to a record. */
export const useReusableAttributeAttachment = (
  recordId: string,
  onAttached: () => void,
) => {
  const reusableAttributes = useQuery(reusableAttributeListOptions());
  const reusableGroups = useQuery(reusableAttributeGroupListOptions());
  const attach = useMutation({
    mutationFn: (revisionId: string) =>
      attachReusableAttribute(recordId, revisionId),
    onSuccess: onAttached,
  });
  const attachGroup = useMutation({
    mutationFn: (groupId: string) =>
      attachReusableAttributeGroup(recordId, groupId),
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
