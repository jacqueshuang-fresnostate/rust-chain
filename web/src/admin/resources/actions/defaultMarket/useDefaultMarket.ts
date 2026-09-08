import { useCallback, useEffect, useRef, useState } from 'react';
import { apiRequest, ApiError } from '../../../../api/client';
import { errorMessage } from '../shared';
import { defaultMarketDraft, defaultMarketPayload } from './model';
import type { DefaultMarketDraft, DefaultMarketPreview, DefaultMarketResponse } from './types';

/** 编辑器仅在打开后挂载。每次会话和预览都有独立代次，关闭后旧响应不会回写新草稿。 */
export function useDefaultMarket(pairId: string, onSaved: () => void) {
  const endpoint = `/admin/api/v1/market-pairs/${pairId}/default-generator`;
  const [value, setValue] = useState<DefaultMarketResponse | null>(null);
  const [draft, setDraft] = useState<DefaultMarketDraft | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [conflict, setConflict] = useState(false);
  const [preview, setPreview] = useState<DefaultMarketPreview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [previewError, setPreviewError] = useState('');
  const session = useRef(0);
  const previewEpoch = useRef(0);
  const getController = useRef<AbortController | null>(null);
  const previewController = useRef<AbortController | null>(null);

  const clearPreview = useCallback(() => {
    previewEpoch.current += 1;
    previewController.current?.abort();
    setPreview(null);
    setPreviewError('');
    setPreviewing(false);
  }, []);

  const load = useCallback(async () => {
    const ticket = ++session.current;
    getController.current?.abort();
    const controller = new AbortController();
    getController.current = controller;
    setLoading(true);
    setError('');
    clearPreview();
    try {
      const result = await apiRequest<DefaultMarketResponse>(endpoint, { signal: controller.signal });
      if (session.current !== ticket) return;
      setValue(result);
      setDraft(defaultMarketDraft(result));
      setConflict(false);
    } catch (cause) {
      if (session.current === ticket) setError(errorMessage(cause));
    } finally {
      if (session.current === ticket) setLoading(false);
    }
  }, [endpoint, clearPreview]);

  useEffect(() => {
    void load();
    return () => {
      session.current += 1;
      previewEpoch.current += 1;
      getController.current?.abort();
      previewController.current?.abort();
    };
  }, [load]);

  function edit<K extends keyof DefaultMarketDraft>(key: K, next: DefaultMarketDraft[K]) {
    setDraft((current) => current ? { ...current, [key]: next } : null);
    clearPreview();
  }

  async function mutate(reason: string, pauseAll?: boolean) {
    if (!value || !draft || loading || saving || conflict) throw new Error('请重新加载最新配置后操作。');
    const ticket = session.current;
    setSaving(true);
    setError('');
    clearPreview();
    try {
      const result = await apiRequest<DefaultMarketResponse>(`${endpoint}${pauseAll === undefined ? '' : '/pause-all'}`, {
        method: 'PATCH', body: JSON.stringify(pauseAll === undefined
          ? { ...defaultMarketPayload(draft, value.version), reason }
          : { expected_version: value.version, all_market_paused: pauseAll, reason })
      });
      if (ticket !== session.current) return;
      setValue(result);
      setDraft(defaultMarketDraft(result));
      onSaved();
    } catch (cause) {
      if (ticket === session.current) {
        setError(errorMessage(cause));
        if (cause instanceof ApiError && cause.status === 409) setConflict(true);
      }
      throw cause;
    } finally {
      if (ticket === session.current) setSaving(false);
    }
  }

  async function runPreview() {
    if (!value || !draft || loading || saving || conflict) return;
    clearPreview();
    const ticket = previewEpoch.current;
    const owner = session.current;
    const controller = new AbortController();
    previewController.current = controller;
    const { expected_version, initial_price, config } = defaultMarketPayload(draft, value.version);
    setPreviewing(true);
    try {
      const result = await apiRequest<DefaultMarketPreview>(`${endpoint}/preview`, {
        method: 'POST', signal: controller.signal, body: JSON.stringify({ expected_version, initial_price, config })
      });
      if (ticket === previewEpoch.current && owner === session.current) setPreview(result);
    } catch (cause) {
      if (ticket === previewEpoch.current && owner === session.current) {
        setPreviewError(errorMessage(cause));
        if (cause instanceof ApiError && cause.status === 409) setConflict(true);
      }
    } finally {
      if (ticket === previewEpoch.current && owner === session.current) setPreviewing(false);
    }
  }

  const dirty = Boolean(value && draft && JSON.stringify(draft) !== JSON.stringify(defaultMarketDraft(value)));
  return { value, draft, loading, saving, error, conflict, dirty, preview, previewError, previewing, edit, load, mutate, runPreview };
}
