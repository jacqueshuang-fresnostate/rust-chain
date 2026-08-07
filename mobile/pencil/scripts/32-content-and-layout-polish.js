// Content truthfulness and layout polish for newly added boards.
function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}
function primary(p,label){const b=Insert(p,{type:"frame",name:"Primary "+label,layout:"horizontal",width:"fill_container",height:50,gap:8,justifyContent:"center",alignItems:"center",fill:"$mint",cornerRadius:4});I(b,"Primary Icon","arrow-left-right","#07110D",17);T(b,"Primary Label",label,13,"650","#07110D");return b;}

// Remove notes that were appended after a flex spacer.
for(const id of ["UCfrp","kq9AZ"]){try{Delete(id);}catch(e){}}

// Truthful placeholders instead of fabricated product/market data.
const roots={A9It6g:'ncr',h4gfd:'ncr',CzpTv:'pred',ZvGMv:'pred',nqP6W:'earn',aXxul:'earn',v6phV:'transfer',TuWXq:'transfer'};
const topIds=new Set();
Get((n,c)=>{c.skipChildren();if(n.type==='frame')topIds.add(n.id);});
let mode=null; const updates=[];
Get((n,c)=>{
  if(topIds.has(n.id)){mode=roots[n.id]||null;return;}
  if(mode==='ncr'){
    if(n.name==='HIPPO Sym')updates.push([n.id,'项目 #— · 认购确认']);
    if(n.name==='HIPPO Meta')updates.push([n.id,'申请与分配数量由接口返回']);
    if(n.name==='ORBIT Sym')updates.push([n.id,'项目 #— · 待支付解锁费']);
    if(n.name==='ORBIT Meta')updates.push([n.id,'锁定数量由接口返回']);
    if(n.name==='NOVA Sym')updates.push([n.id,'项目 #— · 分发到账']);
    if(n.name==='NOVA Meta')updates.push([n.id,'到账数量由接口返回']);
  }
  if(mode==='pred'){
    if(n.name==='Cat')updates.push([n.id,'分类与截止时间由接口返回']);
    if(n.name==='Q')updates.push([n.id,'预测市场问题由接口返回']);
    if(n.name==='Stake V')updates.push([n.id,'0.00']);
  }
  if(mode==='earn'){
    if(n.name==='Eyebrow')updates.push([n.id,'EARN / ASSET']);
    if(n.name==='Name')updates.push([n.id,'理财产品名称由接口返回']);
    if(n.name==='Apy')updates.push([n.id,'收益率由产品接口返回']);
    if(n.name==='Amt V')updates.push([n.id,'0.00']);
  }
  if(mode==='transfer'){
    if(n.name==='Faux V')updates.push([n.id,'— USDT']);
    if(n.name==='Faux R1'||n.name==='Faux R2')updates.push([n.id,'持仓由账户接口返回']);
  }
});
for(const u of updates)Update(u[0],{content:u[1]});

// Reorder Orders empty CTA: spacer first, CTA last.
const oldOrderNodes=["lVFja","KUjDo","HVS32","IsfLi","pRiRu","VzxJP","bxs9d","ZiKRA","pQOZn","YV5R0"];
for(const id of oldOrderNodes){try{Delete(id);}catch(e){}}
for(const id of ["e5Qs1","hxe8l"]){
  Insert(id,{type:"frame",name:"Bottom Spacer",width:"fill_container",height:"fill_container"});
  const wrap=Insert(id,{type:"frame",name:"CTA Wrap",layout:"vertical",width:"fill_container",padding:[0,20,20,20]});
  primary(wrap,"去交易");
}

Print("CONTENT_POLISH updates="+updates.length);
