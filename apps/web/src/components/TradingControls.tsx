/* Hallmark · pre-emit critique: P4 H4 E4 S5 R5 V4 */
import { useEffect, useId, useRef, useState } from 'react';
import { Check, ChevronDown, Search } from 'lucide-react';
export function TradingCheck({checked,onChange,label}:{checked:boolean;onChange:(value:boolean)=>void;label:string}){
 return <button type="button" role="checkbox" aria-checked={checked} aria-label={label} className={`trading-check ${checked?'is-checked':''}`} onClick={()=>onChange(!checked)}><span aria-hidden="true">{checked&&<Check size={14} strokeWidth={3}/>}</span></button>;
}
export function TradingSelect({value,onChange,options,label,searchable=false}:{value:string;onChange:(value:string)=>void;options:{value:string;label:string}[];label:string;searchable?:boolean}){
 const [open,setOpen]=useState(false),[query,setQuery]=useState(''),[index,setIndex]=useState(0);
 const root=useRef<HTMLDivElement>(null),trigger=useRef<HTMLButtonElement>(null),input=useRef<HTMLInputElement>(null);const id=useId();
 const filtered=options.filter(o=>o.label.toLowerCase().includes(query.toLowerCase()));
 useEffect(()=>{if(!open)return;const close=(e:PointerEvent)=>{if(!root.current?.contains(e.target as Node))setOpen(false)};document.addEventListener('pointerdown',close);if(searchable)input.current?.focus();return()=>document.removeEventListener('pointerdown',close)},[open,searchable]);
 function choose(v:string){onChange(v);setOpen(false);setQuery('');trigger.current?.focus()}
 return <div className="trading-select" ref={root} onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget as Node))setOpen(false)}} onKeyDown={e=>{
 if(e.key==='Escape'){e.preventDefault();setOpen(false);trigger.current?.focus()}
 if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();if(!open){setOpen(true);setIndex(Math.max(0,options.findIndex(o=>o.value===value)))}else setIndex(i=>Math.max(0,Math.min(filtered.length-1,i+(e.key==='ArrowDown'?1:-1))))}
 if(e.key==='Enter'&&open){e.preventDefault();if(filtered[index])choose(filtered[index].value)}
 }}>
 <button ref={trigger} className="trading-select-trigger" type="button" aria-label={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={id} onClick={()=>{setOpen(!open);setQuery('');setIndex(Math.max(0,options.findIndex(o=>o.value===value)))}}><span>{options.find(o=>o.value===value)?.label||'请选择'}</span><ChevronDown size={16}/></button>
 {open&&<div className="trading-select-popup">{searchable&&<div className="trading-select-search"><Search size={14}/><input ref={input} aria-label={`搜索${label}`} placeholder="搜索配置名称…" value={query} onChange={e=>{setQuery(e.target.value);setIndex(0)}}/></div>}<div id={id} role="listbox" aria-label={label} className="trading-select-options">{filtered.length?filtered.map((o,i)=><button type="button" key={o.value} role="option" aria-selected={o.value===value} className={`${i===index?'is-focused':''} ${o.value===value?'is-selected':''}`} onMouseEnter={()=>setIndex(i)} onClick={()=>choose(o.value)}><span>{o.label}</span>{o.value===value&&<Check size={15}/>}</button>):<p>没有匹配的配置</p>}</div></div>}
 </div>
}
