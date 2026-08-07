// Structural cleanup after repeated incremental rebuilds.
// Root cause: Delete(parent) does not reliably cascade; stale descendants remain as collapsed shells.
const TOPS=new Set();
Get((n,c)=>{c.skipChildren(); if(n.type==="frame")TOPS.add(n.id);});
const remove=[]; const seenRemove=new Set();
function postorder(n){
  for(const c of (n.children||[]))postorder(c);
  if(n.id&&!seenRemove.has(n.id)){seenRemove.add(n.id);remove.push(n.id);}
}
function hasVisual(n){return n.fill!==undefined||n.stroke!==undefined||n.effect!==undefined||n.icon!==undefined||n.content!==undefined||n.ref!==undefined||n.scriptUri!==undefined;}
function collapsed(n){
  if(n.type!=="frame"&&n.type!=="group")return false;
  if((n.children||[]).length)return false;
  const w=n.width,h=n.height;
  const fitW=typeof w==="string"&&w.indexOf("fit_content")===0;
  const fitH=typeof h==="string"&&h.indexOf("fit_content")===0;
  const noExplicitH=h===undefined||fitH;
  const noExplicitW=w===undefined||fitW||w==="fill_container";
  return noExplicitH&&noExplicitW&&!hasVisual(n)&&!n.slot;
}
function scan(n,parent){
  const children=n.children||[];
  // Deduplicate direct child names after incremental rebuilds, keep last occurrence.
  const last={};
  for(let i=0;i<children.length;i++){const nm=children[i].name;if(nm)last[nm]=i;}
  for(let i=0;i<children.length;i++){
    const c=children[i];
    if(c.name&&last[c.name]!==i){postorder(c);continue;}
    if(collapsed(c)){postorder(c);continue;}
    scan(c,n);
  }
}
// Exact direct-child allowlists for roots known to have been rebuilt from scratch.
const allow={
  FISId:new Set(["De6le","KJqNi","YOcY4","Hpfa7","ioWRF","tUtgk","PiRil","ChM2d"]),
  v6phV:new Set(["oKB7o","Ri0Ck"]),
  TuWXq:new Set(["SnL0Z","cRSLA"]),
  CUK3y:new Set(["N3S2Tt","J5z6YQ","X9QRW","yvLfJ","vTpDn"]),
  i6YDBr:new Set(["thJqN","AjDR9","MNi4Q","O11kB7","TnH6d"]),
};
Get((n,c)=>{
  if(!TOPS.has(n.id))return;
  const a=allow[n.id];
  if(a){for(const child of (n.children||[])){if(!a.has(child.id))postorder(child);else scan(child,n);}}
  else scan(n,null);
  c.skipChildren();
});
// Fix invalid icon names created by first pass.
const iconFix=[];
Get((n,c)=>{
  if(n.type==="icon"&&n.icon==="filter")iconFix.push([n.id,"list-filter"]);
  if(n.type==="icon"&&n.icon==="circle-help")iconFix.push([n.id,"info"]);
});
for(const x of iconFix)Update(x[0],{icon:x[1]});
for(const id of remove){try{Delete(id);}catch(e){}}
Print("STRUCTURE_CLEANUP removed="+remove.length+" icons="+iconFix.length);
