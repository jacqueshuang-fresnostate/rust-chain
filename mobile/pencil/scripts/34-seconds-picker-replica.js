// Rebuild 07c pair picker boards: exact seconds page replica as backdrop + dim + sheet.
function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}
function strip(n){
  const o={};
  for(const k of Object.keys(n)){
    if(k==="id"||k==="children")continue;
    o[k]=n[k];
  }
  delete o.placeholder;
  const kids=n.children||[];
  if(kids.length)o.children=kids.map(strip);
  return o;
}

const tops=new Set();
Get((n,c)=>{c.skipChildren();if(n.type==="frame")tops.add(n.id);});
const boards={vONcc:"VL8er",kLXCs:"g9agt"};
const srcs={};const doomed=[];
Get((n,c)=>{
  if(tops.has(n.id)){
    if(n.id==="VL8er")srcs.VL8er=n;
    if(n.id==="g9agt")srcs.g9agt=n;
    return;
  }
});
for(const [dst,src] of Object.entries(boards)){
  if(!srcs[src])throw new Error("source board missing: "+src);
}
// delete existing children of both picker boards (deepest first)
const order=[];let cur=null;
Get((n,c)=>{
  if(tops.has(n.id)){cur=n.id;return;}
  if(cur==="vONcc"||cur==="kLXCs")order.push({board:cur,id:n.id});
});
for(let i=order.length-1;i>=0;i--){try{Delete(order[i].id);}catch(e){}}

function sheet(stage,dark){
  Insert(stage,{type:"rectangle",name:"Dim",x:0,y:0,width:390,height:816,fill:dark?"#000000B3":"#07110D99",layoutPosition:"absolute"});
  const sh=Insert(stage,{type:"frame",name:"Pair Sheet",layout:"vertical",width:390,height:472,x:0,y:344,layoutPosition:"absolute",padding:[14,16,22,16],gap:10,fill:"$surface",cornerRadius:[20,20,0,0],stroke:"$border",strokeWidth:{top:1},strokeAlignment:"inner",effect:{type:"shadow",shadowType:"outer",color:"#07110D33",offset:{x:0,y:-8},blur:24}});
  const grab=Insert(sh,{type:"frame",name:"Grab",layout:"horizontal",width:"fill_container",height:14,justifyContent:"center",alignItems:"center"});
  Insert(grab,{type:"rectangle",name:"Grab Bar",width:40,height:4,cornerRadius:2,fill:"$border"});
  const head=Insert(sh,{type:"frame",name:"Sheet Head",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"center"});
  const hc=Insert(head,{type:"frame",name:"Head Copy",layout:"vertical",gap:3});
  T(hc,"Sheet Title","选择交易对",18,"700","$text");
  T(hc,"Sheet Sub","秒合约产品由接口返回",10,"450","$muted");
  const close=Insert(head,{type:"frame",name:"Close",layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$surface-2",justifyContent:"center",alignItems:"center"});
  I(close,"Close Icon","x","$muted",16);
  const search=Insert(sh,{type:"frame",name:"Pair Search",layout:"horizontal",width:"fill_container",height:40,padding:[0,12],gap:8,alignItems:"center",fill:dark?"$surface-2":"$canvas",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  I(search,"Search Icon","search","$muted",15);
  T(search,"Search Ph","搜索交易对",11,"450","$muted");
  const list=Insert(sh,{type:"frame",name:"Pair List",layout:"vertical",width:"fill_container",gap:2});
  const rows=[["BTC/USDT","63,085.00","92.40%","bitcoin",true],["ETH/USDT","3,412.88","90.00%","hexagon",false],["HIPPO/USDT","0.0824","88.00%","gem",false]];
  for(const x of rows){
    const r=Insert(list,{type:"frame",name:"Pair "+x[0],layout:"horizontal",width:"fill_container",height:64,padding:[0,12],gap:11,alignItems:"center",fill:x[4]?"$mint-soft":"$surface",stroke:x[4]?"$mint":"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:8});
    I(r,"Pair Icon",x[3],"$mint-strong",19);
    const c=Insert(r,{type:"frame",name:"Pair Copy",layout:"vertical",gap:3,width:"fill_container"});
    T(c,"Pair Symbol",x[0],14,"700","$text",undefined,"$font-data");
    T(c,"Pair Tag","秒合约 · 现货钱包结算",10,"450","$muted");
    const v=Insert(r,{type:"frame",name:"Pair Value",layout:"vertical",gap:3,alignItems:"end"});
    T(v,"Pair Price",x[1],13,"650","$text",undefined,"$font-data");
    T(v,"Pair Payout","收益 "+x[2],10,"550","$mint-strong",undefined,"$font-data");
    if(x[4])I(r,"Pair Check","check","$mint-strong",17);
  }
  T(sh,"Sheet Note","最新价与收益率来自行情与产品接口，不做占位虚构。",10,"450","$muted",320);
}

for(const [dst,srcId] of Object.entries(boards)){
  const dark=srcId==="g9agt";
  Update(dst,{height:844,layout:"vertical",fill:dark?"#000000":"$canvas",clip:true,theme:{mode:dark?"dark":"light"}});
  // stage holds the replica + dim + sheet
  const stage=Insert(dst,{type:"frame",name:"Picker Stage",layout:"none",width:"fill_container",height:816,fill:dark?"#000000":"$canvas"});
  const statusSrc=srcs[srcId].children.find(c=>c.name==="Status Bar");
  // replica of the real seconds page (all children, in a none-layout clone layer)
  const replica=Insert(stage,{type:"frame",name:"Seconds Replica",layout:"vertical",width:390,height:816,x:0,y:0,layoutPosition:"absolute",fill:dark?"#000000":"$canvas",clip:true});
  for(const child of srcs[srcId].children){
    const clone=strip(child);
    Insert(replica,clone);
  }
  sheet(stage,dark);
  Update(dst,{placeholder:false});
}
Print("PICKER_REPLICA_DONE");
