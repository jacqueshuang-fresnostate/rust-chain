// Polish newly added pages: transfer backdrop, full phone height, copy fixes.
const IDS={
  ncrL:"A9It6g", ncrD:"h4gfd",
  trL:"v6phV", trD:"TuWXq",
  helpL:"UouET", helpD:"FM5tp",
  ordL:"e5Qs1", ordD:"hxe8l",
  ledL:"Bcug6", ledD:"IVMAO",
  msgL:"t7j6n", msgD:"eSMHf",
  predL:"CzpTv", predD:"ZvGMv",
  earnL:"nqP6W", earnD:"aXxul"
};

function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}

// Fix broken "可划转" unit text anywhere
Get((n,c)=>{
  if(n.type==="text"&&n.content&&String(n.content).indexOf("可划转")>=0){
    Update(n.id,{content:"可划转 —"});
  }
});

// Full phone height for short secondary pages
const fullH=["A9It6g","h4gfd","UouET","FM5tp","e5Qs1","hxe8l","Bcug6","IVMAO","t7j6n","eSMHf","CzpTv","ZvGMv","nqP6W","aXxul"];
for(const id of fullH){
  try{Update(id,{height:844,layout:"vertical",clip:true});}catch(e){}
}

// Ensure each full page has a bottom spacer if missing
const needSpacer=new Set(fullH);
const hasSpacer=new Set();
let cur=null;
const TOPS=new Set(Object.values(IDS));
Get((n,c)=>{
  if(TOPS.has(n.id)){cur=n.id;return;}
  if(cur&&needSpacer.has(cur)&&n.name==="Bottom Spacer")hasSpacer.add(cur);
});
for(const id of needSpacer){
  if(!hasSpacer.has(id)){
    Insert(id,{type:"frame",name:"Bottom Spacer",width:"fill_container",height:"fill_container"});
  }
}

// Rebuild transfer sheets with assets-like backdrop + sheet
function rebuildTransfer(id,dark){
  // delete children
  const kids=[]; let root=null;
  const tops=new Set();
  Get((n,c)=>{c.skipChildren(); if(n.type==="frame")tops.add(n.id);});
  Get((n,c)=>{
    if(tops.has(n.id)){root=n.id;return;}
    if(root===id)kids.push(n.id);
  });
  for(let i=kids.length-1;i>=0;i--){try{Delete(kids[i]);}catch(e){}}
  Update(id,{height:844,layout:"vertical",fill:dark?"#000000":"$canvas",clip:true,theme:{mode:dark?"dark":"light"}});

  // status
  const st=Insert(id,{type:"frame",name:"Status Bar",layout:"horizontal",width:"fill_container",height:28,padding:[0,16],justifyContent:"space_between",alignItems:"center"});
  T(st,"Time","09:41",11,"650","$text",undefined,"$font-data");
  const sig=Insert(st,{type:"frame",name:"Status Signals",layout:"horizontal",gap:8,alignItems:"center"});
  T(sig,"Network","4G+",10,"550","$muted",undefined,"$font-data");I(sig,"Wifi","wifi","$text",14);T(sig,"Battery","82%",10,"550","$text",undefined,"$font-data");

  // backdrop stage
  const stage=Insert(id,{type:"frame",name:"Sheet Stage",layout:"none",width:"fill_container",height:816,fill:dark?"#000000":"$canvas"});
  // faux assets page under dim
  const faux=Insert(stage,{type:"frame",name:"Faux Assets",layout:"vertical",width:390,height:816,x:0,y:0,layoutPosition:"absolute",padding:[12,20],gap:10,fill:dark?"#000000":"$canvas"});
  T(faux,"Faux Title","资产",22,"750","$text");
  const card=Insert(faux,{type:"frame",name:"Faux Card",layout:"vertical",width:"fill_container",height:160,cornerRadius:"$radius-l",padding:[16,16],gap:8,fill:dark?"$surface-2":"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner"});
  T(card,"Faux L","总资产估值",12,"500","$muted");
  T(card,"Faux V","24,806.32 USDT",28,"700","$text",undefined,"$font-data");
  T(faux,"Faux H","我的持仓",18,"700","$text");
  T(faux,"Faux R1","BTC  ·  0.2500",13,"500","$muted");
  T(faux,"Faux R2","USDT · 3,500.00",13,"500","$muted");
  // dim
  Insert(stage,{type:"rectangle",name:"Dim",x:0,y:0,width:390,height:816,fill:dark?"#000000B3":"#07110D99",layoutPosition:"absolute"});
  // sheet
  const sheet=Insert(stage,{type:"frame",name:"Transfer Sheet",layout:"vertical",width:390,height:460,x:0,y:356,layoutPosition:"absolute",padding:[14,16,22,16],gap:12,fill:"$surface",cornerRadius:[20,20,0,0],stroke:"$border",strokeWidth:{top:1},strokeAlignment:"inner",effect:{type:"shadow",shadowType:"outer",color:"#07110D33",offset:{x:0,y:-8},blur:24}});
  const grab=Insert(sheet,{type:"frame",name:"Grab",layout:"horizontal",width:"fill_container",height:14,justifyContent:"center",alignItems:"center"});
  Insert(grab,{type:"rectangle",name:"Grab Bar",width:40,height:4,cornerRadius:2,fill:"$border"});
  const head=Insert(sheet,{type:"frame",name:"Sheet Head",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"center"});
  T(head,"Sheet Title","资金划转",18,"700","$text");
  const close=Insert(head,{type:"frame",name:"Close",layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$surface-2",justifyContent:"center",alignItems:"center"});I(close,"Close Icon","x","$muted",16);
  const route=Insert(sheet,{type:"frame",name:"Route",layout:"horizontal",width:"fill_container",gap:8,alignItems:"center"});
  const from=Insert(route,{type:"frame",name:"From",layout:"vertical",width:"fill_container",gap:4,padding:[12,12],fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  T(from,"From L","从",10,"500","$muted");T(from,"From V","现货账户",14,"650","$text");
  I(route,"Swap Icon","arrow-left-right","$mint-strong",18);
  const to=Insert(route,{type:"frame",name:"To",layout:"vertical",width:"fill_container",gap:4,padding:[12,12],fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  T(to,"To L","到",10,"500","$muted");T(to,"To V","杠杆账户",14,"650","$text");
  // fields
  const a=Insert(sheet,{type:"frame",name:"Asset",layout:"vertical",width:"fill_container",gap:4,padding:[10,12],fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  const ah=Insert(a,{type:"frame",name:"Asset H",layout:"horizontal",width:"fill_container",justifyContent:"space_between"});T(ah,"Asset L","资产",10,"550","$muted");T(ah,"Asset U","可划转 —",10,"550","$muted",undefined,"$font-data");
  T(a,"Asset V","USDT",16,"600","$text",undefined,"$font-data");
  const m=Insert(sheet,{type:"frame",name:"Amount",layout:"vertical",width:"fill_container",gap:4,padding:[10,12],fill:"$surface",stroke:"$blue",strokeWidth:2,strokeAlignment:"inner",cornerRadius:4});
  const mh=Insert(m,{type:"frame",name:"Amount H",layout:"horizontal",width:"fill_container",justifyContent:"space_between"});T(mh,"Amount L","数量",10,"550","$blue");T(mh,"Amount U","USDT",10,"550","$muted",undefined,"$font-data");
  T(m,"Amount V","0.00",16,"600","$text",undefined,"$font-data");
  T(sheet,"Hint","可用余额由钱包接口返回 · 划转即时生效",10,"450","$muted");
  const btn=Insert(sheet,{type:"frame",name:"Primary 确认划转",layout:"horizontal",width:"fill_container",height:50,gap:8,justifyContent:"center",alignItems:"center",fill:"$mint",cornerRadius:4});
  I(btn,"Primary Icon","arrow-left-right","#07110D",17);T(btn,"Primary Label","确认划转",13,"650","#07110D");
}

rebuildTransfer("v6phV",false);
rebuildTransfer("TuWXq",true);

// New coin records: add date lines under rows if missing meta density - append footer note
function appendNote(id,text){
  // insert before bottom spacer if any
  Insert(id,{type:"frame",name:"Footer Note Wrap",layout:"vertical",width:"fill_container",padding:[8,16,16,16],children:[{type:"text",name:"Footer Note",content:text,fontFamily:"$font-sans",fontSize:10,fontWeight:"450",fill:"$muted",textGrowth:"fixed-width",width:350}]});
}
appendNote("A9It6g","记录来自新币认购/分发/解锁接口；金额与状态不伪造。");
appendNote("h4gfd","记录来自新币认购/分发/解锁接口；金额与状态不伪造。");

// Empty orders: pin CTA to bottom via spacer already; tighten empty vertical padding by updating Empty State if found
Get((n,c)=>{
  if(n.name==="Empty State"&&n.padding){
    // leave as is
  }
  if(n.name==="CTA Wrap"){
    Update(n.id,{padding:[0,20,20,20]});
  }
});

// Prediction odds: select YES visually stronger - already ok
// Earn: fix "— USDT" if broken dash
Get((n,c)=>{
  if(n.type==="text"&&n.content==="- USDT")Update(n.id,{content:"— USDT"});
  if(n.type==="text"&&n.content==="- ¢")Update(n.id,{content:"— ¢"});
  if(n.type==="text"&&n.content&&n.content.indexOf("— ¢")>=0){/*ok*/}
});
// fix cent placeholders that rendered as "- ¢"
Get((n,c)=>{
  if(n.type==="text"&&typeof n.content==="string"&&/^\s*-\s*¢\s*$/.test(n.content))Update(n.id,{content:"— ¢"});
  if(n.type==="text"&&typeof n.content==="string"&&/^\s*-\s*USDT\s*$/.test(n.content))Update(n.id,{content:"— USDT"});
});

Print("POLISH_NEW done");
