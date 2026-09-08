import { Toast } from '@douyinfe/semi-ui';
import { useCallback, useLayoutEffect, useRef, useState } from 'react';

import { adminErrorMessage } from '../../shared/adminErrorMessage';
import type { DetailDrawerData, DetailDrawerFieldMeta } from '../../shared/DetailDrawer';

export type ResourceDetailLoader = (signal: AbortSignal) => Promise<DetailDrawerData>;

function withFieldMeta(detail: DetailDrawerData, base: DetailDrawerFieldMeta): DetailDrawerData {
  return {
    ...detail,
    fieldMeta: {
      assets: { ...base.assets, ...detail.fieldMeta?.assets },
      labels: { ...base.labels, ...detail.fieldMeta?.labels },
      types: { ...base.types, ...detail.fieldMeta?.types },
      valueMaps: { ...base.valueMaps, ...detail.fieldMeta?.valueMaps }
    }
  };
}

/** Every detail source shares one page-owned request; abort alone cannot guard late mock/cache results. */
export function useResourceDetail(context: unknown, fieldMeta: DetailDrawerFieldMeta) {
  const [detail, setDetail] = useState<DetailDrawerData | null>(null);
  const request = useRef<AbortController | null>(null);
  const mounted = useRef(false);
  const cancelRequest = useCallback(() => {
    request.current?.abort();
    request.current = null;
  }, []);
  const closeDetail = useCallback(() => {
    cancelRequest();
    setDetail(null);
  }, [cancelRequest]);

  useLayoutEffect(() => {
    mounted.current = true;
    closeDetail();
    return () => {
      mounted.current = false;
      cancelRequest();
    };
  }, [cancelRequest, closeDetail, context]);

  const openDetail = useCallback((next: DetailDrawerData) => {
    cancelRequest();
    if (mounted.current) setDetail(withFieldMeta(next, fieldMeta));
  }, [cancelRequest, fieldMeta]);

  const loadDetail = useCallback(async (loader: ResourceDetailLoader) => {
    cancelRequest();
    if (!mounted.current) return;
    const controller = new AbortController();
    request.current = controller;
    const isCurrent = () => mounted.current && request.current === controller && !controller.signal.aborted;
    try {
      const next = await loader(controller.signal);
      if (isCurrent()) setDetail(withFieldMeta(next, fieldMeta));
    } catch (cause) {
      if (isCurrent()) Toast.error(adminErrorMessage(cause, '操作失败'));
    } finally {
      if (request.current === controller) request.current = null;
    }
  }, [cancelRequest, fieldMeta]);

  return { closeDetail, detail, loadDetail, openDetail };
}
