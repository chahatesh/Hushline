// SPDX-License-Identifier: MIT
import type { Statistics, Usage, NamedUsage, Transcript } from './types';

export function emptyUsage(): Usage { return {words:0,recordings:0,durationMs:0,latencyMs:0,inserted:0,longest:0,withLists:0,withParagraphs:0}; }
export function emptyStatistics(): Statistics { return {total:emptyUsage(),days:[],apps:[],hours:Array.from({length:24},emptyUsage),weekdays:Array.from({length:7},emptyUsage),profiles:[],topics:[],terms:[],unclassifiedWords:0,skippedRecords:0}; }
export function localDay(date: Date) { return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`; }
export function dayDate(key: string) { const [year,month,day]=key.split('-').map(Number); return new Date(year,month-1,day,12); }
export function addUsage(a: Usage, b: Usage): Usage { return {words:a.words+b.words,recordings:a.recordings+b.recordings,durationMs:a.durationMs+b.durationMs,latencyMs:a.latencyMs+b.latencyMs,inserted:a.inserted+b.inserted,longest:Math.max(a.longest,b.longest),withLists:a.withLists+b.withLists,withParagraphs:a.withParagraphs+b.withParagraphs}; }
export function streaks(days: NamedUsage[], now = new Date()) {
  const active = new Set(days.filter(d=>d.words>0).map(d=>d.name));
  const ordered=[...active].sort(); let longest=0,run=0,previous='';
  for(const key of ordered) { const date=dayDate(key); date.setDate(date.getDate()-1); run=localDay(date)===previous?run+1:1; longest=Math.max(longest,run); previous=key; }
  const cursor=new Date(now); cursor.setHours(12,0,0,0);
  if(!active.has(localDay(cursor))) cursor.setDate(cursor.getDate()-1);
  let current=0; while(active.has(localDay(cursor))) {current++;cursor.setDate(cursor.getDate()-1);}
  return {current,longest,active:active.size};
}
/** Sunday-first weeks, padded only for alignment. Noon avoids DST day shifts. */
export function calendarDays(year: number) {
  const start=new Date(year,0,1,12),end=new Date(year,11,31,12);
  const cells:(string|null)[]=Array.from({length:start.getDay()},()=>null);
  for(const cursor=new Date(start);cursor<=end;cursor.setDate(cursor.getDate()+1)) cells.push(localDay(cursor));
  while(cells.length%7) cells.push(null);
  return cells;
}
export function intensity(value: number, maximum: number) { if(!value)return 0; return Math.min(4,Math.max(1,Math.ceil(Math.sqrt(value/Math.max(1,maximum))*4))); }
export function trend(days: NamedUsage[], period: '30'|'90'|'all', today = new Date()) {
  const byDate=new Map(days.map(d=>[d.name,d]));
  const start=new Date(today); start.setHours(12,0,0,0);
  if(period==='all') { if(!days.length) return []; start.setTime(dayDate([...byDate.keys()].sort()[0]).getTime()); }
  else start.setDate(start.getDate()-Number(period)+1);
  const buckets=new Map<string,NamedUsage>();
  const monthly=period==='all';
  for(const date=new Date(start);localDay(date)<=localDay(today);date.setDate(date.getDate()+1)) {
    const key=monthly?localDay(date).slice(0,7):localDay(date);
    buckets.set(key,{name:key,...addUsage(buckets.get(key)||emptyUsage(),byDate.get(localDay(date))||emptyUsage())});
  }
  return [...buckets.values()];
}
/** Browser review only; native statistics always come from the complete SQLite history. */
export function previewStatistics(history: Transcript[]): Statistics {
  const result=emptyStatistics(),days=new Map<string,NamedUsage>(),apps=new Map<string,NamedUsage>();
  for(const record of history) {
    const date=new Date(record.createdAt); if(Number.isNaN(date.getTime())) {result.skippedRecords++;continue;}
    const usage:Usage={words:record.text.trim()?record.text.trim().split(/\s+/u).length:0,recordings:1,durationMs:record.durationMs,latencyMs:record.latencyMs,inserted:Number(record.inserted),longest:record.words,withLists:Number(/^\s*(?:[•*-]|\d+\.)\s/mu.test(record.text)),withParagraphs:Number(record.text.includes('\n\n'))};
    result.total=addUsage(result.total,usage); const name=localDay(date),app=record.app||'Unknown application';
    days.set(name,{name,...addUsage(days.get(name)||emptyUsage(),usage)}); apps.set(app,{name:app,...addUsage(apps.get(app)||emptyUsage(),usage)});
    result.hours[date.getHours()]=addUsage(result.hours[date.getHours()],usage); const day=(date.getDay()+6)%7; result.weekdays[day]=addUsage(result.weekdays[day],usage);
  }
  result.days=[...days.values()].sort((a,b)=>a.name.localeCompare(b.name)); result.apps=[...apps.values()].sort((a,b)=>b.words-a.words); return result;
}
