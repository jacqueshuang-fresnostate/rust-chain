// Rebuild 00 / Design System to current immersive language (v03).
const TOPS=new Set();
Get((n,c)=>{c.skipChildren(); if(n.type==="frame")TOPS.add(n.id);});
const doomed=[]; let curRoot=null;
Get((n,c)=>{
  if(TOPS.has(n.id)){curRoot=n.id;return;}
  if(curRoot==="FISId")doomed.push(n.id);
});
for(let i=doomed.length-1;i>=0;i--){try{Delete(doomed[i]);}catch(e){}}

function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}
function section(p,title){const s=Insert(p,{type:"frame",name:title,layout:"vertical",width:"fill_container",gap:14,padding:[24,0,0,0]});T(s,title+" Title",title,14,"700","$text",undefined,"$font-data");return s;}
function swatch(p,name,role,fill){const c=Insert(p,{type:"frame",name:name,layout:"vertical",width:"fill_container",gap:8});Insert(c,{type:"rectangle",name:name+" Color",width:"fill_container",height:56,cornerRadius:8,fill:fill,stroke:"$border",strokeWidth:1,strokeAlignment:"inner"});T(c,name+" Name",name,12,"650","$text");T(c,name+" Role",role,10,"450","$muted");return c;}
function btn(p,n,label,kind,icon){
  const primary=kind==="primary", danger=kind==="danger", glass=kind==="glass";
  const b=Insert(p,{type:"frame",name:n,layout:"horizontal",width:"fill_container",height:48,gap:8,padding:[0,14],justifyContent:"center",alignItems:"center",
    fill:primary?"$mint":danger?"$coral":glass?"#FFFFFF99":"$surface",
    stroke:primary?"$mint":danger?"$coral":glass?"#FFFFFFCC":"$border",
    strokeWidth:1,strokeAlignment:"inner",cornerRadius:glass?"$radius-m":4,
    effect:glass?{type:"background_blur",radius:18}:undefined});
  if(icon)I(b,n+" Icon",icon,primary||danger?"#07110D":"$text",17);
  T(b,n+" Label",label,13,"650",primary||danger?"#07110D":"$text");
  return b;
}
function field(p,n,label,value,focus){
  const f=Insert(p,{type:"frame",name:n,layout:"vertical",width:"fill_container",gap:4,padding:[10,12],fill:"$surface",stroke:focus?"$blue":"$border",strokeWidth:focus?2:1,strokeAlignment:"inner",cornerRadius:4});
  T(f,n+" Label",label,10,"550",focus?"$blue":"$muted");
  T(f,n+" Value",value,15,"600","$text",undefined,"$font-data");
  return f;
}
function chip(p,n,label,kind){
  const color=kind==="good"?"$mint-strong":kind==="warn"?"$warning":kind==="bad"?"$coral":"$muted";
  const fill=kind==="good"?"$mint-soft":kind==="bad"?"$coral-soft":"$surface-2";
  return Insert(p,{type:"frame",name:n,layout:"horizontal",height:28,padding:[0,10],gap:4,alignItems:"center",fill,stroke:color,strokeWidth:1,strokeAlignment:"inner",cornerRadius:14,children:[{type:"text",name:n+" T",content:label,fontFamily:"$font-data",fontSize:10,fontWeight:"600",fill:color}]});
}

Update("FISId",{width:1200,layout:"vertical",fill:"$canvas",padding:[40,48,48,48],gap:8,clip:true,x:0,y:0});
const root="FISId";

// Header
const head=Insert(root,{type:"frame",name:"Foundation Header",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"start",padding:[0,0,20,0],stroke:"$border",strokeWidth:{bottom:1},strokeAlignment:"inner"});
const hc=Insert(head,{type:"frame",name:"Foundation Header Copy",layout:"vertical",gap:8,width:"fill_container"});
T(hc,"Foundation Eyebrow","HIPPO MOBILE / UI·UX SYSTEM 03",11,"650","$mint-strong",undefined,"$font-data");
T(hc,"Foundation Title","Immersive Instrument Language",32,"750","$text");
T(hc,"Foundation Description","以资产/首页沉浸卡片为母版：丝绸底、薄荷 Bloom、毛玻璃控件、浮动五域 Dock + FAB、纯 Lucide 图标。不伪造金额。",13,"450","$muted",720);
const stamp=Insert(head,{type:"frame",name:"Foundation Stamp",layout:"vertical",width:160,height:72,padding:[14,16],gap:4,justifyContent:"center",fill:"#07110D",cornerRadius:8});
T(stamp,"Stamp Brand","HIPPO",16,"700","#FFFFFF");
T(stamp,"Stamp Meta","MOBILE / 390 · v03",10,"500","$mint",undefined,"$font-data");

// 01 Colors
const csec=section(root,"01 / COLOR ROLES");
const sw=Insert(csec,{type:"frame",name:"Color Swatches",layout:"horizontal",width:"fill_container",gap:12});
swatch(sw,"Canvas","page ground","$canvas");
swatch(sw,"Surface","operational plate","$surface");
swatch(sw,"Surface-2","raised / soft plate","$surface-2");
swatch(sw,"Text","graphite / cold white","$text");
swatch(sw,"Mint","primary / signal","$mint");
swatch(sw,"Coral","risk / sell / alert","$coral");
swatch(sw,"Blue","focus / information","$blue");

// 02 Type
const tsec=section(root,"02 / TYPE & DATA");
const ts=Insert(tsec,{type:"frame",name:"Typography Samples",layout:"horizontal",width:"fill_container",gap:24});
const d=Insert(ts,{type:"frame",name:"Display Sample",layout:"vertical",gap:6,width:"fill_container"});
T(d,"Display Number","24,806.32",34,"700","$text",undefined,"$font-data");
T(d,"Display Label","TOTAL VALUE / USDT · GEIST MONO 34/700",10,"500","$muted",undefined,"$font-data");
const h=Insert(ts,{type:"frame",name:"Heading Sample",layout:"vertical",gap:6,width:"fill_container"});
T(h,"Heading Text","登录后查看资产",26,"750","$text");
T(h,"Heading Label","GEIST / 22–26 / 750 · page title",10,"500","$muted");
const b=Insert(ts,{type:"frame",name:"Body Sample",layout:"vertical",gap:6,width:"fill_container"});
T(b,"Body Text","余额、估值与持仓仅在登录后同步；访客态不展示任何资产数字。",13,"450","$muted",280);
T(b,"Body Label","GEIST / 12–13 / 450 · helper copy",10,"500","$muted");

// 03 Controls
const ctl=section(root,"03 / CONTROLS");
const btns=Insert(ctl,{type:"frame",name:"Button Samples",layout:"horizontal",width:"fill_container",gap:12});
btn(btns,"Primary Button","确认买入","primary","check");
btn(btns,"Secondary Button","查看委托","secondary","list");
btn(btns,"Danger Button","确认卖出","danger","arrow-up-from-line");
btn(btns,"Glass Button","登录查看资产","glass","arrow-right");
const fields=Insert(ctl,{type:"frame",name:"Field Samples",layout:"horizontal",width:"fill_container",gap:12,padding:[8,0,0,0]});
field(fields,"价格 Field","价格","63,085.00 USDT",false);
field(fields,"数量 Field","数量","0.015 BTC",true);
field(fields,"验证码 Field","验证码","6 位数字",false);
const chips=Insert(ctl,{type:"frame",name:"Status Chips",layout:"horizontal",width:"fill_container",gap:10,padding:[8,0,0,0],alignItems:"center"});
T(chips,"Chip Label","STATUS",10,"600","$muted",undefined,"$font-data");
chip(chips,"Live","LIVE DATA","good");
chip(chips,"Warn","处理中","warn");
chip(chips,"Bad","失败","bad");
chip(chips,"Guest","GUEST / SIGN IN","neutral");

// 04 Immersive card
const ims=section(root,"04 / IMMERSIVE HERO CARD");
T(ims,"Immersive Note","资产/首页共用：h=236、$radius-l、丝绸底图 + 薄荷径向 Bloom；Guest 只放登录提示与毛玻璃 CTA，Member 放估值与操作。",12,"450","$muted",900);
const cards=Insert(ims,{type:"frame",name:"Card Samples",layout:"horizontal",width:"fill_container",gap:16});
function miniCard(p,name,dark,guest){
  const card=Insert(p,{type:"frame",name:name,layout:"vertical",width:358,height:236,cornerRadius:"$radius-l",gap:12,padding:[18,20,16,20],stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:guest?"space_between":"center",clip:true,fill:dark?"#0B1210":"$surface"});
  Insert(card,{type:"rectangle",name:name+" BG",layoutPosition:"absolute",x:0,y:0,width:358,height:236,fill:{type:"image",enabled:true,url:dark?"images/generated-1785685909638.png":"images/generated-1785687557714.png",mode:"fill"}});
  Insert(card,{type:"rectangle",name:name+" OV",layoutPosition:"absolute",x:0,y:0,width:358,height:236,fill:dark?"#00000040":"#FFFFFF00"});
  Insert(card,{type:"ellipse",name:name+" Bloom",layoutPosition:"absolute",x:186,y:-64,width:220,height:220,fill:{type:"gradient",gradientType:"radial",enabled:true,rotation:0,size:{width:1,height:1},colors:[{color:"#43EFA92E",position:0},{color:"#43EFA900",position:1}]}});
  const ink=dark?"#FFFFFF":"$text"; const sub=dark?"#9BA8A1":"$muted"; const pos=dark?"$mint":"$mint-strong";
  if(guest){
    const top=Insert(card,{type:"frame",name:name+" Head",layout:"vertical",gap:8});
    T(top,name+" K","GUEST / SIGN IN",11,"600",sub,undefined,"$font-data");
    T(top,name+" T","登录后查看资产",26,"750",ink);
    T(top,name+" S","余额、估值与持仓仅在登录后同步",12,"450",sub,300);
    const cta=Insert(card,{type:"frame",name:name+" CTA",layout:"horizontal",width:"fill_container",height:50,gap:8,justifyContent:"center",alignItems:"center",fill:dark?"#FFFFFF14":"#FFFFFF99",stroke:dark?"#FFFFFF26":"#FFFFFFCC",strokeWidth:1,strokeAlignment:"inner",cornerRadius:"$radius-m",effect:dark?undefined:{type:"background_blur",radius:18}});
    T(cta,name+" CTA L","登录查看资产",14,"650",ink);I(cta,name+" CTA I","arrow-right",ink,17);
  }else{
    const top=Insert(card,{type:"frame",name:name+" Head",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"end"});
    const lv=Insert(top,{type:"frame",name:name+" Val",layout:"vertical",gap:6});
    const lr=Insert(lv,{type:"frame",name:name+" LR",layout:"horizontal",gap:6,alignItems:"center"});T(lr,name+" BL","总资产估值",12,"500",sub);I(lr,name+" Eye","eye",sub,14);
    const vr=Insert(lv,{type:"frame",name:name+" VR",layout:"horizontal",gap:6,alignItems:"end"});T(vr,name+" BV","24,806.32",30,"700",ink,undefined,"$font-data");T(vr,name+" BU","USDT",11,"500",sub,undefined,"$font-data");
    const rt=Insert(top,{type:"frame",name:name+" Today",layout:"vertical",gap:4,alignItems:"end"});T(rt,name+" TL","今日收益",11,"500",sub);T(rt,name+" TV","+1,204.55",16,"650",pos,undefined,"$font-data");
    const g=Insert(card,{type:"frame",name:name+" Ops",layout:"horizontal",width:"fill_container",gap:8});
    for(const x of [["充币","arrow-down-to-line"],["提币","arrow-up-from-line"],["划转","repeat-2"],["账单","file-text"]]){
      const op=Insert(g,{type:"frame",name:name+" "+x[0],layout:"vertical",width:"fill_container",height:58,gap:5,justifyContent:"center",alignItems:"center",fill:dark?"#FFFFFF14":"#FFFFFF99",stroke:dark?"#FFFFFF26":"#FFFFFFCC",strokeWidth:1,strokeAlignment:"inner",cornerRadius:"$radius-m",effect:dark?undefined:{type:"background_blur",radius:18}});
      I(op,x[0]+" I",x[1],ink,18);T(op,x[0]+" L",x[0],11,"500",ink);
    }
  }
  return card;
}
miniCard(cards,"Guest Card Light",false,true);
miniCard(cards,"Member Card Light",false,false);
miniCard(cards,"Member Card Dark",true,false);

// 05 Holdings row
const hold=section(root,"05 / HOLDINGS ROW · PURE LUCIDE");
T(hold,"Hold Note","币种标记禁止 emoji/字母徽章；使用 Lucide + mint 图标，无圆盘底图感。",12,"450","$muted",900);
const rows=Insert(hold,{type:"frame",name:"Holding Samples",layout:"vertical",width:"fill_container",gap:0,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:8});
function holding(p,sym,name,amt,fiat,icon,split){
  const r=Insert(p,{type:"frame",name:"H "+sym,layout:"horizontal",width:"fill_container",gap:10,padding:[12,16],alignItems:"center",stroke:"$border",strokeWidth:{bottom:1},strokeAlignment:"inner"});
  I(r,sym+" Icon",icon,"$mint-strong",20);
  const c=Insert(r,{type:"frame",name:sym+" Copy",layout:"vertical",gap:2,width:"fill_container"});
  T(c,sym+" S",sym,14,"700","$text");T(c,sym+" N",name,10,"500","$muted");
  if(split)T(c,sym+" Sp",split,10,"500","$muted",undefined,"$font-data");
  const v=Insert(r,{type:"frame",name:sym+" V",layout:"vertical",gap:2,alignItems:"end"});
  T(v,sym+" A",amt,15,"650","$text",undefined,"$font-data");T(v,sym+" F",fiat,10,"500","$muted",undefined,"$font-data");
}
holding(rows,"BTC","比特币","0.2500","≈ $15,771.25","bitcoin");
holding(rows,"USDT","稳定币 · Tether","3,500.00","≈ $3,500.00","coins","可用 3,450.00 · 冻结 50.00");
holding(rows,"HIPPO","平台币","5,000","≈ $412.00","gem");

// 06 Navigation — 5 domain + FAB
const navs=section(root,"06 / FIVE-DOMAIN DOCK + FAB");
T(navs,"Nav Note","生产底栏为 5 域：首页 / 行情 / 交易(FAB) / 资产 / 我的。禁止七域全宽旧导航。FAB 绝对定位补偿 padding 原点差异。",12,"450","$muted",900);
const navStage=Insert(navs,{type:"frame",name:"Nav Stage",layout:"vertical",width:390,height:100,padding:[10,16,12,16],fill:"$canvas"});
const n=Insert(navStage,{type:"frame",name:"Bottom Navigation",width:"fill_container",height:84,padding:[6,16,10,16],fill:"#00000000",stroke:"#00000000",strokeAlignment:"inner"});
const dock=Insert(n,{type:"frame",name:"Nav Dock",layout:"horizontal",width:"fill_container",height:68,padding:[0,16],cornerRadius:24,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",alignItems:"center",effect:{type:"shadow",shadowType:"outer",color:"#07110D14",offset:{x:0,y:8},blur:24}});
function entry(d,label,icon,on){const c=Insert(d,{type:"frame",name:"Nav "+label,layout:"vertical",width:"fill_container",gap:4,justifyContent:"center",alignItems:"center"});I(c,label+" Icon",icon,on?"$mint-strong":"$muted",22);T(c,label+" Label",label,10,on?"650":"500",on?"$text":"$muted");}
entry(dock,"首页","house",false);entry(dock,"行情","chart-line",false);
const fs=Insert(dock,{type:"frame",name:"FAB Space",layout:"vertical",width:56,height:56,justifyContent:"end",alignItems:"center"});T(fs,"FAB Label","交易",10,"500","$muted");
entry(dock,"资产","wallet-cards",true);entry(dock,"我的","user-round",false);
const fab=Insert(n,{type:"frame",name:"Nav FAB",layout:"horizontal",width:56,height:56,cornerRadius:28,fill:"$mint",justifyContent:"center",alignItems:"center",layoutPosition:"absolute",x:167,y:-6,effect:{type:"shadow",shadowType:"outer",color:"#43EFA966",offset:{x:0,y:6},blur:16}});
I(fab,"FAB Icon","arrow-left-right","#07110D",24);

// 07 Principles
const pr=section(root,"07 / PRINCIPLES");
const grid=Insert(pr,{type:"frame",name:"Principle Grid",layout:"horizontal",width:"fill_container",gap:12});
function principle(p,num,title,copy){const c=Insert(p,{type:"frame",name:"P "+num,layout:"vertical",width:"fill_container",gap:6,padding:[14,14],fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:8});T(c,"N "+num,num,11,"700","$mint-strong",undefined,"$font-data");T(c,"T "+num,title,13,"700","$text");T(c,"C "+num,copy,11,"450","$muted",160);}
principle(grid,"01","ONE HERO","每页一个视觉主角");
principle(grid,"02","REAL DATA","不伪造金额与产品");
principle(grid,"03","LUCIDE ONLY","界面图标只用 Lucide");
principle(grid,"04","GLASS / DOCK","沉浸卡 + 五域 FAB");
principle(grid,"05","GUEST SAFE","访客不展示估值数字");
principle(grid,"06","44–52","触控目标最小高度");

Print("DS_V03 rebuilt deleted="+doomed.length);
