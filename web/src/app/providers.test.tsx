import { ConfigConsumer } from '@douyinfe/semi-ui';
import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AppProviders } from './providers';

describe('AppProviders', () => {
  it('provides Semi Chinese locale and Shanghai timezone', () => {
    render(
      <AppProviders>
        <ConfigConsumer>
          {(config) => (
            <div>
              <span data-testid="timezone">{config.timeZone}</span>
              <span data-testid="pagination-page-size">{config.locale?.Pagination?.pageSize}</span>
              <span data-testid="table-empty">{config.locale?.Table?.emptyText}</span>
            </div>
          )}
        </ConfigConsumer>
      </AppProviders>
    );

    expect(screen.getByTestId('timezone')).toHaveTextContent('Asia/Shanghai');
    expect(screen.getByTestId('pagination-page-size')).toHaveTextContent('每页条数');
    expect(screen.getByTestId('table-empty')).toHaveTextContent('暂无数据');
  });

  it('enables Semi theme mode on the document body', async () => {
    render(
      <AppProviders>
        <div>theme target</div>
      </AppProviders>
    );

    await waitFor(() => expect(document.body).toHaveAttribute('theme-mode', 'light'));
  });
});
