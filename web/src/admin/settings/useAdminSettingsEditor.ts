import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { type Dispatch, type SetStateAction, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ApiError } from '../../api/client';
import { settingsValuesEqual } from './differences';
import {
  adminSettingsQueryKeys,
  settingsErrorMessage,
  settingsMutationRetry,
  settingsQueryRetry
} from './query';

export type SettingsFeedback = {
  kind: 'conflict' | 'error' | 'success';
  message: string;
};

type SaveVariables<TForm> = {
  draft: TForm;
  reason: string;
};

type UseAdminSettingsEditorOptions<TData, TForm> = {
  areEqual?: (left: TForm, right: TForm) => boolean;
  initialForm: TForm;
  load: () => Promise<TData>;
  save: (draft: TForm, reason: string) => Promise<TData>;
  selectForm: (data: TData) => TForm;
  settingKey: string;
  successMessage: string;
};

export type AdminSettingsEditor<TData, TForm> = {
  baseline: TForm | null;
  data: TData | undefined;
  draft: TForm;
  feedback: SettingsFeedback | null;
  isDirty: boolean;
  isFetching: boolean;
  isInitialLoading: boolean;
  isReady: boolean;
  isSaving: boolean;
  loadError: Error | null;
  queryKey: ReturnType<typeof adminSettingsQueryKeys.detail>;
  reloadLatest: () => Promise<void>;
  saveChanges: (reason: string) => Promise<TData>;
  setDraft: Dispatch<SetStateAction<TForm>>;
};

/**
 * 单例设置页的统一状态机：读取权威值、保留本地草稿、提交后更新并失效缓存，409 时保留草稿。
 */
export function useAdminSettingsEditor<TData, TForm>({
  areEqual = settingsValuesEqual,
  initialForm,
  load,
  save,
  selectForm,
  settingKey,
  successMessage
}: UseAdminSettingsEditorOptions<TData, TForm>): AdminSettingsEditor<TData, TForm> {
  const queryClient = useQueryClient();
  const queryKey = useMemo(() => adminSettingsQueryKeys.detail(settingKey), [settingKey]);
  const selectFormRef = useRef(selectForm);
  selectFormRef.current = selectForm;
  const areEqualRef = useRef(areEqual);
  areEqualRef.current = areEqual;

  const [baseline, setBaseline] = useState<TForm | null>(null);
  const [draft, setDraftState] = useState<TForm>(initialForm);
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);

  const query = useQuery({
    queryKey,
    queryFn: load,
    retry: settingsQueryRetry,
    refetchOnWindowFocus: false
  });

  const isDirty = baseline !== null && !areEqualRef.current(baseline, draft);

  useEffect(() => {
    if (query.data === undefined || isDirty) {
      return;
    }

    const next = selectFormRef.current(query.data);
    setBaseline(next);
    setDraftState(next);
  }, [isDirty, query.data]);

  const mutation = useMutation<TData, Error, SaveVariables<TForm>>({
    mutationFn: ({ draft: submittedDraft, reason }) => save(submittedDraft, reason),
    retry: settingsMutationRetry,
    onSuccess: async (saved) => {
      const next = selectFormRef.current(saved);
      queryClient.setQueryData(queryKey, saved);
      setBaseline(next);
      setDraftState(next);
      setFeedback({ kind: 'success', message: successMessage });
      await queryClient.invalidateQueries({ queryKey, exact: true, refetchType: 'none' });
    },
    onError: async (error) => {
      const conflict = error instanceof ApiError && error.status === 409;
      setFeedback({
        kind: conflict ? 'conflict' : 'error',
        message: settingsErrorMessage(error)
      });
      if (conflict) {
        await queryClient.invalidateQueries({ queryKey, exact: true, refetchType: 'none' });
      }
    }
  });

  const setDraft: Dispatch<SetStateAction<TForm>> = useCallback((next) => {
    setFeedback(null);
    setDraftState(next);
  }, []);

  const reloadLatest = useCallback(async () => {
    setFeedback(null);
    const result = await query.refetch({ cancelRefetch: true });
    if (result.error || result.data === undefined) {
      const error = result.error ?? new Error('未读取到配置数据');
      setFeedback({ kind: 'error', message: settingsErrorMessage(error, '加载配置失败，请稍后重试。') });
      throw error;
    }

    const next = selectFormRef.current(result.data);
    setBaseline(next);
    setDraftState(next);
  }, [query]);

  const saveChanges = useCallback(
    (reason: string) => mutation.mutateAsync({ draft, reason }),
    [draft, mutation]
  );

  return {
    baseline,
    data: query.data,
    draft,
    feedback,
    isDirty,
    isFetching: query.isFetching,
    isInitialLoading: query.isPending && baseline === null,
    isReady: baseline !== null,
    isSaving: mutation.isPending,
    loadError: query.error && baseline === null ? query.error : null,
    queryKey,
    reloadLatest,
    saveChanges,
    setDraft
  };
}
