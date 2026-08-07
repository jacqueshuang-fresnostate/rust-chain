// Arrange all top-level frames into a clean numbered grid.
const tops=[];
Get((n,c)=>{c.skipChildren(); if(n.type==="frame"&&n.name) tops.push({id:n.id,name:n.name,w:typeof n.width==="number"?n.width:390});});
function key(name){
  const m=name.match(/^(\d{2})([a-z]?)/i);
  const num=m?parseInt(m[1],10):999;
  const suf=m&&m[2]?m[2]:" ";
  const dark=/Dark/.test(name)?1:0;
  const member=/Member/.test(name)?1:0;
  return [num,suf,dark,member,name];
}
tops.sort((a,b)=>{
  const ka=key(a.name), kb=key(b.name);
  for(let i=0;i<ka.length;i++){ if(ka[i]<kb[i])return -1; if(ka[i]>kb[i])return 1; }
  return 0;
});
const COLS=6, GAPX=80, GAPY=100, CELLW=390, CELLH=980, ORIGIN_X=0, ORIGIN_Y=0;
// Design system first if present, wide
let design=null;
const rest=[];
for(const t of tops){ if(t.name.indexOf("00 /")==0) design=t; else rest.push(t); }
if(design) Update(design.id,{x:ORIGIN_X,y:ORIGIN_Y});
// Design System v03 is ~3366px tall; keep a 200px buffer before the artboard grid.
const startY=design?3600:ORIGIN_Y;
let i=0;
for(const t of rest){
  const col=i%COLS, row=Math.floor(i/COLS);
  const x=ORIGIN_X+col*(CELLW+GAPX);
  const y=startY+row*(CELLH+GAPY);
  Update(t.id,{x:x,y:y});
  i++;
}
Print("TIDY total="+tops.length+" grid="+rest.length+" cols="+COLS);
