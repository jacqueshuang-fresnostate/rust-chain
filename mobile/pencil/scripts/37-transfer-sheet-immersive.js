// Rebuild Transfer Sheet internals on 39 Light/Dark: amount hero + glass route + holdings asset row.
function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}

const DARK={TuWXq:true,v6phV:false};
const boards=new Set(Object.keys(DARK));
const tops=new Set();
Get((n,c)=>{c.skipChildren();if(n.type==="frame")tops.add(n.id);});
const sheets={};const doomed=[];
let board=null,inSheet=false;
Get((n,c)=>{
  if(tops.has(n.id)){board=boards.has(n.id)?n.id:null;inSheet=false;return;}
  if(!board)return;
  if(n.name==="Transfer Sheet"){sheets[board]=n.id;inSheet=true;return;}
  if(inSheet)doomed.push(n.id);
});
if(!sheets.v6phV||!sheets.TuWXq)throw new Error("transfer sheet missing "+JSON.stringify(sheets));
for(let i=doomed.length-1;i>=0;i--){try{Delete(doomed[i]);}catch(e){}}

function glass(dark){
  return {fill:dark?"#FFFFFF14":"#FFFFFF99",stroke:dark?"#FFFFFF26":"#FFFFFFCC",strokeWidth:1,strokeAlignment:"inner",effect:dark?undefined:{type:"background_blur",radius:18}};
}

function fillSheet(sheet,dark){
  const ink=dark?"#FFFFFF":"$text";
  const sub=dark?"#9BA8A1":"$muted";
  const g=glass(dark);
  Update(sheet,{height:520,y:296,padding:[14,16,22,16],gap:10,fill:"$surface",cornerRadius:[20,20,0,0],stroke:"$border",strokeWidth:{top:1},strokeAlignment:"inner",effect:{type:"shadow",shadowType:"outer",color:"#07110D33",offset:{x:0,y:-8},blur:24}});

  const grab=Insert(sheet,{type:"frame",name:"Grab",layout:"horizontal",width:"fill_container",height:14,justifyContent:"center",alignItems:"center"});
  Insert(grab,{type:"rectangle",name:"Grab Bar",width:40,height:4,cornerRadius:2,fill:"$border"});

  const head=Insert(sheet,{type:"frame",name:"Sheet Head",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"center"});
  T(head,"Sheet Title","资金划转",18,"700","$text");
  const close=Insert(head,{type:"frame",name:"Close",layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$surface-2",justifyContent:"center",alignItems:"center"});
  I(close,"Close Icon","x","$muted",16);

  const hero=Insert(sheet,{type:"frame",name:"Amount Hero",layout:"vertical",width:"fill_container",gap:8,padding:[16,18],cornerRadius:"$radius-l",clip:true,fill:dark?"$surface-2":"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner"});
  if(dark)Insert(hero,{type:"rectangle",name:"Hero Wash",layoutPosition:"absolute",x:0,y:0,width:358,height:140,fill:"#FFFFFF14"});
  Insert(hero,{type:"ellipse",name:"Hero Bloom",layoutPosition:"absolute",x:186,y:-64,width:220,height:220,fill:{type:"gradient",gradientType:"radial",enabled:true,rotation:0,size:{width:1,height:1},colors:[{color:"#43EFA92E",position:0},{color:"#43EFA900",position:1}]}});
  T(hero,"Hero Label","划转数量 · USDT",11,"500",sub);
  T(hero,"Hero Amount","0.00",30,"700",ink,undefined,"$font-data");
  const meta=Insert(hero,{type:"frame",name:"Hero Meta",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"center"});
  T(meta,"Hero Avail","可划转 —",10,"500",sub,undefined,"$font-data");
  const chip=Insert(meta,{type:"frame",name:"All Chip",layout:"horizontal",height:28,padding:[0,10],alignItems:"center",cornerRadius:14,fill:g.fill,stroke:g.stroke,strokeWidth:1,strokeAlignment:"inner",effect:g.effect});
  T(chip,"All Chip L","全部",10,"650",ink);

  const route=Insert(sheet,{type:"frame",name:"Route Bar",layout:"horizontal",width:"fill_container",height:52,padding:[8,12],gap:8,alignItems:"center",cornerRadius:"$radius-m",fill:g.fill,stroke:g.stroke,strokeWidth:1,strokeAlignment:"inner",effect:g.effect});
  const from=Insert(route,{type:"frame",name:"From",layout:"vertical",width:"fill_container",gap:2});
  T(from,"From L","从",10,"500",sub);T(from,"From V","现货账户",14,"650",ink);
  const swap=Insert(route,{type:"frame",name:"Swap",layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$mint",justifyContent:"center",alignItems:"center"});
  I(swap,"Swap Icon","arrow-left-right","#07110D",16);
  const to=Insert(route,{type:"frame",name:"To",layout:"vertical",width:"fill_container",gap:2,alignItems:"end"});
  T(to,"To L","到",10,"500",sub);T(to,"To V","杠杆账户",14,"650",ink);

  const row=Insert(sheet,{type:"frame",name:"Asset Row",layout:"horizontal",width:"fill_container",height:52,gap:10,alignItems:"center"});
  const mark=Insert(row,{type:"frame",name:"Asset Mark",layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",alignItems:"center"});
  I(mark,"Asset Icon","coins","$mint-strong",16);
  const copy=Insert(row,{type:"frame",name:"Asset Copy",layout:"vertical",gap:2,width:"fill_container"});
  T(copy,"Asset Symbol","USDT",14,"700","$text",undefined,"$font-data");
  T(copy,"Asset Sub","选择资产",10,"500","$muted");
  const val=Insert(row,{type:"frame",name:"Asset Value",layout:"vertical",gap:2,alignItems:"end"});
  T(val,"Asset Avail","—",15,"650","$text",undefined,"$font-data");
  T(val,"Asset Avail L","可划转",10,"500","$muted");

  T(sheet,"Hint","可用余额由钱包接口返回 · 划转即时生效",10,"450","$muted");
  const btn=Insert(sheet,{type:"frame",name:"Primary 确认划转",layout:"horizontal",width:"fill_container",height:50,gap:8,justifyContent:"center",alignItems:"center",fill:"$mint",cornerRadius:4});
  I(btn,"Primary Icon","arrow-left-right","#07110D",17);T(btn,"Primary Label","确认划转",13,"650","#07110D");
}

fillSheet(sheets.v6phV,false);
fillSheet(sheets.TuWXq,true);
Print("TRANSFER_IMMERSIVE light="+sheets.v6phV+" dark="+sheets.TuWXq+" deleted="+doomed.length);
