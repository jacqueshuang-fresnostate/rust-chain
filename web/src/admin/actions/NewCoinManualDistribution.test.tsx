import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { apiRequest } from '../../api/client';
import { NewCoinManualDistribution } from './NewCoinManualDistribution';
import { center, manualOrder } from '../new-coins/__tests__/fixtures';
vi.mock('../../api/client',async()=>({...await vi.importActual<typeof import('../../api/client')>('../../api/client'),apiRequest:vi.fn()}));
const request=vi.mocked(apiRequest);
function mount(order=manualOrder){const reload=vi.fn();render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><NewCoinManualDistribution order={order} onSettled={reload}/></QueryClientProvider>);return reload;}
async function open(){await userEvent.click(screen.getByRole('button',{name:'派发申购 91'}));await waitFor(()=>expect(request).toHaveBeenCalledWith('/admin/api/v1/new-coins/7',expect.anything()));await waitFor(()=>expect(screen.queryByText('正在核对项目阶段…')).not.toBeInTheDocument());}
beforeEach(()=>{request.mockReset();request.mockImplementation(async(_p,options)=>options?.method==='POST'?{id:1}:center('distribution'));});
describe('manual order settlement',()=>{
 it('previews exact partial refund, derives identity from row and reloads after success',async()=>{
  const reload=mount();await open();fireEvent.change(screen.getByLabelText('最终派发数量'),{target:{value:'4'}});
  expect(screen.getByText(/实际扣款：10；退回差额：15/)).toBeInTheDocument();
  await userEvent.type(screen.getByLabelText('操作原因'),'partial allocation');await userEvent.click(screen.getByRole('button',{name:'确认派发并退差额'}));
  await waitFor(()=>expect(reload).toHaveBeenCalledOnce());
  const call=request.mock.calls.find(([,r])=>r?.method==='POST');expect(call?.[0]).toBe('/admin/api/v1/new-coins/7/distribute');
  expect(JSON.parse(String(call?.[1]?.body))).toEqual({user_id:42,subscription_id:91,quantity:'4',reason:'partial allocation',idempotency_key:expect.any(String)});
 });
 it('rejects negative/excess, supports zero and keeps the key on uncertain-result retries',async()=>{
  mount();await open();await userEvent.type(screen.getByLabelText('操作原因'),'refund');
  for(const value of ['-1','11']){fireEvent.change(screen.getByLabelText('最终派发数量'),{target:{value}});expect(screen.getByRole('button',{name:'确认派发并退差额'})).toBeDisabled();}
  fireEvent.change(screen.getByLabelText('最终派发数量'),{target:{value:'0'}});expect(screen.getByText(/实际扣款：0；退回差额：25/)).toBeInTheDocument();
  request.mockRejectedValueOnce(new Error('network interrupted'));
  await userEvent.click(screen.getByRole('button',{name:'确认派发并退差额'}));await screen.findByText(/network interrupted/);
  // Semi Modal deliberately debounces repeated OK clicks for 100 ms.
  await new Promise(resolve=>setTimeout(resolve,120));
  await userEvent.click(screen.getByRole('button',{name:'确认派发并退差额'}));await waitFor(()=>expect(request.mock.calls.filter(([,r])=>r?.method==='POST')).toHaveLength(2));
  const calls=request.mock.calls.filter(([,r])=>r?.method==='POST');expect(calls[0][1]?.body).toBe(calls[1][1]?.body);
 });
 it('does not expose a new confirmation for legacy or settled orders',()=>{mount({...manualOrder,settlement_mode:'legacy_instant'});expect(screen.queryByRole('button',{name:'派发申购 91'})).not.toBeInTheDocument();expect(request).not.toHaveBeenCalled();});
 it('blocks settlement during subscription even when amount and reason are valid',async()=>{request.mockResolvedValue(center('subscription'));mount();await open();fireEvent.change(screen.getByLabelText('最终派发数量'),{target:{value:'4'}});await userEvent.type(screen.getByLabelText('操作原因'),'test');expect(screen.getByRole('button',{name:'确认派发并退差额'})).toBeDisabled();});
});
