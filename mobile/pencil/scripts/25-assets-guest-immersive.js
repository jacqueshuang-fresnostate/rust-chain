// Guest assets: same immersive card shell as member, content = login prompt + button only.
const TOPS=new Set();
Get((n,c)=>{c.skipChildren(); if(n.type==="frame")TOPS.add(n.id);});
const doomed=[]; let root=null;
Get((n,c)=>{
  if(TOPS.has(n.id)){root=n.id;return;}
  if(root==="CUK3y"||root==="i6YDBr")doomed.push(n.id);
});
for(let i=doomed.length-1;i>=0;i--){try{Delete(doomed[i]);}catch(e){}}

function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}
function status(p){const a=Insert(p,{type:"frame",name:"Status Bar",layout:"horizontal",width:"fill_container",height:28,padding:[0,16],justifyContent:"space_between",alignItems:"center"});T(a,"Time","09:41",11,"650","$text",undefined,"$font-data");const b=Insert(a,{type:"frame",name:"Status Signals",layout:"horizontal",gap:8,alignItems:"center"});T(b,"Network","4G+",10,"550","$muted",undefined,"$font-data");I(b,"Wifi","wifi","$text",14);T(b,"Battery","82%",10,"550","$text",undefined,"$font-data");}
function header(p){const h=Insert(p,{type:"frame",name:"Assets Header",layout:"horizontal",width:"fill_container",padding:[12,20,4,20],justifyContent:"space_between",alignItems:"center",fill:"#00000000"});T(h,"Title","资产",22,"750");}
// Card shell mirrors member: h=236, pad=[18,20,16,20], radius-l, silk bg, bloom.
function hero(p,dark){
  const band=Insert(p,{type:"frame",name:"Portfolio Guest Overview",layout:"vertical",width:"fill_container",gap:8,padding:[8,16,4,16],fill:"#00000000",stroke:"#00000000"});
  const card=Insert(band,{type:"frame",name:"Guest Hero",layout:"vertical",width:"fill_container",height:236,cornerRadius:"$radius-l",gap:14,padding:[18,20,16,20],stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"space_between",clip:true});
  Insert(card,{type:"rectangle",name:"Card BG Image",layoutPosition:"absolute",x:0,y:0,width:358,height:236,fill:{type:"image",enabled:true,url:dark?"images/generated-1785685909638.png":"images/generated-1785687557714.png",mode:"fill"}});
  Insert(card,{type:"rectangle",name:"Card BG Overlay",layoutPosition:"absolute",x:0,y:0,width:358,height:236,fill:dark?"#00000040":"#FFFFFF00"});
  Insert(card,{type:"ellipse",name:"Bloom",layoutPosition:"absolute",x:186,y:-64,width:220,height:220,fill:{type:"gradient",gradientType:"radial",enabled:true,rotation:0,size:{width:1,height:1},colors:[{color:"#43EFA92E",position:0},{color:"#43EFA900",position:1}]}});
  const ink=dark?"#FFFFFF":"$text";
  const sub=dark?"#9BA8A1":"$muted";
  const top=Insert(card,{type:"frame",name:"Hero Head",layout:"vertical",width:"fill_container",gap:8});
  T(top,"Guest Kicker","GUEST / SIGN IN",11,"600",sub,undefined,"$font-data");
  T(top,"Guest Title","登录后查看资产",26,"750",ink);
  T(top,"Guest Sub","余额、估值与持仓仅在登录后同步",12,"450",sub,300);
  const cta=Insert(card,{type:"frame",name:"Guest Login",layout:"horizontal",width:"fill_container",height:50,gap:8,justifyContent:"center",alignItems:"center",fill:dark?"#FFFFFF14":"#FFFFFF99",stroke:dark?"#FFFFFF26":"#FFFFFFCC",strokeWidth:1,strokeAlignment:"inner",cornerRadius:"$radius-m",effect:dark?undefined:{type:"background_blur",radius:18}});
  T(cta,"Guest Login Label","登录查看资产",14,"650",ink);
  I(cta,"Guest Login Arrow","arrow-right",ink,17);
}
function spacer(p){Insert(p,{type:"frame",name:"Bottom Spacer",width:"fill_container",height:"fill_container"});}
function entry(d,it,on){const c=Insert(d,{type:"frame",name:"Nav "+it[0],layout:"vertical",width:"fill_container",gap:4,justifyContent:"center",alignItems:"center"});I(c,it[0]+" Icon",it[1],on?"$mint-strong":"$muted",22);T(c,it[0]+" Label",it[0],10,on?"650":"500",on?"$text":"$muted");}
function nav(p){
  const n=Insert(p,{type:"frame",name:"Bottom Navigation",width:"fill_container",height:84,padding:[6,16,10,16],fill:"#00000000",stroke:"#00000000",strokeAlignment:"inner"});
  const d=Insert(n,{type:"frame",name:"Nav Dock",layout:"horizontal",width:"fill_container",height:68,padding:[0,16],cornerRadius:24,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",alignItems:"center",effect:{type:"shadow",shadowType:"outer",color:"#07110D14",offset:{x:0,y:8},blur:24}});
  entry(d,["首页","house"],false);entry(d,["行情","chart-line"],false);
  const fs=Insert(d,{type:"frame",name:"FAB Space",layout:"vertical",width:56,height:56,justifyContent:"end",alignItems:"center"});T(fs,"FAB Label","交易",10,"500","$muted");
  entry(d,["资产","wallet-cards"],true);entry(d,["我的","user-round"],false);
  const fab=Insert(n,{type:"frame",name:"Nav FAB",layout:"horizontal",width:56,height:56,cornerRadius:28,fill:"$mint",justifyContent:"center",alignItems:"center",layoutPosition:"absolute",x:167,y:-6,effect:{type:"shadow",shadowType:"outer",color:"#43EFA966",offset:{x:0,y:6},blur:16}});
  I(fab,"FAB Icon","arrow-left-right","#07110D",24);
}
function build(id,dark){
  Update(id,{fill:dark?"#000000":"$canvas",layout:"vertical",clip:true});
  status(id);header(id);hero(id,dark);spacer(id);nav(id);
}
build("CUK3y",false);
build("i6YDBr",true);
Update("CUK3y",{name:"09 / Assets · Light · Guest"});
Update("i6YDBr",{name:"09 / Assets · Dark · Guest"});
Print("GUEST_CARD_PARITY deleted="+doomed.length);
