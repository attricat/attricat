import { useMutation, type UseMutationOptions } from '@tanstack/react-query';
import { useRef } from 'react';

/**
 * A mutation whose `submit` ignores further calls until the running one
 * settles, including a second submit in the same tick, before React has
 * re-rendered with `isPending`.
 */
export const useExclusiveMutation = <TData, TVariables = void>(
  options: UseMutationOptions<TData, Error, TVariables>,
) => {
  const running = useRef(false);
  const mutation = useMutation(options);
  const submit = (variables: TVariables) => {
    if (running.current) return;
    running.current = true;
    mutation.mutate(variables, {
      onSettled: () => {
        running.current = false;
      },
    });
  };
  return { ...mutation, submit };
};
