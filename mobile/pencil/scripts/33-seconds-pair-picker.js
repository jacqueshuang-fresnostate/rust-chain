// Seconds: tap-header pair picker effect.
// A) chevron-down on Pair header of all 4 seconds boards
// B) new 07c pair-picker sheet boards (light/dark)
function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}

const boards=new Set(["VL8er","g9agt","Lpt6q","WxeB8"]);
const tops=new Set();
Get((n,c)=>{c.skipChildren();if(n.type==="frame")tops.add(n.id);});
let mode=null;const pairFrames=[];
Get((n,c)=>{
  if(tops.has(n.id)){mode=boards.has(n.id)?n.id:null;return;}
  if(mode&&n.type==="frame"&&n.name==="Pair")pairFrames.push(n.id);
});
for(const id of pairFrames)I(id,"Pair Chevron","chevron-down","$muted",15);

function S(name,dark){const p=FindEmptySpace({width:390,height:844,padding:80});return Insert(document,{type:"frame",name,x:p.x,y:p.y,width:390,height:844,layout:"vertical",fill:dark?"#000000":"$canvas",theme:{mode:dark?"dark":"light"},clip:true,placeholder:true});}
function status(p){const a=Insert(p,{type:"frame",name:"Status Bar",layout:"horizontal",width:"fill_container",height:28,padding:[0,16],justifyContent:"space_between",alignItems:"center"});T(a,"Time","09:41",11,"650","$text",undefined,"$font-data");const b=Insert(a,{type:"frame",name:"Status Signals",layout:"horizontal",gap:8,alignItems:"center"});T(b,"Network","4G+",10,"550","$muted",undefined,"$font-data");I(b,"Wifi","wifi","$text",14);T(b,"Battery","82%",10,"550","$text",undefined,"$font-data");}

function picker(dark){
  const s=S("07c / Seconds · Pair Picker · "+(dark?"Dark":"Light"),dark);
  status(s);
  const stage=Insert(s,{type:"frame",name:"Picker Stage",layout:"none",width:"fill_container",height:816,fill:dark?"#000000":"$canvas"});
  // faux seconds page behind
  const faux=Insert(stage,{type:"frame",name:"Faux Seconds",layout:"vertical",width:390,height:816,x:0,y:0,layoutPosition:"absolute",padding:[12,20],gap:10,fill:dark?"#000000":"$canvas"});
  const fh=Insert(faux,{type:"frame",name:"Faux Header",layout:"horizontal",width:"fill_container",gap:8,alignItems:"center"});
  T(fh,"Faux Pair","BTC/USDT",18,"700","$text");I(fh,"Faux Chevron","chevron-down","$muted",15);
  T(fh,"Faux Tag","秒合约",10,"550","$muted",undefined,"$font-data");
  const fc=Insert(faux,{type:"frame",name:"Faux Board",layout:"vertical",width:"fill_container",height:170,cornerRadius:"$radius-l",padding:[16,16],gap:8,fill:dark?"$surface-2":"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner"});
  T(fc,"Faux Round","当前轮次 · 实时",11,"500","$muted");
  T(fc,"Faux Price","63,085.00",28,"700","$text",undefined,"$font-data");
  T(fc,"Faux Timer","00:08 · 92.4% payout",11,"550","$mint-strong",undefined,"$font-data");
  T(faux,"Faux Dir","选择方向",11,"600","$muted");
  Insert(stage,{type:"rectangle",name:"Dim",x:0,y:0,width:390,height:816,fill:dark?"#000000B3":"#07110D99",layoutPosition:"absolute"});
  // sheet
  const sheet=Insert(stage,{type:"frame",name:"Pair Sheet",layout:"vertical",width:390,height:472,x:0,y:344,layoutPosition:"absolute",padding:[14,16,22,16],gap:10,fill:"$surface",cornerRadius:[20,20,0,0],stroke:"$border",strokeWidth:{top:1},strokeAlignment:"inner",effect:{type:"shadow",shadowType:"outer",color:"#07110D33",offset:{x:0,y:-8},blur:24}});
  const grab=Insert(sheet,{type:"frame",name:"Grab",layout:"horizontal",width:"fill_container",height:14,justifyContent:"center",alignItems:"center"});
  Insert(grab,{type:"rectangle",name:"Grab Bar",width:40,height:4,cornerRadius:2,fill:"$border"});
  const head=Insert(sheet,{type:"frame",name:"Sheet Head",layout:"horizontal",width:"fill_container",justifyContent:"space_between",alignItems:"center"});
  const hc=Insert(head,{type:"frame",name:"Head Copy",layout:"vertical",gap:3});
  T(hc,"Sheet Title","选择交易对",18,"700","$text");
  T(hc,"Sheet Sub","秒合约产品由接口返回",10,"450","$muted");
  const close=Insert(head,{type:"frame",name:"Close",layout:"horizontal",width:32,height:32,cornerRadius:16,fill:"$surface-2",justifyContent:"center",alignItems:"center"});
  I(close,"Close Icon","x","$muted",16);
  const search=Insert(sheet,{type:"frame",name:"Pair Search",layout:"horizontal",width:"fill_container",height:40,padding:[0,12],gap:8,alignItems:"center",fill:dark?"$surface-2":"$canvas",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",cornerRadius:4});
  I(search,"Search Icon","search","$muted",15);
  T(search,"Search Ph","搜索交易对",11,"450","$muted");
  const list=Insert(sheet,{type:"frame",name:"Pair List",layout:"vertical",width:"fill_container",gap:2});
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
  T(sheet,"Sheet Note","最新价与收益率来自行情与产品接口，不做占位虚构。",10,"450","$muted",320);
  Update(s,{placeholder:false});
  return s;
}
const pl=picker(false);
const pd=picker(true);
Print("PAIR_PICKER chevrons="+pairFrames.length);
Print("LIGHT_ID="+pl);
Print("DARK_ID="+pd);
