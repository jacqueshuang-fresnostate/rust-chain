SetVariables({
  "canvas": {"type":"color","value":[{"value":"#F7F9F8","theme":{"mode":"light"}},{"value":"#070A09","theme":{"mode":"dark"}}]},
  "surface": {"type":"color","value":[{"value":"#FFFFFF","theme":{"mode":"light"}},{"value":"#0C100E","theme":{"mode":"dark"}}]},
  "surface-2": {"type":"color","value":[{"value":"#EEF2F0","theme":{"mode":"light"}},{"value":"#121714","theme":{"mode":"dark"}}]},
  "surface-3": {"type":"color","value":[{"value":"#E4EAE7","theme":{"mode":"light"}},{"value":"#19211D","theme":{"mode":"dark"}}]},
  "text": {"type":"color","value":[{"value":"#111714","theme":{"mode":"light"}},{"value":"#F2F7F4","theme":{"mode":"dark"}}]},
  "muted": {"type":"color","value":[{"value":"#68736D","theme":{"mode":"light"}},{"value":"#95A19A","theme":{"mode":"dark"}}]},
  "border": {"type":"color","value":[{"value":"#CCD5D0","theme":{"mode":"light"}},{"value":"#29342E","theme":{"mode":"dark"}}]},
  "hairline": {"type":"color","value":[{"value":"#DDE4E0","theme":{"mode":"light"}},{"value":"#202923","theme":{"mode":"dark"}}]},
  "mint": {"type":"color","value":"#43EFA9"},
  "mint-strong": {"type":"color","value":[{"value":"#087B52","theme":{"mode":"light"}},{"value":"#61F1B6","theme":{"mode":"dark"}}]},
  "mint-soft": {"type":"color","value":[{"value":"#D9F9EB","theme":{"mode":"light"}},{"value":"#103326","theme":{"mode":"dark"}}]},
  "coral": {"type":"color","value":"#FF654A"},
  "coral-soft": {"type":"color","value":[{"value":"#FFE2DC","theme":{"mode":"light"}},{"value":"#3B1B16","theme":{"mode":"dark"}}]},
  "blue": {"type":"color","value":"#3478F6"},
  "warning": {"type":"color","value":"#E8B348"},
  "font-sans": {"type":"string","value":"Geist"},
  "font-data": {"type":"string","value":"Geist Mono"},
  "control": {"type":"number","value":44},
  "field": {"type":"number","value":52},
  "gap-1": {"type":"number","value":4},
  "gap-2": {"type":"number","value":8},
  "gap-3": {"type":"number","value":12},
  "gap-4": {"type":"number","value":16},
  "gap-5": {"type":"number","value":20},
  "gap-6": {"type":"number","value":24},
  "radius-s": {"type":"number","value":4},
  "radius-m": {"type":"number","value":12},
  "radius-l": {"type":"number","value":20}
});
function textNode(parent,name,content,size,weight,fill,width,font){return Insert(parent,{type:"text",name,content,fontFamily:font||"$font-sans",fontSize:size,fontWeight:weight||"400",fill:fill||"$text",textGrowth:width?"fixed-width":"auto",width:width||undefined,lineHeight:1.25});}
function swatch(parent,name,color,role){let card=Insert(parent,{type:"frame",name,layout:"vertical",width:"fill_container",gap:8});Insert(card,{type:"rectangle",name:name+" Color",width:"fill_container",height:64,fill:color,stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});textNode(card,name+" Name",name,13,"600","$text");textNode(card,name+" Role",role,11,"400","$muted","fill_container","$font-data");return card;}
function icon(parent,name,iconName,color,size){return Insert(parent,{type:"icon",name,library:"lucide",icon:iconName,width:size||20,height:size||20,fill:color||"$text"});}
function button(parent,name,label,kind,iconName){let primary=kind==="primary";let danger=kind==="danger";let b=Insert(parent,{type:"frame",name,layout:"horizontal",height:52,width:"fill_container",padding:[0,16],gap:8,alignItems:"center",justifyContent:"center",fill:primary?"$mint":danger?"$coral":"$surface",stroke:primary?"$mint":danger?"$coral":"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});if(iconName)icon(b,name+" Icon",iconName,primary||danger?"#07110D":"$text",18);textNode(b,name+" Label",label,14,"650",primary||danger?"#07110D":"$text");return b;}
pos=FindEmptySpace({width:1200,height:1100,padding:120});
board=Insert(document,{type:"frame",name:"00 / Design System",x:pos.x,y:pos.y,width:1200,height:"fit_content(1100)",layout:"vertical",padding:48,gap:32,fill:"$canvas",theme:{mode:"light"},clip:true,placeholder:true});
hero=Insert(board,{type:"frame",name:"Foundation Header",width:"fill_container",layout:"horizontal",justifyContent:"space_between",alignItems:"end",padding:[0,0,24,0],stroke:"$text",strokeWidth:{bottom:2},strokeAlignment:"inner"});
heroText=Insert(hero,{type:"frame",name:"Foundation Header Copy",layout:"vertical",gap:8,width:720});
textNode(heroText,"Foundation Eyebrow","HIPPO MOBILE / UI·UX SYSTEM 02",12,"600","$mint-strong",undefined,"$font-data");
textNode(heroText,"Foundation Title","Instrument Home Language",48,"700","$text","fill_container");
textNode(heroText,"Foundation Description","以首页为母版：淡网格、发丝结构、薄荷主动作、珊瑚风险语义和紧凑数据排版。",16,"400","$muted","fill_container");
stamp=Insert(hero,{type:"frame",name:"Foundation Stamp",width:160,height:86,layout:"vertical",justifyContent:"center",alignItems:"center",fill:"$text",cornerRadius:4});
textNode(stamp,"Stamp Brand","HIPPO",22,"750","$surface");
textNode(stamp,"Stamp Meta","MOBILE / 390",11,"500","$mint",undefined,"$font-data");
colors=Insert(board,{type:"frame",name:"Color Roles",layout:"vertical",width:"fill_container",gap:16});
textNode(colors,"Color Roles Title","01 / COLOR ROLES",18,"700","$text",undefined,"$font-data");
swatches=Insert(colors,{type:"frame",name:"Color Swatches",layout:"horizontal",width:"fill_container",gap:12});
swatch(swatches,"Canvas","$canvas","page / grid ground");swatch(swatches,"Surface","$surface","operational plate");swatch(swatches,"Text","$text","graphite / cold white");swatch(swatches,"Mint","$mint","primary / signal");swatch(swatches,"Coral","$coral","risk / sell / alert");swatch(swatches,"Blue","$blue","focus / information");
typeSection=Insert(board,{type:"frame",name:"Typography",layout:"vertical",width:"fill_container",gap:16,padding:[24,0],stroke:"$border",strokeWidth:{top:1,bottom:1},strokeAlignment:"inner"});
textNode(typeSection,"Typography Title","02 / TYPE & DATA",18,"700","$text",undefined,"$font-data");
typeRow=Insert(typeSection,{type:"frame",name:"Typography Samples",layout:"horizontal",width:"fill_container",gap:24,alignItems:"end"});
typeA=Insert(typeRow,{type:"frame",name:"Display Sample",layout:"vertical",width:"fill_container",gap:6});textNode(typeA,"Display Number","63,085.00",42,"700","$mint-strong",undefined,"$font-data");textNode(typeA,"Display Label","LATEST PRICE / BTC·USDT",11,"500","$muted",undefined,"$font-data");
typeB=Insert(typeRow,{type:"frame",name:"Heading Sample",layout:"vertical",width:"fill_container",gap:6});textNode(typeB,"Heading Text","资产与市场，一眼完成判断",28,"700","$text","fill_container");textNode(typeB,"Heading Label","GEIST / 28 / 700",11,"500","$muted",undefined,"$font-data");
typeC=Insert(typeRow,{type:"frame",name:"Body Sample",layout:"vertical",width:"fill_container",gap:6});textNode(typeC,"Body Text","每个页面只保留一个视觉主角，所有状态、金额与风险信息必须真实可辨。",14,"400","$muted","fill_container");textNode(typeC,"Body Label","GEIST / 14 / 400",11,"500","$muted",undefined,"$font-data");
controls=Insert(board,{type:"frame",name:"Controls",layout:"vertical",width:"fill_container",gap:16});textNode(controls,"Controls Title","03 / CONTROLS",18,"700","$text",undefined,"$font-data");
controlRow=Insert(controls,{type:"frame",name:"Button Samples",layout:"horizontal",width:"fill_container",gap:12});button(controlRow,"Primary Button","确认买入","primary","arrow-up-right");button(controlRow,"Secondary Button","查看委托","secondary","list-ordered");button(controlRow,"Danger Button","确认卖出","danger","arrow-down-right");
fieldRow=Insert(controls,{type:"frame",name:"Field Samples",layout:"horizontal",width:"fill_container",gap:12});
for(let i=0;i<3;i++){let names=["价格","数量","验证码"];let vals=["63,085.00 USDT","0.015 BTC","6 位数字"];let f=Insert(fieldRow,{type:"frame",name:names[i]+" Field",layout:"vertical",width:"fill_container",height:68,padding:[8,12],gap:4,fill:"$surface",stroke:i===1?"$blue":"$border",strokeWidth:i===1?2:1,strokeAlignment:"inner",cornerRadius:4});textNode(f,names[i]+" Label",names[i],10,"550","$muted");textNode(f,names[i]+" Value",vals[i],15,"550","$text",undefined,"$font-data");}
navSection=Insert(board,{type:"frame",name:"Navigation Language",layout:"vertical",width:"fill_container",gap:16});textNode(navSection,"Navigation Title","04 / HEADER & SEVEN-DOMAIN NAV",18,"700","$text",undefined,"$font-data");
headerSample=Insert(navSection,{type:"frame",name:"Root Header Sample",layout:"horizontal",width:"fill_container",height:64,padding:[0,16],justifyContent:"space_between",alignItems:"center",fill:"$surface",stroke:"$border",strokeWidth:{bottom:1},strokeAlignment:"inner"});textNode(headerSample,"Header Brand","HIPPO",24,"750","$text");headerActions=Insert(headerSample,{type:"frame",name:"Header Actions",layout:"horizontal",gap:8});for(let pair of [["Theme","moon"],["Messages","bell"]]){let c=Insert(headerActions,{type:"frame",name:pair[0],width:44,height:44,layout:"horizontal",justifyContent:"center",alignItems:"center",fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:22});icon(c,pair[0]+" Icon",pair[1],"$text",20);}
nav=Insert(navSection,{type:"frame",name:"Seven Domain Navigation",layout:"horizontal",width:"fill_container",height:84,padding:[10,8,8,8],gap:2,fill:"$surface",stroke:"$border",strokeWidth:{top:1},strokeAlignment:"inner"});
for(let item of [["首页","home"],["行情","chart-no-axes-combined"],["现货","arrow-right-left"],["秒合约","zap"],["合约","activity"],["资产","wallet-cards"],["我的","user-round"]]){let active=item[0]==="首页";let n=Insert(nav,{type:"frame",name:"Nav "+item[0],layout:"vertical",width:"fill_container",height:64,gap:4,justifyContent:"center",alignItems:"center",fill:active?"$mint-soft":"$surface",cornerRadius:item[0]==="秒合约"?20:4});icon(n,item[0]+" Icon",item[1],active?"$mint-strong":"$muted",20);textNode(n,item[0]+" Label",item[0],10,active?"650":"500",active?"$text":"$muted");}
principles=Insert(board,{type:"frame",name:"UX Principles",layout:"horizontal",width:"fill_container",gap:0,stroke:"$text",strokeWidth:1,strokeAlignment:"inner"});
for(let item of [["01","ONE HERO","每页一个主任务"],["02","REAL DATA","不伪造金额与产品"],["03","44–52","触控与表单合同"],["04","SEPARATE","现货/合约/秒合约独立"]]){let p=Insert(principles,{type:"frame",name:"Principle "+item[0],layout:"vertical",width:"fill_container",padding:20,gap:8,stroke:"$border",strokeWidth:{right:1},strokeAlignment:"inner"});textNode(p,"Principle Number "+item[0],item[0],12,"650","$mint-strong",undefined,"$font-data");textNode(p,"Principle Title "+item[0],item[1],15,"700","$text",undefined,"$font-data");textNode(p,"Principle Copy "+item[0],item[2],12,"400","$muted","fill_container");}
Update(board,{placeholder:false});
