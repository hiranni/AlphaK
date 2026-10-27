/* Hallmark · pre-emit critique: P4 H4 E4 S5 R4 V4 */
import { useEffect, useRef, useState } from 'react';
import { Activity, ArrowRight, BookOpen, Check, ChevronRight, Database, FlaskConical, GitBranch, Layers3, Search, X } from 'lucide-react';
import { Link } from 'react-router-dom';
import { indicators, strategies, type Definition } from '../data/quantDefinitions';
import { contractTabs, engineContracts } from '../data/quantEngineContracts';

export function QuantLibrary({kind='strategies'}:{kind?:'strategies'|'indicators'}) {
 const [query,setQuery]=useState(''),[category,setCategory]=useState('all'),[selected,setSelected]=useState<Definition|null>(null),[tab,setTab]=useState(0);
 const dialog=useRef<HTMLElement>(null),opener=useRef<HTMLButtonElement|null>(null);
 const rows=kind==='indicators'?indicators:strategies;
 const categories=[...new Set(rows.map(r=>r.category))];
 const visible=rows.filter(r=>(category==='all'||category===r.category)&&`${r.name}${r.id}${r.category}${r.purpose}`.toLowerCase().includes(query.toLowerCase()));
 const contract=selected?engineContracts[selected.id]:null;
 useEffect(()=>{
  if(!selected)return;
  const previous=document.body.style.overflow; document.body.style.overflow='hidden'; dialog.current?.focus();
  const key=(e:KeyboardEvent)=>{
   if(e.key==='Escape')setSelected(null);
   if(e.key==='Tab'){
    const items=dialog.current?.querySelectorAll<HTMLElement>('button,[href],[tabindex="0"]');
    if(!items?.length)return;
    const first=items[0],last=items[items.length-1];
    if(e.shiftKey&&(document.activeElement===first||document.activeElement===dialog.current)){e.preventDefault();last.focus();}
    else if(!e.shiftKey&&document.activeElement===last){e.preventDefault();first.focus();}
   }
  };
  document.addEventListener('keydown',key);
  return()=>{document.body.style.overflow=previous;document.removeEventListener('keydown',key);opener.current?.focus();};
 },[selected]);
 const open=(r:Definition,e:React.MouseEvent<HTMLButtonElement>)=>{opener.current=e.currentTarget;setSelected(r);setTab(0);};
 return <div className="quant-manager quant-library">
  <section className="panel quant-table-panel">
   <div className="quant-toolbar"><label className="quant-search"><Search size={15}/><input aria-label="搜索定义" placeholder="搜索模型、名称、方向" value={query} onChange={e=>setQuery(e.target.value)}/></label><span className="muted">{visible.length} / {rows.length} 项定义</span></div>
   <div className="quant-directions" role="group" aria-label="研究方向"><span>研究方向</span>{['all',...categories].map(c=><button key={c} aria-pressed={category===c} className={category===c?'is-active':''} onClick={()=>setCategory(c)}>{category===c&&<Check size={12}/>} {c==='all'?'全部':c}</button>)}</div>
   <div className="quant-table-scroll"><table className="quant-strategy-table"><thead><tr><th>名称 / 版本</th><th>研究方向</th><th>用途</th><th>{kind==='indicators'?'说明':'引擎契约'}</th></tr></thead><tbody>{visible.map(r=><tr key={r.id}><td><button className="quant-name" onClick={e=>open(r,e)}>{kind==='indicators'?<Activity size={15}/>:<FlaskConical size={15}/>} {r.name}</button><small className="muted">{r.id}</small></td><td>{r.category}</td><td>{r.purpose}</td><td><button className="quant-details" onClick={e=>open(r,e)}>{kind==='indicators'?'计算定义':'数据 / 公式 / 流程'}<ChevronRight size={13}/></button>{engineContracts[r.id]&&<small className="muted">{engineContracts[r.id].status.startsWith('已')?'试验适配已接入':'待实现'}</small>}</td></tr>)}</tbody></table></div>
   {!visible.length&&<div className="quant-empty">没有匹配定义，请调整搜索或研究方向。</div>}
  </section>
  {selected&&<div className="quant-dialog-backdrop" onClick={()=>setSelected(null)}><section ref={dialog} tabIndex={-1} className="quant-dialog quant-contract-dialog" role="dialog" aria-modal="true" aria-label={selected.name} onClick={e=>e.stopPropagation()}>
   <header><div><span className="quant-eyebrow">{selected.id} · {selected.category}</span><h2>{selected.name}</h2></div><button className="btn btn-soft" aria-label="关闭定义" onClick={()=>setSelected(null)}><X size={18}/></button></header>
   <p className="quant-contract-purpose">{selected.purpose}</p>
   {contract?<><div className="quant-contract-status"><GitBranch size={14}/>{contract.status}<small>规范版本 research-contract-1 · 描述不等于执行DSL</small></div>
    <div className="quant-contract-tabs" role="tablist" aria-label="策略计算契约">{contractTabs.map((t,i)=><button key={t} id={`contract-tab-${i}`} role="tab" aria-selected={tab===i} aria-controls="contract-panel" tabIndex={tab===i?0:-1} onClick={()=>setTab(i)} onKeyDown={e=>{if(['ArrowRight','ArrowLeft','Home','End'].includes(e.key)){e.preventDefault();const next=e.key==='Home'?0:e.key==='End'?3:(i+(e.key==='ArrowRight'?1:3))%4;setTab(next);document.getElementById(`contract-tab-${next}`)?.focus();}}}>{i===0?<Database size={14}/>:i===1?<FlaskConical size={14}/>:i===2?<Layers3 size={14}/>:<GitBranch size={14}/>} {t}</button>)}</div>
    <div id="contract-panel" role="tabpanel" aria-labelledby={`contract-tab-${tab}`} className="quant-contract-content">
     {tab===0&&<><h3>输入字段与时间口径</h3><div className="quant-contract-inputs">{contract.inputs.map(([name,fields,rule])=><article key={name}><code>{name}</code><pre>{fields}</pre><p>{rule}</p></article>)}</div><p>必需数据缺失、日期不齐或过期 → blocked；不回填未来财报，不把所选池情绪当成全市场情绪。</p></>}
     {tab===1&&<><h3>01 · 特征构造</h3><p className="quant-formula">{contract.features}</p><h3>02 · 拟合、标签与验证</h3><p>{contract.fit}</p><h3>03 · 策略专属门槛</h3><p className="quant-formula">{contract.gate}</p></>}
     {tab===2&&<><h3>信号 → 预算 → 风险 → 执行</h3><p>{contract.integration}</p><h3>不可绕过的边界</h3><p>最多8只股票；不扫描全市场；当前分析不创建订单。Agent绑定属于实例，Agent不能直接写订单表。研究候选不是可执行订单。</p></>}
     {tab===3&&<><div className="quant-context-flow">{['冻结配置','校验输入','构建特征','历史验证','策略判断','冻结报告'].map((s,i)=><span key={s}><b>{String(i+1).padStart(2,'0')}</b>{s}{i<5&&<ArrowRight size={12}/>}</span>)}</div><h3>上下文传递与所有权</h3><p>{contract.context}</p><pre className="quant-context-json">{JSON.stringify({contract_version:'research-contract-1',workspace_id:'<不可变配置ID>',as_of:'<数据截止时间>',input_hash:'<冻结输入SHA256>',scope:'pool | market',model_id:selected.id,features:'<带日期、单位与来源>',prediction:'<成熟标签训练结果或blocked>',decision:'buy_candidate | reduce_review | wait | blocked',execution:'research_only'},null,2)}</pre><p>这是字段流转示意，不是已生成的报告。现行服务端输入与报告见 trading_research.rs；目标模型与现有试验适配分别标记。</p></>}
    </div><details className="quant-original-notes"><summary>研究背景与原始模型说明</summary>{selected.sections.map(([title,body])=><div key={title}><h3>{title}</h3><p>{body}</p></div>)}</details>
   </>:selected.sections.map(([title,body])=><div key={title}><h3>{title}</h3><p>{body}</p></div>)}
  </section></div>}
 </div>;
}
