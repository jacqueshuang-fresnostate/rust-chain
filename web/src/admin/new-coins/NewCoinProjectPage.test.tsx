import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { apiRequest, ApiError } from '../../api/client';
import { AdminAccessProvider } from '../access';
import { NewCoinProjectPage } from './NewCoinProjectPage';
import { center } from './__tests__/fixtures';
import { loadProjectCenter, projectLocalTime, type ProjectCenter } from './projectModel';
vi.mock('../../api/client',async()=>({...await vi.importActual<typeof import('../../api/client')>('../../api/client'),apiRequest:vi.fn()}));
const request=vi.mocked(apiRequest);
function mount(data:ProjectCenter,permissions=['*']){
 request.mockImplementation(async (path, options)=>{
  if(options?.method==='PATCH') return data.project;
  if(path===`/admin/api/v1/new-coins/${data.project.id}`) return data;
  if(path.startsWith('/admin/api/v1/market/pairs')) return {pairs:[],total:0};
  if(path.startsWith('/admin/api/v1/users')) return {users:[],total:0};
  throw new Error(`Unexpected request ${path}`);
 });
 const router=createMemoryRouter([{path:'/admin/new-coins/projects/:projectId',element:<NewCoinProjectPage/>},{path:'/admin/new-coins/projects',element:<div>Project list destination</div>}],{initialEntries:[`/admin/new-coins/projects/${data.project.id}`]});
 render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><AdminAccessProvider access={{admin_id:1,username:'operator',role_id:1,role_name:'operator',permissions,is_super_admin:false}}><RouterProvider router={router}/></AdminAccessProvider></QueryClientProvider>);
 return router;
}
async function selectControl(label:string, option:string){
 const wrapper=[...document.querySelectorAll('label')].find(e=>e.textContent?.startsWith(label));
 const control=wrapper?.querySelector('.semi-select');expect(control).toBeInTheDocument();
 await userEvent.click(control as HTMLElement);
 await waitFor(()=>expect([...document.querySelectorAll('.semi-select-option')].some(e=>e.textContent===option)).toBe(true));
 await userEvent.click([...document.querySelectorAll('.semi-select-option')].find(e=>e.textContent===option) as HTMLElement);
}
async function selectCategory(label:string){await selectControl('配置分类',label);}

beforeEach(()=>{ request.mockReset(); });
describe('new coin project center',()=>{
 it('loads by exact ID beyond the reference page and exposes only the next command',async()=>{
  mount(center('preheat',7001));await screen.findByText('HIP · 项目中心');
  expect(screen.getByRole('button',{name:'开始申购'})).toBeEnabled();expect(screen.queryByRole('button',{name:'确认上市'})).not.toBeInTheDocument();
  expect(request.mock.calls.every(([path])=>path==='/admin/api/v1/new-coins/7001')).toBe(true);
 });
 it('hydrates issuance, requires reason, submits original values and guards dirty navigation',async()=>{
  const data=center();mount(data);await screen.findByText('HIP · 项目中心');await userEvent.click(screen.getByRole('tab',{name:'项目配置'}));
  expect(screen.getByLabelText('发行总量')).toHaveValue('100');expect(screen.getByLabelText('发行价')).toHaveValue('2.5');
  fireEvent.change(screen.getByLabelText('发行价'),{target:{value:'3.25'}});expect(screen.getByRole('button',{name:'开始申购'})).toBeDisabled();
  await userEvent.click(screen.getByRole('link',{name:'返回项目管理'}));await screen.findByText('确认离开当前页面');await userEvent.click(screen.getByLabelText('继续编辑'));
  await userEvent.click(screen.getByRole('button',{name:'保存当前配置'}));await userEvent.type(screen.getByLabelText('操作原因'),'correct price');await userEvent.click(screen.getByRole('button',{name:'确认'}));
  await waitFor(()=>expect(request.mock.calls.some(([,r])=>r?.method==='PATCH')).toBe(true));
  const call=request.mock.calls.find(([,r])=>r?.method==='PATCH');expect(call?.[0]).toBe('/admin/api/v1/new-coins/7/issuance');
  expect(JSON.parse(String(call?.[1]?.body))).toEqual({total_supply:'100',issue_price:'3.25',expected_total_supply:'100',expected_issue_price:'2.5',expected_config:'snapshot-v1',reason:'correct price'});
 });
 it('hydrates actual unlock timestamps and clears inactive rule fields in the payload',async()=>{
  const data=center();mount(data);await screen.findByText('HIP · 项目中心');await userEvent.click(screen.getByRole('tab',{name:'项目配置'}));await selectCategory('解禁规则');
  expect(screen.getByLabelText('固定解禁时间')).toHaveValue(projectLocalTime(data.project.fixed_unlock_at));
  await selectControl('解禁类型','相对周期解禁');
  fireEvent.change(screen.getByLabelText('相对周期秒数'),{target:{value:'86400'}});
  await userEvent.click(screen.getByRole('button',{name:'保存当前配置'}));await userEvent.type(screen.getByLabelText('操作原因'),'future locks');await userEvent.click(screen.getByRole('button',{name:'确认'}));
  await waitFor(()=>expect(request.mock.calls.some(([,r])=>r?.method==='PATCH')).toBe(true));
  const body=JSON.parse(String(request.mock.calls.find(([,r])=>r?.method==='PATCH')?.[1]?.body));expect(body).toEqual({unlock_type:'relative_period',relative_unlock_seconds:86400,expected_config:'snapshot-v1',reason:'future locks'});
 });
 it('preserves a rejected draft and explicitly reloads a configuration conflict',async()=>{
  const data=center();mount(data);await screen.findByText('HIP · 项目中心');await userEvent.click(screen.getByRole('tab',{name:'项目配置'}));
  fireEvent.change(screen.getByLabelText('发行价'),{target:{value:'3.25'}});
  request.mockImplementation(async(path,options)=>{if(options?.method==='PATCH')throw new ApiError(409,'CONFLICT','stale configuration');if(path.endsWith('/7'))return {...data,configuration_version:'snapshot-v2',project:{...data.project,issue_price:'4'}};throw new Error(path);});
  await userEvent.click(screen.getByRole('button',{name:'保存当前配置'}));await userEvent.type(screen.getByLabelText('操作原因'),'keep draft');await userEvent.click(screen.getByRole('button',{name:'确认'}));
  await waitFor(()=>expect(request.mock.calls.some(([,r])=>r?.method==='PATCH')).toBe(true));
  await screen.findByRole('alert');expect(screen.getByLabelText('操作原因')).toHaveValue('keep draft');expect(screen.getByLabelText('发行价')).toHaveValue('3.25');
  await userEvent.click(screen.getByLabelText('取消'));await userEvent.click(screen.getByRole('button',{name:'丢弃草稿并加载最新配置'}));
  await waitFor(()=>expect(screen.getByLabelText('发行价')).toHaveValue('4'));expect(screen.getByRole('button',{name:'保存当前配置'})).toBeDisabled();
 });
 it('hydrates purchase configuration and disables only project purchasing without requesting unauthorized pair data',async()=>{
  const data=center('listed');data.project.post_listing_purchase_enabled=true;data.project.post_listing_pair_id=9001;
  mount(data,['new_coin.projects.read','new_coin.projects.write']);await screen.findByText('HIP · 项目中心');await userEvent.click(screen.getByRole('tab',{name:'项目配置'}));await selectCategory('上市后购买');
  expect(screen.getByRole('checkbox',{name:'启用上市后购买'})).toBeChecked();expect(screen.getByText(/关闭只停止本项目/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('checkbox',{name:'启用上市后购买'}));await userEvent.click(screen.getByRole('button',{name:'保存当前配置'}));await userEvent.type(screen.getByLabelText('操作原因'),'stop purchases');await userEvent.click(screen.getByRole('button',{name:'确认'}));
  await waitFor(()=>expect(request.mock.calls.some(([,r])=>r?.method==='PATCH')).toBe(true));const call=request.mock.calls.find(([,r])=>r?.method==='PATCH');
  expect(call?.[0]).toBe('/admin/api/v1/new-coins/7/post-listing-purchase');expect(JSON.parse(String(call?.[1]?.body))).toEqual({enabled:false,expected_config:'snapshot-v1',reason:'stop purchases'});
  expect(request.mock.calls.some(([path])=>path.includes('/market/pairs'))).toBe(false);
 });
 it('blocks listing with unsettled obligations and hides unauthorized write and reference requests',async()=>{
  const data={...center('distribution'),pending_manual_count:1,lifecycle_block_reason:'仍有待派发或待退款申购，请先完成结算'};
  mount(data,['new_coin.projects.read']);await screen.findByText('HIP · 项目中心');expect(screen.queryByRole('button',{name:'确认上市'})).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('tab',{name:'配置详情'}));expect(screen.getByRole('button',{name:'保存当前配置'})).toBeDisabled();expect(screen.queryByRole('tab',{name:'额外赠币'})).not.toBeInTheDocument();
  expect(request.mock.calls).toHaveLength(1);
 });
 it('refuses malformed decimal configuration rather than rendering writable defaults',async()=>{
  request.mockResolvedValue({...center(),project:{...center().project,issue_price:2.5}});
  await expect(loadProjectCenter('7')).rejects.toThrow('新币项目配置响应不完整');
  request.mockRejectedValue(new ApiError(404,'NOT_FOUND','not found'));await expect(loadProjectCenter('7')).rejects.toThrow();
 });
});

it('separates planned time from the server-owned actual listing command',async()=>{
 const data=center('distribution');mount(data);await screen.findByText('HIP · 项目中心');
 expect(screen.getByText(/尚未确认上市/)).toBeInTheDocument();
 expect(screen.getByText(/计划时间不自动推进阶段/)).toBeInTheDocument();
 await userEvent.click(screen.getByRole('button',{name:'确认上市'}));
 await userEvent.type(screen.getByLabelText('操作原因'),'confirm actual listing');await userEvent.click(screen.getByRole('button',{name:'确认'}));
 await waitFor(()=>expect(request.mock.calls.some(([,r])=>r?.method==='PATCH')).toBe(true));
 expect(JSON.parse(String(request.mock.calls.find(([,r])=>r?.method==='PATCH')?.[1]?.body))).toEqual({lifecycle_status:'listed',expected_config:'snapshot-v1',reason:'confirm actual listing'});
});
it('does not invent a historic listing event and preserves exact planned milliseconds',async()=>{
 const data=center('listed');data.project.actual_listed_at=null;data.project.unlock_type='immediate_on_listing';data.project.listed_at=1794309753250;
 mount(data);await screen.findByText('HIP · 项目中心');expect(screen.getByText(/历史事件未记录/)).toBeInTheDocument();
 await userEvent.click(screen.getByRole('tab',{name:'项目配置'}));await selectCategory('解禁规则');
 expect(screen.getByLabelText('计划上市时间')).toHaveValue(projectLocalTime(data.project.listed_at));expect(screen.queryByLabelText('实际上市时间')).not.toBeInTheDocument();
 expect(screen.getByText(/已形成的上市门禁不受后续计划或规则修改影响/)).toBeInTheDocument();
 const changed=1794309753789;fireEvent.change(screen.getByLabelText('计划上市时间'),{target:{value:projectLocalTime(changed)}});
 await userEvent.click(screen.getByRole('button',{name:'保存当前配置'}));await userEvent.type(screen.getByLabelText('操作原因'),'plan only');await userEvent.click(screen.getByRole('button',{name:'确认'}));
 await waitFor(()=>expect(request.mock.calls.some(([,r])=>r?.method==='PATCH')).toBe(true));
 expect(JSON.parse(String(request.mock.calls.find(([,r])=>r?.method==='PATCH')?.[1]?.body))).toEqual({unlock_type:'immediate_on_listing',listed_at:changed,expected_config:'snapshot-v1',reason:'plan only'});
});
it('fails closed if the actual listing field is absent',async()=>{
 const data=center();delete (data.project as Record<string,unknown>).actual_listed_at;request.mockResolvedValue(data);
 await expect(loadProjectCenter('7')).rejects.toThrow('新币项目配置响应不完整');
});
