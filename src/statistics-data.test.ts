// SPDX-License-Identifier: MIT
import { describe,it,expect } from 'vitest';
import { calendarDays,localDay,streaks,intensity,trend,emptyUsage,previewStatistics } from './statistics-data';

describe('all-time statistics',()=>{
  it('aligns every calendar day including leap years without a DST-sized day',()=>{
    for(const year of [2023,2024,2026]) {
      const days=calendarDays(year).filter((d):d is string=>d!==null);
      expect(days.length).toBe(year===2024?366:365);
      expect(new Set(days).size).toBe(days.length);
      expect(days[0]).toBe(`${year}-01-01`);expect(days.at(-1)).toBe(`${year}-12-31`);
      expect(calendarDays(year).length%7).toBe(0);
    }
  });
  it('keeps older years and zero days in an all-time trend',()=>{
    const days=[{...emptyUsage(),name:'2020-01-01',words:50},{...emptyUsage(),name:'2026-10-28',words:100}];
    const months=trend(days,'all',new Date(2026,9,30));
    expect(months[0].name).toBe('2020-01');expect(months[0].words).toBe(50);expect(months.length).toBe(82);expect(months.at(-1)?.words).toBe(100);
    const recent=trend(days,'30',new Date(2026,9,30));expect(recent.length).toBe(30);expect(recent.reduce((sum,d)=>sum+d.words,0)).toBe(100);
  });
  it('calculates streaks across year boundaries and allows today to be quiet',()=>{
    const days=['2025-12-30','2025-12-31','2026-01-01','2026-01-03'].map(name=>({...emptyUsage(),name,words:1}));
    expect(streaks(days,new Date(2026,0,4))).toEqual({current:1,longest:3,active:4});
    expect(streaks(days,new Date(2026,0,6)).current).toBe(0);
  });
  it('intensity is ordered, zero is empty, and local dates avoid UTC truncation',()=>{
    expect([0,1,10,50,100].map(n=>intensity(n,100))).toEqual([0,1,2,3,4]);
    expect(localDay(new Date(2026,2,8,23,59))).toBe('2026-03-08');
  });
  it('does not invent profiles or activity in a browser review',()=>{
    expect(previewStatistics([])).toMatchObject({total:{words:0,recordings:0},profiles:[],days:[],topics:[]});
  });
});
