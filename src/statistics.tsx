// SPDX-License-Identifier: MIT
import { createSignal, createMemo, createEffect, onCleanup, onMount, For, Show } from 'solid-js';
import { listen } from '@tauri-apps/api/event';
import Download from 'lucide-solid/icons/download';
import RotateCcw from 'lucide-solid/icons/rotate-ccw';
import ShieldCheck from 'lucide-solid/icons/shield-check';
import { call, desktop, download, errorText } from './bridge';
import { calendarDays, dayDate, emptyStatistics, emptyUsage, intensity, localDay, streaks, trend } from './statistics-data';
import type { Statistics } from './types';

const count = (n: number) => n.toLocaleString();
const minutes = (ms: number) => (ms/60000).toLocaleString(undefined,{maximumFractionDigits:1});
const seconds = (ms: number) => `${(ms/1000).toFixed(2)} s`;
function Metric(props: {label: string; value: string; detail: string}) { return <div><span class="stat-label">{props.label}</span><strong>{props.value}</strong><span>{props.detail}</span></div>; }

export function StatisticsPage(props: {revision: number; privateMode: boolean}) {
  const [stats,setStats]=createSignal<Statistics>(emptyStatistics());
  const [loading,setLoading]=createSignal(true),[error,setError]=createSignal('');
  const [year,setYear]=createSignal(new Date().getFullYear());
  const [selected,setSelected]=createSignal(localDay(new Date()));
  const [period,setPeriod]=createSignal<'30'|'90'|'all'>('all');
  const [profile,setProfile]=createSignal<number|null>(null);
  let request=0, live=true;
  async function refresh() {
    const ticket=++request; setLoading(true); setError('');
    try { const value=await call<Statistics>('usage_statistics'); if(live&&ticket===request) setStats(value); }
    catch(e) {if(live&&ticket===request)setError(errorText(e));}
    finally {if(live&&ticket===request)setLoading(false);}
  }
  // Parent views also receive live microphone levels. Only a changed history
  // revision may trigger disk analysis, never a meter frame or timer tick.
  const historyRevision=createMemo(()=>props.revision);
  createEffect(()=>{void historyRevision();void refresh();});
  onMount(()=>{if(desktop){const dispose=listen('insights-updated',()=>void refresh());onCleanup(()=>void dispose.then(fn=>fn()));}});
  onCleanup(()=>{live=false;request++;});
  const days=createMemo(()=>new Map(stats().days.map(day=>[day.name,day])));
  const years=createMemo(()=>{const first=Number(stats().days[0]?.name.slice(0,4)||new Date().getFullYear());return Array.from({length:Math.max(1,new Date().getFullYear()-first+1)},(_,i)=>new Date().getFullYear()-i);});
  const calendar=createMemo(()=>calendarDays(year()));
  const maximum=createMemo(()=>Math.max(1,...stats().days.filter(day=>Number(day.name.slice(0,4))===year()).map(day=>day.words)));
  const selectedUsage=()=>days().get(selected())||emptyUsage();
  const activity=createMemo(()=>streaks(stats().days));
  const graph=createMemo(()=>trend(stats().days,period()));
  const graphMax=createMemo(()=>Math.max(1,...graph().map(day=>day.words)));
  const chosenProfile=()=>stats().profiles.find(p=>p.throughWord===profile())||stats().profiles.at(-1);
  const profileTopics=()=>chosenProfile()?.topics||[];
  const profileTerms=()=>chosenProfile()?.terms||[];
  const matched=()=>profileTopics().reduce((sum,t)=>sum+t.count,0);
  const nextProfile=()=>Math.floor(stats().total.words/10000)*10000+10000;
  function selectYear(value: number) {setYear(value);const last=stats().days.filter(d=>Number(d.name.slice(0,4))===value).at(-1);setSelected(last?.name||(value===new Date().getFullYear()?localDay(new Date()):`${value}-01-01`));}
  function moveCell(event: KeyboardEvent,index: number) {
    const moves: Record<string,number>={ArrowLeft:-7,ArrowRight:7,ArrowUp:-1,ArrowDown:1};
    const valid=calendar().map((key,index)=>key&&key<=localDay(new Date())?index:-1).filter(index=>index>=0);
    const first=valid[0]||0,last=valid.at(-1)||first;
    let next=event.key==='Home'?first:event.key==='End'?last:index+(moves[event.key]||0);
    if(!(event.key in moves)&&event.key!=='Home'&&event.key!=='End')return;
    event.preventDefault();next=Math.max(first,Math.min(last,next));
    const step=next<index?-1:1;
    while(next>=0&&next<calendar().length&&(!calendar()[next]||(calendar()[next]||'')>localDay(new Date())))next+=step;
    const key=calendar()[next];if(key&&key<=localDay(new Date())){setSelected(key);document.getElementById(`activity-${key}`)?.focus();}
  }
  const exportStats=()=>download('hush-statistics.json',JSON.stringify({version:1,exportedAt:new Date().toISOString(),scope:'All retained history; private and deleted records excluded',...stats()},null,2));
  return <div class="statistics-page" aria-busy={loading()}>
    <div class="insight-toolbar"><p><ShieldCheck size={15}/> All time · on this computer{props.privateMode?' · private sessions excluded':''}</p><div class="button-row"><button class="quiet" disabled={loading()} onClick={()=>void refresh()} aria-label="Refresh statistics"><RotateCcw size={15}/></button><button class="secondary" disabled={loading()||!!error()} onClick={exportStats}><Download size={15}/> Export statistics</button></div></div>
    <Show when={error()}><div role="alert" class="notice"><span>{error()}</span><button class="text-button" onClick={()=>void refresh()}>Try again</button></div></Show>
    <Show when={loading()}><p role="status" class="field-note">Updating your all-time statistics…</p></Show>
    <div class="stat-strip standalone insight-totals">
      <Metric label="WORDS SAVED" value={count(stats().total.words)} detail={`${count(stats().total.recordings)} saved transcripts`}/>
      <Metric label="TIME SPEAKING" value={`${minutes(stats().total.durationMs)} min`} detail="Recorded audio duration"/>
      <Metric label="ACTIVE DAYS" value={count(activity().active)} detail={`Since ${stats().days[0]?dayDate(stats().days[0].name).toLocaleDateString():'your first dictation'}`}/>
      <Metric label="LONGEST STREAK" value={`${count(activity().longest)} days`} detail={`${activity().current} day current streak`}/>
    </div>
    <section class="panel activity-panel" aria-label="Daily dictation activity">
      <div class="section-title"><div><h2>Your year in words</h2><p>Every square is a day. More words, deeper color.</p></div><label class="year-picker"><span class="sr-only">Activity year</span><select aria-label="Activity year" value={year()} onChange={e=>selectYear(Number(e.currentTarget.value))}><For each={years()}>{y=><option value={y}>{y}</option>}</For></select></label></div>
      <div class="activity-scroll"><div class="activity-calendar"><div class="activity-months" aria-hidden="true"><For each={Array.from({length:12},(_,i)=>i)}>{month=><span>{new Date(year(),month,1).toLocaleDateString(undefined,{month:'short'})}</span>}</For></div><div class="activity-body"><div class="activity-weekdays" aria-hidden="true"><span>Sun</span><span>Mon</span><span>Tue</span><span>Wed</span><span>Thu</span><span>Fri</span><span>Sat</span></div><div class="activity-grid" role="group" aria-label={`Daily word counts for ${year()}`}><For each={calendar()}>{(key,index)=><Show when={key} fallback={<i class="activity-pad"/>}><button id={`activity-${key}`} classList={{'activity-cell':true,selected:selected()===key}} data-level={intensity(days().get(key!)?.words||0,maximum())} disabled={key!>localDay(new Date())} tabIndex={selected()===key?0:-1} title={`${key}: ${count(days().get(key!)?.words||0)} words`} aria-label={`${key}: ${count(days().get(key!)?.words||0)} words`} aria-pressed={selected()===key} onClick={()=>setSelected(key!)} onKeyDown={e=>moveCell(e,index())}/></Show>}</For></div></div></div></div>
      <div class="activity-footer"><span>{stats().days.filter(d=>d.name.startsWith(String(year()))&&d.words>0).length} active days in {year()}</span><span class="activity-legend">Less <For each={[0,1,2,3,4]}>{level=><i class="activity-cell" data-level={level}/>}</For> More</span></div>
      <div class="day-detail" aria-live="polite"><strong>{dayDate(selected()).toLocaleDateString(undefined,{weekday:'long',month:'short',day:'numeric',year:'numeric'})}</strong><span>{count(selectedUsage().words)} words</span><span>{selectedUsage().recordings} transcripts</span><span>{minutes(selectedUsage().durationMs)} min speaking</span></div>
    </section>
    <section class="panel trend-panel"><div class="section-title"><div><h2>Words over time</h2><p>{period()==='all'?'Monthly totals from your very first saved transcript.':'Daily totals, including quiet days.'}</p></div><select aria-label="Chart time range" value={period()} onChange={e=>setPeriod(e.currentTarget.value as '30'|'90'|'all')}><option value="all">All time · by month</option><option value="90">Last 90 days</option><option value="30">Last 30 days</option></select></div><Show when={stats().days.length>0} fallback={<div class="insight-empty"><h3>Your first words start the chart.</h3><p>Dictate with history on to start keeping a local record.</p></div>}><div class="trend-scroll"><div class="word-trend" role="img" aria-label={`${count(graph().reduce((sum,day)=>sum+day.words,0))} words across ${graph().length} ${period()==='all'?'months':'days'}`}><For each={graph()}>{day=><div title={`${day.name}: ${count(day.words)} words`}><i style={{height:`${day.words/graphMax()*100}%`}}/><small>{day.name.slice(period()==='all'?0:5)}</small></div>}</For></div></div><details class="insight-table"><summary>View chart data</summary><table><thead><tr><th>{period()==='all'?'Month':'Day'}</th><th>Words</th><th>Transcripts</th><th>Minutes speaking</th></tr></thead><tbody><For each={graph()}>{day=><tr><td>{day.name}</td><td>{count(day.words)}</td><td>{count(day.recordings)}</td><td>{minutes(day.durationMs)}</td></tr>}</For></tbody></table></details></Show></section>
    <section class="panel voice-profile" aria-label="Local voice profile"><div class="section-title"><div><span class="eyebrow">YOUR WORDS, TAKING SHAPE</span><h2>Your voice profile</h2><p>A new snapshot for every 10,000 saved words.</p></div><span class="tag">LOCAL ONLY</span></div><div class="profile-progress"><div><strong>{count(stats().total.words%10000)} / 10,000</strong><span>Next profile at {count(nextProfile())} words</span></div><progress max={10000} value={stats().total.words%10000} aria-label="Progress to next voice profile"/></div>
      <Show when={chosenProfile()} fallback={<div class="insight-empty"><h3>Your first profile is on its way.</h3><p>After 10,000 words, see the topics and vocabulary that show up most in your writing.</p></div>}>{p=><><div class="profile-selector"><label>Profile snapshot<select aria-label="Profile snapshot" value={p().throughWord} onChange={e=>setProfile(Number(e.currentTarget.value))}><For each={[...stats().profiles].reverse()}>{item=><option value={item.throughWord}>{count(item.fromWord)}–{count(item.throughWord)} words · {new Date(item.completedAt).toLocaleDateString()}</option>}</For></select></label><p>{profileTopics()[0]?<>Most recognized topic words in this block relate to <strong>{profileTopics()[0].name.toLowerCase()}</strong>.</>:'There are not enough recognized topic words in this block to estimate a main topic.'}</p></div><div class="profile-evidence"><div><h3>What you talk about</h3><For each={profileTopics()}>{topic=><div class="topic-row"><span>{topic.name}</span><span class="topic-track"><i style={{width:`${topic.count/Math.max(1,matched())*100}%`}}/></span><strong>{Math.round(topic.count/Math.max(1,matched())*100)}%</strong></div>}</For></div><div><h3>Recurring vocabulary</h3><div class="term-cloud"><For each={profileTerms()}>{term=><span>{term.name}<small>{count(term.count)}</small></span>}</For></div></div></div></>}</Show>
      <p class="field-note">A writing profile, based on local English keyword matching. Percentages describe recognized topic words, not everything you said. Other languages and unmatched words remain unclassified. This does not identify your voice or infer personal traits. No history is sent to an AI provider for profiles.</p>
    </section>
    <div class="insight-columns"><section class="panel app-usage"><div class="section-title"><h2>Where you dictate</h2><span class="tag">ALL TIME</span></div><Show when={stats().apps.length} fallback={<p class="field-note">Applications appear after your first saved dictation.</p>}><For each={stats().apps.slice(0,8)}>{app=><div class="app-usage-row"><span title={app.name}>{app.name}</span><strong>{count(app.words)}</strong><i style={{width:`${app.words/Math.max(1,stats().total.words)*100}%`}}/></div>}</For><Show when={stats().apps.length>8}><p class="field-note">Showing your top 8 applications. Export includes every application.</p></Show></Show></section>
    <section class="panel rhythm-panel"><div class="section-title"><h2>Your daily rhythm</h2><span class="tag">ALL TIME</span></div><p class="field-note">Words by local hour</p><div class="hour-chart" role="img" aria-label="Word counts by hour of day"><For each={stats().hours}>{(hour,index)=><div title={`${String(index()).padStart(2,'0')}:00 — ${count(hour.words)} words`}><i style={{height:`${hour.words/Math.max(1,...stats().hours.map(h=>h.words))*100}%`}}/></div>}</For></div><div class="hour-labels"><span>00</span><span>06</span><span>12</span><span>18</span><span>23</span></div><div class="weekday-totals"><For each={stats().weekdays}>{(day,index)=><div><span>{['Mon','Tue','Wed','Thu','Fri','Sat','Sun'][index()]}</span><strong>{count(day.words)}</strong></div>}</For></div></section></div>
    <section class="panel writing-metrics"><div class="section-title"><h2>How your words flow</h2><span class="tag">ALL TIME</span></div><div class="metrics-grid"><Metric label="WORDS PER TRANSCRIPT" value={stats().total.recordings?count(Math.round(stats().total.words/stats().total.recordings)):'—'} detail="Average saved length"/><Metric label="SPEAKING PACE" value={stats().total.durationMs?`${Math.round(stats().total.words/(stats().total.durationMs/60000))} wpm`:'—'} detail="Saved words per minute of recorded audio"/><Metric label="PROCESSING TIME" value={stats().total.recordings?seconds(stats().total.latencyMs/stats().total.recordings):'—'} detail="Recognition through cleanup, average"/><Metric label="LONGEST TRANSCRIPT" value={count(stats().total.longest)} detail="Words in a single saved result"/><Metric label="TRANSCRIPTS WITH LISTS" value={count(stats().total.withLists)} detail="Saved text containing list markers"/><Metric label="TRANSCRIPTS WITH PARAGRAPHS" value={count(stats().total.withParagraphs)} detail="Saved text containing blank-line breaks"/></div></section>
    <p class="field-note">Text history is kept from the beginning, until you delete it. These statistics include all retained transcripts, not just the current page. Retries update one record; private and deleted sessions are excluded. Profiles are rebuilt when their source records change. Times use this computer’s time zone. Recovery audio has its own 30-day storage limit.</p><Show when={stats().skippedRecords}><p class="notice" role="status">{stats().skippedRecords} unreadable or invalid-date records were excluded.</p></Show>
  </div>;
}
