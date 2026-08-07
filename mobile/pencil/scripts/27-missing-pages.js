// Add missing production-aligned artboards (light+dark) in current immersive language.
function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}
function S(name,dark,h){const p=FindEmptySpace({width:390,height:h||980,padding:80});return Insert(document,{type:"frame",name:name,x:p.x,y:p.y,width:390,height:"fit_content("+(h||980)+")",layout:"vertical",fill:dark?"#000000":"$canvas",theme:{mode:dark?"dark":"light"},clip:true,placeholder:true});}
function status(p){const a=Insert(p,{type:"frame",name:"Status Bar",layout:"horizontal",width:"fill_container",height:28,padding:[0,16],justifyContent:"space_between",alignItems:"center"});T(a,"Time","09:41",11,"650","$text",undefined,"$font-data");const b=Insert(a,{type:"frame",name:"Status Signals",layout:"horizontal",gap:8,alignItems:"center"});T(b,"Network","4G+",10,"550","$muted",undefined,"$font-data");I(b,"Wifi","wifi","$text",14);T(b,"Battery","82%",10,"550","$text",undefined,"$font-data");}
function header(p,title,actionIcon){const h=Insert(p,{type:"frame",name:"Page Header",layout:"horizontal",width:"fill_container",padding:[12,20,4,20],justifyContent:"space_between",alignItems:"center",fill:"#00000000"});const left=Insert(h,{type:"frame",name:"Header Left",layout:"horizontal",gap:10,alignItems:"center"});const back=Insert(left,{type:"frame",name:"Back",layout:"horizontal",width:36,height:36,cornerRadius:18,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",alignItems:"center"});I(back,"Back Icon","arrow-left","$text",18);T(left,"Title",title,18,"700");if(actionIcon){const a=Insert(h,{type:"frame",name:"Header Action",layout:"horizontal",width:36,height:36,cornerRadius:18,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",alignItems:"center"});I(a,"Header Action Icon",actionIcon,"$text",18);}else Insert(h,{type:"frame",name:"Header Empty",width:36,height:36});}
function tabRail(p,tabs,active){const r=Insert(p,{type:"frame",name:"Tab Rail",layout:"horizontal",width:"fill_container",height:44,padding:[0,16],gap:16,stroke:"$border",strokeWidth:{bottom:1},strokeAlignment:"inner"});for(const t of tabs){const on=t===active;const c=Insert(r,{type:"frame",name:"Tab "+t,layout:"vertical",height:44,justifyContent:"center",stroke:on?"$mint-strong":"$canvas",strokeWidth:{bottom:on?2:0},strokeAlignment:"inner"});T(c,t+" Label",t,12,on?"700":"500",on?"$text":"$muted");}}
function row(p,n,icon,title,sub,right,rightColor){const r=Insert(p,{type:"frame",name:n,layout:"horizontal",width:"fill_container",height:64,padding:[0,16],gap:12,alignItems:"center",fill:"$surface",stroke:"$border",strokeWidth:{bottom:1},strokeAlignment:"inner"});I(r,n+" Icon",icon,"$text",18);const c=Insert(r,{type:"frame",name:n+" Copy",layout:"vertical",gap:3,width:"fill_container"});T(c,n+" Title",title,13,"650","$text");if(sub)T(c,n+" Sub",sub,10,"450","$muted",undefined,"$font-data");if(right)T(r,n+" Right",right,12,"600",rightColor||"$text",undefined,"$font-data");else I(r,n+" Chevron","chevron-right","$muted",16);return r;}
function empty(p,icon,main,sub){const e=Insert(p,{type:"frame",name:"Empty State",layout:"vertical",width:"fill_container",gap:12,padding:[48,20],justifyContent:"center",alignItems:"center"});const plate=Insert(e,{type:"frame",name:"Empty Plate",layout:"horizontal",width:56,height:56,cornerRadius:28,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",alignItems:"center"});I(plate,"Empty Icon",icon,"$muted",24);T(e,"Empty Main",main,15,"650","$text");T(e,"Empty Sub",sub,11,"450","$muted",300);return e;}
function pill(p,label,on){return Insert(p,{type:"frame",name:"Pill "+label,layout:"horizontal",height:30,padding:[0,12],alignItems:"center",fill:on?"$mint-soft":"$surface-2",stroke:on?"$mint":"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:15,children:[{type:"text",name:label+" Text",content:label,fontFamily:"$font-sans",fontSize:11,fontWeight:on?"650":"500",fill:on?"$mint-strong":"$muted"}]});}
function primary(p,label,icon){const b=Insert(p,{type:"frame",name:"Primary "+label,layout:"horizontal",width:"fill_container",height:50,gap:8,justifyContent:"center",alignItems:"center",fill:"$mint",cornerRadius:4});if(icon)I(b,"Primary Icon",icon,"#07110D",17);T(b,"Primary Label",label,13,"650","#07110D");return b;}
function field(p,n,label,value,unit){const f=Insert(p,{type:"frame",name:n,layout:"vertical",width:"fill_container",gap:6,padding:[12,14],fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});const h=Insert(f,{type:"frame",name:n+" H",layout:"horizontal",width:"fill_container",justifyContent:"space_between"});T(h,n+" L",label,10,"550","$muted");if(unit)T(h,n+" U",unit,10,"550","$muted",undefined,"$font-data");T(f,n+" V",value,16,"600","$text",undefined,"$font-data");return f;}
function finish(s){Update(s,{placeholder:false});return s;}

// 1) New Coin Records — production /products/new-coins/records
function newCoinRecords(dark){
  const s=S("38 / New Coin Records · "+(dark?"Dark":"Light"),dark,980);
  status(s);header(s,"认购记录","list-filter");
  tabRail(s,["认购","分发","申购","解锁"],"认购");
  const list=Insert(s,{type:"frame",name:"Record List",layout:"vertical",width:"fill_container",padding:[8,0]});
  for(const x of [
    ["HIPPO","认购确认","+ 1,200 HIPPO","已确认","$mint-strong","receipt-text"],
    ["ORBIT","待支付解锁费","锁定 800 ORBIT","待解锁","$warning","lock-keyhole"],
    ["NOVA","分发到账","+ 50 NOVA","已到账","$mint-strong","package-open"]
  ]){
    const r=Insert(list,{type:"frame",name:"Rec "+x[0],layout:"horizontal",width:"fill_container",height:72,padding:[0,16],gap:12,alignItems:"center",fill:"$surface",stroke:"$border",strokeWidth:{bottom:1},strokeAlignment:"inner"});
    const mark=Insert(r,{type:"frame",name:x[0]+" Mark",layout:"horizontal",width:36,height:36,cornerRadius:18,fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",alignItems:"center"});
    I(mark,"Mark Icon",x[5],"$mint-strong",16);
    const c=Insert(r,{type:"frame",name:x[0]+" Copy",layout:"vertical",gap:3,width:"fill_container"});
    T(c,x[0]+" Sym",x[0]+" · "+x[1],13,"650","$text");
    T(c,x[0]+" Meta",x[2],10,"450","$muted",undefined,"$font-data");
    T(r,x[0]+" St",x[3],11,"650",x[4],undefined,"$font-data");
  }
  return finish(s);
}

// 2) Transfer sheet — assets transfer modal (spot <-> margin)
function transferSheet(dark){
  const s=S("39 / Transfer Sheet · "+(dark?"Dark":"Light"),dark,720);
  status(s);
  // dimmed backdrop + sheet
  const stage=Insert(s,{type:"frame",name:"Sheet Stage",layout:"none",width:"fill_container",height:620,fill:dark?"#000000AA":"#07110D66"});
  Insert(stage,{type:"rectangle",name:"Dim",x:0,y:0,width:390,height:620,fill:dark?"#000000AA":"#07110D66",layoutPosition:"absolute"});
  const sheet=Insert(stage,{type:"frame",name:"Transfer Sheet",layout:"vertical",width:390,height:420,x:0,y:200,layoutPosition:"absolute",padding:[16,16,20,16],gap:12,fill:"$surface",cornerRadius:[20,20,0,0],stroke:"$border",strokeWidth:{top:1},strokeAlignment:"inner"});
  const grab=Insert(sheet,{type:"frame",name:"Grab",layout:"horizontal",width:"fill_container",height:16,justifyContent:"center",alignItems:"center"});
  Insert(grab,{type:"rectangle",name:"Grab Bar",width:40,height:4,cornerRadius:2,fill:"$border"});
  const head=Insert(sheet,{type:"frame",name:"Sheet Head",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"center"});
  T(head,"Sheet Title","资金划转",18,"700","$text");
  const close=Insert(head,{type:"frame",name:"Close",layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$surface-2",justifyContent:"center",alignItems:"center"});I(close,"Close Icon","x","$muted",16);
  // from/to
  const route=Insert(sheet,{type:"frame",name:"Route",layout:"horizontal",width:"fill_container",gap:8,alignItems:"center"});
  const from=Insert(route,{type:"frame",name:"From",layout:"vertical",width:"fill_container",gap:4,padding:[12,12],fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  T(from,"From L","从",10,"500","$muted");T(from,"From V","现货账户",14,"650","$text");
  I(route,"Swap Icon","arrow-left-right","$mint-strong",18);
  const to=Insert(route,{type:"frame",name:"To",layout:"vertical",width:"fill_container",gap:4,padding:[12,12],fill:"$surface-2",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  T(to,"To L","到",10,"500","$muted");T(to,"To V","杠杆账户",14,"650","$text");
  field(sheet,"Asset","资产","USDT","可划转 —");
  field(sheet,"Amount","数量","0.00","USDT");
  T(sheet,"Hint","可用余额由钱包接口返回",10,"450","$muted");
  primary(sheet,"确认划转","arrow-left-right");
  return finish(s);
}

// 3) Help & Support — profile entry
function helpSupport(dark){
  const s=S("40 / Help & Support · "+(dark?"Dark":"Light"),dark,920);
  status(s);header(s,"帮助与客服");
  const hero=Insert(s,{type:"frame",name:"Help Hero",layout:"vertical",width:"fill_container",gap:8,padding:[16,20,8,20]});
  T(hero,"Help Title","我们能帮你什么？",22,"750","$text");
  T(hero,"Help Sub","优先查阅常见问题；复杂资金问题请联系在线客服。",12,"450","$muted",340);
  const searchWrap=Insert(s,{type:"frame",name:"Help Search Wrap",layout:"vertical",width:"fill_container",padding:[0,16]});
  const search=Insert(searchWrap,{type:"frame",name:"Help Search",layout:"horizontal",width:"fill_container",height:44,padding:[0,12],gap:8,alignItems:"center",fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  I(search,"Search Icon","search","$muted",16);T(search,"Search Ph","搜索问题关键词",12,"450","$muted");
  const g=Insert(s,{type:"frame",name:"Help Groups",layout:"vertical",width:"fill_container",gap:6,padding:[12,20]});
  T(g,"G1","常见问题",11,"600","$muted");
  row(g,"FAQ1","info","如何充币到账","网络与确认数说明");
  row(g,"FAQ2","shield-check","提币安全验证","资金密码与 2FA");
  row(g,"FAQ3","arrow-left-right","现货与杠杆划转","账户间资金移动");
  T(g,"G2","联系我们",11,"600","$muted");
  row(g,"CS","message-circle","在线客服","7×24 人工支持","进入 ›","$mint-strong");
  row(g,"Mail","mail","邮箱工单","support@hippo.exchange");
  return finish(s);
}

// 4) Orders empty
function ordersEmpty(dark){
  const s=S("08c / Orders · Empty · "+(dark?"Dark":"Light"),dark,920);
  status(s);header(s,"订单","history");
  const tabs=Insert(s,{type:"frame",name:"Mode Tabs",layout:"horizontal",width:"fill_container",padding:[8,20],gap:10});
  pill(tabs,"现货",true);pill(tabs,"杠杆",false);
  tabRail(s,["当前委托","历史委托","持仓"],"当前委托");
  empty(s,"clipboard-list","暂无委托","下单后将在此展示真实订单状态");
  const cta=Insert(s,{type:"frame",name:"CTA Wrap",layout:"vertical",width:"fill_container",padding:[0,20]});
  primary(cta,"去交易","arrow-left-right");
  return finish(s);
}

// 5) Wallet ledger empty
function ledgerEmpty(dark){
  const s=S("26b / Wallet Ledger · Empty · "+(dark?"Dark":"Light"),dark,900);
  status(s);header(s,"资金账单","list-filter");
  const filters=Insert(s,{type:"frame",name:"Filters",layout:"horizontal",width:"fill_container",padding:[8,16],gap:8});
  for(const x of ["全部资产","全部方向","日期"]){const b=Insert(filters,{type:"frame",name:"F "+x,layout:"horizontal",width:"fill_container",height:34,padding:[0,10],justifyContent:"space_between",alignItems:"center",fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});T(b,x+" L",x,10,"550","$muted");I(b,x+" I","chevron-down","$muted",14);}
  empty(s,"file-search","没有流水","调整筛选或等待钱包事件产生");
  return finish(s);
}

// 6) Message center empty
function messagesEmpty(dark){
  const s=S("11b / Message Center · Empty · "+(dark?"Dark":"Light"),dark,880);
  status(s);header(s,"消息中心","check-check");
  tabRail(s,["全部","资金","交易","系统"],"全部");
  empty(s,"bell-off","暂无消息","公告与账户通知会显示在这里");
  return finish(s);
}

// 7) Prediction markets list (more complete than hub entry)
function predictionBet(dark){
  const s=S("20b / Prediction · Bet · "+(dark?"Dark":"Light"),dark,980);
  status(s);header(s,"预测市场");
  const wrap=Insert(s,{type:"frame",name:"Card Wrap",layout:"vertical",width:"fill_container",padding:[8,16],gap:10});
  const c=Insert(wrap,{type:"frame",name:"Market Card",layout:"vertical",width:"fill_container",gap:10,padding:[16,16],fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:12});
  T(c,"Cat","加密 · 截止由接口返回",10,"550","$muted",undefined,"$font-data");
  T(c,"Q","BTC 本周收盘是否高于 65,000？",16,"700","$text",320);
  const odds=Insert(c,{type:"frame",name:"Odds",layout:"horizontal",width:"fill_container",gap:8});
  const yes=Insert(odds,{type:"frame",name:"Yes",layout:"vertical",width:"fill_container",gap:4,padding:[12,12],fill:"$mint-soft",stroke:"$mint",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  T(yes,"Yes L","是",12,"650","$mint-strong");T(yes,"Yes P","— ¢",18,"700","$mint-strong",undefined,"$font-data");
  const no=Insert(odds,{type:"frame",name:"No",layout:"vertical",width:"fill_container",gap:4,padding:[12,12],fill:"$coral-soft",stroke:"$coral",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  T(no,"No L","否",12,"650","$coral");T(no,"No P","— ¢",18,"700","$coral",undefined,"$font-data");
  field(wrap,"Stake","投入数量","100.00","USDT");
  const sum=Insert(wrap,{type:"frame",name:"Summary",layout:"vertical",width:"fill_container",gap:0,padding:[12,14],fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  const r1=Insert(sum,{type:"frame",name:"R1",layout:"horizontal",width:"fill_container",height:36,justifyContent:"space_between",alignItems:"center"});T(r1,"R1L","潜在回报",11,"500","$muted");T(r1,"R1V","由赔率接口返回",11,"600","$text",undefined,"$font-data");
  const r2=Insert(sum,{type:"frame",name:"R2",layout:"horizontal",width:"fill_container",height:36,justifyContent:"space_between",alignItems:"center"});T(r2,"R2L","结算来源",11,"500","$muted");T(r2,"R2V","官方行情源",11,"600","$text");
  primary(wrap,"确认预测","check");
  T(wrap,"Risk","预测结果以官方结算为准，提交后不可撤销。",10,"450","$warning",340);
  return finish(s);
}

// 8) Earn subscribe confirm
function earnSubscribe(dark){
  const s=S("16b / Earn · Subscribe · "+(dark?"Dark":"Light"),dark,900);
  status(s);header(s,"理财申购");
  const id=Insert(s,{type:"frame",name:"Product ID",layout:"vertical",width:"fill_container",gap:6,padding:[16,20]});
  T(id,"Eyebrow","EARN / USDT",10,"600","$mint-strong",undefined,"$font-data");
  T(id,"Name","USDT 活期理财",20,"750","$text");
  T(id,"Apy","参考年化由产品接口返回",12,"450","$muted");
  field(s,"Amt","申购数量","1,000.00","USDT");
  const sum=Insert(s,{type:"frame",name:"Sum",layout:"vertical",width:"fill_container",padding:[0,16]});
  const box=Insert(sum,{type:"frame",name:"Sum Box",layout:"vertical",width:"fill_container",padding:[12,14],fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  for(const x of [["预计日收益","— USDT"],["起息规则","以产品配置为准"],["赎回","随时 / 规则由接口返回"]]){
    const r=Insert(box,{type:"frame",name:x[0],layout:"horizontal",width:"fill_container",height:36,justifyContent:"space_between",alignItems:"center"});
    T(r,x[0]+"L",x[0],11,"500","$muted");T(r,x[0]+"V",x[1],11,"600","$text",undefined,"$font-data");
  }
  const cta=Insert(s,{type:"frame",name:"CTA",layout:"vertical",width:"fill_container",padding:[12,16],gap:10});
  primary(cta,"确认申购","landmark");
  T(cta,"Note","收益与额度以理财产品接口返回值为准。",10,"450","$muted",340);
  return finish(s);
}

const created=[];
created.push(newCoinRecords(false),newCoinRecords(true));
created.push(transferSheet(false),transferSheet(true));
created.push(helpSupport(false),helpSupport(true));
created.push(ordersEmpty(false),ordersEmpty(true));
created.push(ledgerEmpty(false),ledgerEmpty(true));
created.push(messagesEmpty(false),messagesEmpty(true));
created.push(predictionBet(false),predictionBet(true));
created.push(earnSubscribe(false),earnSubscribe(true));
Print("CREATED="+created.join(","));
Print("COUNT="+created.length);
