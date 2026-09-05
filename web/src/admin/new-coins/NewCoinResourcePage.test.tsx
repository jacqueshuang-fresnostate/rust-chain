import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import { AdminAccessProvider } from '../access';
import { NewCoinLocksPage, NewCoinResourcePage } from './NewCoinResourcePage';
import { resourceConfigs, type ResourceConfig } from '../resources/resourceConfigs';

// The shared list's fetching/paging is covered separately; observe only the
// mounted list identity and initial filters so lazy permission gates are explicit.
vi.mock('../resources/resourceConfigs', async () => ({
  ...await vi.importActual<typeof import('../resources/resourceConfigs')>('../resources/resourceConfigs'),
  ResourcePage: ({ config, initialFilters }: {config: ResourceConfig; initialFilters: Record<string,string>}) =>
    <div data-testid="mounted-resource">{JSON.stringify({endpoint:config.endpoint,initialFilters})}</div>
}));

function mount(permissions: string[]) {
  render(<MemoryRouter initialEntries={['/admin/new-coins/unlocks?asset_id=11&unknown=ignored&limit=999&offset=50']}>
    <AdminAccessProvider access={{admin_id:1,username:'reader',role_id:1,role_name:'reader',permissions,is_super_admin:false}}>
      <NewCoinLocksPage />
    </AdminAccessProvider>
  </MemoryRouter>);
}

describe('new coin workspaces', () => {
  it('mounts only the authorized unlock tab and applies whitelisted asset scope', () => {
    mount(['new_coin.unlocks.read']);
    expect(screen.queryByRole('tab',{name:'锁仓仓位'})).not.toBeInTheDocument();
    expect(JSON.parse(screen.getByTestId('mounted-resource').textContent ?? '')).toEqual({endpoint:'/admin/api/v1/new-coins/unlocks',initialFilters:{asset_id:'11'}});
  });
  it('replaces rather than eagerly fetching both lists and preserves asset scope', async () => {
    mount(['new_coin.unlocks.read','new_coin.locks.read']);
    expect(screen.getAllByTestId('mounted-resource')).toHaveLength(1);
    await userEvent.click(screen.getByRole('tab',{name:'锁仓仓位'}));
    await waitFor(()=>expect(screen.getAllByTestId('mounted-resource')).toHaveLength(1));
    expect(JSON.parse(screen.getByTestId('mounted-resource').textContent ?? '')).toEqual({endpoint:'/admin/api/v1/new-coins/lock-positions',initialFilters:{asset_id:'11'}});
  });
  it('keeps project and pending-status deep-link filters without accepting unknown keys', () => {
    render(<MemoryRouter initialEntries={['/admin/new-coins/subscriptions?project_id=7001&status=pending&arbitrary=x']}>
      <NewCoinResourcePage config={resourceConfigs.newCoinSubscriptions}/>
    </MemoryRouter>);
    expect(JSON.parse(screen.getByTestId('mounted-resource').textContent ?? '').initialFilters).toEqual({project_id:'7001',status:'pending'});
  });
});
