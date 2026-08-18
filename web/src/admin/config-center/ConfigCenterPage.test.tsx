import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiRequest } from '../../api/client';
import { ConfigCenterPage } from './ConfigCenterPage';

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return { ...actual, apiRequest: vi.fn() };
});

const apiRequestMock = vi.mocked(apiRequest);

function response() {
  return {
    items: [
      {
        applied_version: 2,
        code: 'prediction_settings',
        config_path: '/admin/prediction/settings',
        config_status: 'pending_apply',
        configured_count: 1,
        group: 'market',
        group_name: '行情与交易',
        last_applied_at: 1_735_732_700_000,
        last_error_summary: null,
        last_modified_at: 1_735_732_800_000,
        last_tested_at: null,
        name: '预测配置',
        operation_path: '/admin/prediction/sync',
        published_version: 3,
        runtime_status: 'stopped'
      },
      {
        applied_version: null,
        code: 'smtp',
        config_path: '/admin/system/smtp',
        config_status: 'runtime_error',
        configured_count: 2,
        group: 'platform',
        group_name: '平台集成',
        last_applied_at: null,
        last_error_summary: '最近测试连接失败',
        last_modified_at: 1_735_732_800_000,
        last_tested_at: 1_735_732_800_000,
        name: 'SMTP 邮件',
        operation_path: null,
        published_version: null,
        runtime_status: 'error'
      }
    ],
    summary: { normal: 8, pending_apply: 1, runtime_error: 1, total: 13, unconfigured: 3 },
    total: 2
  };
}

function renderPage() {
  return render(<MemoryRouter><ConfigCenterPage /></MemoryRouter>);
}

function selectByLabel(label: string): HTMLElement {
  const field = screen.getByText(label).closest('label');
  const select = field?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

async function chooseOption(user: ReturnType<typeof userEvent.setup>, label: string, optionLabel: string) {
  await user.click(selectByLabel(label));
  const option = await waitFor(() => {
    const match = [...document.querySelectorAll('.semi-select-option')].find(
      (item) => item.textContent === optionLabel
    ) as HTMLElement | undefined;
    expect(match).toBeDefined();
    return match as HTMLElement;
  });
  fireEvent.mouseDown(option);
  fireEvent.mouseUp(option);
  fireEvent.click(option);
}

describe('ConfigCenterPage', () => {
  beforeEach(() => {
    apiRequestMock.mockReset();
    apiRequestMock.mockResolvedValue(response());
  });

  it('renders backend-authoritative status, versions, and workflow links', async () => {
    renderPage();

    expect(screen.getByText('正在聚合配置状态…')).toBeInTheDocument();
    await waitFor(() => expect(apiRequestMock).toHaveBeenCalledWith('/admin/api/v1/config-center'));
    expect(await screen.findByText('预测配置')).toBeInTheDocument();
    expect(screen.getByText('发布 v3 / 已应用 v2')).toBeInTheDocument();
    expect(screen.getAllByText('待应用')).toHaveLength(2);
    expect(screen.getByText('最近测试连接失败')).toBeInTheDocument();
    expect(screen.getAllByRole('link', { name: '进入配置' })[0]).toHaveAttribute('href', '/admin/prediction/settings');
    expect(screen.getByRole('link', { name: '运行与处置' })).toHaveAttribute('href', '/admin/prediction/sync');
    expect(screen.getByRole('region', { name: '配置状态摘要' })).toHaveTextContent('13');
  });

  it('submits trimmed search and group/status filters to the aggregation API', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('预测配置');

    await user.type(screen.getByLabelText('搜索配置'), ' 预测 ');
    await chooseOption(user, '业务分组', '行情与交易');
    await chooseOption(user, '配置状态', '待应用');
    await user.click(screen.getByRole('button', { name: /查询/ }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenLastCalledWith(
        '/admin/api/v1/config-center?query=%E9%A2%84%E6%B5%8B&group=market&status=pending_apply'
      );
    });
  });

  it('shows a safe error and retries without leaking a stack trace', async () => {
    const user = userEvent.setup();
    const error = new Error('network down secret="dont-leak"\nprivate stack');
    apiRequestMock.mockRejectedValueOnce(error).mockResolvedValueOnce(response());
    renderPage();

    expect(await screen.findByText('network down secret=***')).toBeInTheDocument();
    expect(screen.queryByText(/dont-leak/)).not.toBeInTheDocument();
    expect(screen.queryByText(/private stack/)).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '重新加载' }));
    expect(await screen.findByText('预测配置')).toBeInTheDocument();
    expect(apiRequestMock).toHaveBeenCalledTimes(2);
  });
});
