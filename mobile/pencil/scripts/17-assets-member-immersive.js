const IDS=["rCvD2","bDCHw","db9Ly","s9tP7","V2V4G","cWKy0","uveNF","x7YT1","IE0D6","JMhfz","Ic2x8","WajnA","T0DzZ","jv0qf"];
const seen={};
Get((n,c)=>{if(n.id==="p61z2Q"||n.id==="Q4JYj")seen[n.id]=true;});
if(!seen["p61z2Q"]||!seen["Q4JYj"])throw new Error("member frames not found");
for(const id of IDS)Delete(id);
function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}
function status(p){const a=Insert(p,{type:"frame",name:"Status Bar",layout:"horizontal",width:"fill_container",height:28,padding:[0,16],justifyContent:"space_between",alignItems:"center"});T(a,"Time","09:41",11,"650","$text",undefined,"$font-data");const b=Insert(a,{type:"frame",name:"Status Signals",layout:"horizontal",gap:8,alignItems:"center"});T(b,"Network","4G+",10,"550","$muted",undefined,"$font-data");I(b,"Wifi","wifi","$text",14);T(b,"Battery","82%",10,"550","$text",undefined,"$font-data");}
function headerRoot(p,title,icon){const h=Insert(p,{type:"frame",name:"Assets Header",layout:"horizontal",width:"fill_container",padding:[12,20,4,20],justifyContent:"space_between",alignItems:"center"});T(h,"Title",title,22,"750");I(h,"Header Action",icon,"$text",20);}
function hero(p,dark){
  const band=Insert(p,{type:"frame",name:"Portfolio Member Overview",layout:"vertical",width:"fill_container",gap:8,padding:16,fill:dark?"$surface-2":"$surface",stroke:"$border",strokeWidth:{top:1,bottom:1},strokeAlignment:"inner"});
  const card=Insert(band,{type:"frame",name:"Member Hero",layout:"vertical",width:"fill_container",height:264,cornerRadius:"$radius-l",gap:14,padding:[18,20,16,20],stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",clip:true});
  Insert(card,{type:"rectangle",name:"Card BG Image",layoutPosition:"absolute",x:0,y:0,width:358,height:264,fill:{type:"image",enabled:true,url:dark?"images/generated-1785685909638.png":"images/generated-1785687482899.png",mode:"fill"}});
  Insert(card,{type:"rectangle",name:"Card BG Overlay",layoutPosition:"absolute",x:0,y:0,width:358,height:264,fill:dark?"#00000026":"#FFFFFF00"});
  Insert(card,{type:"ellipse",name:"Bloom",layoutPosition:"absolute",x:186,y:-64,width:220,height:220,fill:{type:"gradient",gradientType:"radial",enabled:true,rotation:0,size:{width:1,height:1},colors:[{color:"#43EFA92E",position:0},{color:"#43EFA900",position:1}]}});
  const ink=dark?"#FFFFFF":"$text";
  const sub=dark?"#9BA8A1":"$muted";
  const pos=dark?"$mint":"$mint-strong";
  const top=Insert(card,{type:"frame",name:"Hero Head",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"start"});
  const lv=Insert(top,{type:"frame",name:"Hero Value",layout:"vertical",gap:8});
  const lr=Insert(lv,{type:"frame",name:"Balance Label Row",layout:"horizontal",gap:6,alignItems:"center"});
  T(lr,"Balance Label","总资产估值",12,"500",sub);
  I(lr,"Balance Eye","eye",sub,14);
  const vr=Insert(lv,{type:"frame",name:"Balance Value Row",layout:"horizontal",gap:6,alignItems:"end"});
  T(vr,"Balance Value","24,806.32",34,"700",ink,undefined,"$font-data");
  T(vr,"Balance Unit","USDT",11,"500",sub,undefined,"$font-data");
  const rt=Insert(top,{type:"frame",name:"Today Return",layout:"vertical",gap:6,alignItems:"end"});
  T(rt,"Today Label","今日收益",11,"500",sub);
  T(rt,"Today Value","+1,204.55",18,"650",pos,undefined,"$font-data");
  T(rt,"Today Percent","+4.85%",11,"550",pos,undefined,"$font-data");
  const g=Insert(card,{type:"frame",name:"Hero Actions",layout:"horizontal",width:"fill_container",gap:8});
  for(const x of [["充币","arrow-down-to-line"],["提币","arrow-up-from-line"],["划转","repeat-2"],["账单","file-text"]]){
    const c=Insert(g,{type:"frame",name:"Op "+x[0],layout:"vertical",width:"fill_container",height:66,gap:6,justifyContent:"center",alignItems:"center",fill:dark?"#FFFFFF14":"$surface",stroke:dark?"#FFFFFF26":"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:"$radius-m"});
    I(c,x[0]+" Icon",x[1],ink,19);
    T(c,x[0]+" Label",x[0],11,"500",ink);
  }
}
function holding(p,symbol,name,amount,fiat,icon,split){
  const r=Insert(p,{type:"frame",name:"Holding "+symbol,layout:"horizontal",width:"fill_container",gap:10,padding:[7,0],alignItems:"center"});
  const coin=Insert(r,{type:"frame",name:"Coin "+symbol,layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",alignItems:"center"});
  I(coin,"Coin Icon",icon,"$mint-strong",16);
  const c=Insert(r,{type:"frame",name:"Holding Copy",layout:"vertical",width:"fill_container",gap:2});
  T(c,"Holding Symbol",symbol,14,"700","$text");
  T(c,"Holding Name",name,10,"500","$muted");
  if(split)T(c,"Holding Split",split,10,"500","$muted",undefined,"$font-data");
  const v=Insert(r,{type:"frame",name:"Holding Value",layout:"vertical",gap:2,alignItems:"end"});
  T(v,"Holding Amount",amount,15,"650","$text",undefined,"$font-data");
  T(v,"Holding Fiat",fiat,10,"500","$muted",undefined,"$font-data");
}
function holdingsSec(p,withEmpty){
  const s=Insert(p,{type:"frame",name:"Assets Holdings",layout:"vertical",width:"fill_container",gap:8,padding:[14,16,4,16]});
  const h=Insert(s,{type:"frame",name:"Holdings Header",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"center"});
  T(h,"Holdings Title","我的持仓",22,"750","$text");
  T(h,"Holdings More","按估值降序",12,"500","$muted");
  holding(s,"BTC","比特币","0.2500","≈ $15,771.25","bitcoin");
  holding(s,"ETH","以太坊","1.5000","≈ $5,119.32","hexagon");
  holding(s,"USDT","稳定币 · Tether","3,500.00","≈ $3,500.00","coins","可用 3,450.00 · 冻结 50.00");
  holding(s,"HIPPO","平台币","5,000","≈ $412.00","gem");
  if(withEmpty){
    T(s,"Empty Kicker","空态 / EMPTY",10,"600","$muted",undefined,"$font-data");
    const e=Insert(s,{type:"frame",name:"Holdings Empty",layout:"vertical",width:"fill_container",padding:[18,16],gap:7,justifyContent:"center",alignItems:"center",fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:12});
    I(e,"Empty Icon","wallet-cards","$muted",23);
    T(e,"Empty Main","暂无持仓",13,"600","$text");
    T(e,"Empty Sub","充币后展示币种数量、估值与可用余额",10,"450","$muted",300);
    const b=Insert(e,{type:"frame",name:"Empty Deposit",layout:"horizontal",width:"fill_container",height:44,gap:8,justifyContent:"center",alignItems:"center",fill:"$mint",stroke:"$mint",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
    I(b,"Empty Deposit Icon","arrow-down-to-line","#07110D",17);
    T(b,"Empty Deposit Label","去充币",13,"650","#07110D");
  }
}
function tools(p){const g=Insert(p,{type:"frame",name:"Group 资金工具",layout:"vertical",width:"fill_container",gap:6,padding:[12,20,0,20]});T(g,"Group Title","资金工具",11,"600","$muted");for(const x of [["资金账单","file-text","按时间筛选钱包流水"],["提币记录","history","查看处理状态和链上哈希"],["快捷充值","zap","使用已配置的充值渠道"]]){const r=Insert(g,{type:"frame",name:"Row "+x[0],layout:"horizontal",width:"fill_container",height:52,gap:12,alignItems:"center"});I(r,"Icon",x[1],"$text",18);T(r,"Label",x[0],13,"600","$text");Insert(r,{type:"frame",name:"Spacer",width:"fill_container",height:10});T(r,"Row Sub",x[2],10,"450","$muted");I(r,"Chevron","chevron-right","$muted",16);}}
function entry(d,it,on){const c=Insert(d,{type:"frame",name:"Nav "+it[0],layout:"vertical",width:"fill_container",gap:4,justifyContent:"center",alignItems:"center"});I(c,it[0]+" Icon",it[1],on?"$mint-strong":"$muted",22);T(c,it[0]+" Label",it[0],10,on?"650":"500",on?"$text":"$muted");}
function nav(p,active){const n=Insert(p,{type:"frame",name:"Bottom Navigation",layout:"horizontal",width:"fill_container",height:84,padding:[6,16,10,16],fill:"#00000000",stroke:"#00000000"});const d=Insert(n,{type:"frame",name:"Nav Dock",layout:"horizontal",width:"fill_container",height:68,padding:[0,16],cornerRadius:24,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",alignItems:"center"});entry(d,["首页","house"],false);entry(d,["行情","chart-line"],false);const fs=Insert(d,{type:"frame",name:"FAB Space",layout:"vertical",width:56,height:56,justifyContent:"end",alignItems:"center"});T(fs,"FAB Label","交易",10,"500","$muted");entry(d,["资产","wallet-cards"],active==="资产");entry(d,["我的","user-round"],false);const fab=Insert(n,{type:"frame",name:"Nav FAB",layout:"horizontal",width:56,height:56,cornerRadius:28,fill:"$mint",justifyContent:"center",alignItems:"center",layoutPosition:"absolute",x:151,y:-12});I(fab,"FAB Icon","arrow-left-right","#07110D",24);}
function build(s,dark,withEmpty){status(s);headerRoot(s,"资产","eye");hero(s,dark);holdingsSec(s,withEmpty);tools(s);nav(s,"资产");}
build("p61z2Q",false,true);
build("Q4JYj",true,false);
Print("IMMERSIVE_REBUILT");
