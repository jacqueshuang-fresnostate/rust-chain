import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { NewCoinActions } from './NewCoinActions';
import { optionalNewCoinLocalDateTimeMillis, requiredNewCoinLocalDateTimeMillis } from '../newCoinDateTime';
import { projectLocalTime } from '../new-coins/projectModel';
function LocationProbe(){const l=useLocation();return <output>{l.pathname+l.search}</output>;}
describe('NewCoinActions legacy entry',()=>{
  it.each([['?project_id=7001','/admin/new-coins/projects/7001'],['','/admin/new-coins/projects'],['?project_id=bad','/admin/new-coins/projects']])('redirects %s without a first-page lookup',(query,target)=>{
    render(<MemoryRouter initialEntries={[`/admin/new-coins/actions${query}`]}><Routes><Route path="/admin/new-coins/actions" element={<NewCoinActions/>}/><Route path="*" element={<LocationProbe/>}/></Routes></MemoryRouter>);
    expect(screen.getByRole('status')).toHaveTextContent(target);
  });
  it('preserves local datetime milliseconds and rejects invalid dates',()=>{
    const value=new Date(2026,10,10,9,15,30,250).getTime();
    expect(requiredNewCoinLocalDateTimeMillis(projectLocalTime(value),'上市时间')).toBe(value);
    expect(()=>requiredNewCoinLocalDateTimeMillis('','上市时间')).toThrow('上市时间不能为空');
    expect(()=>requiredNewCoinLocalDateTimeMillis('2026-02-30T10:00','上市时间')).toThrow();
    expect(()=>optionalNewCoinLocalDateTimeMillis('invalid','固定解禁时间')).toThrow();
    expect(optionalNewCoinLocalDateTimeMillis(' ','固定解禁时间')).toBeUndefined();
  });
});
