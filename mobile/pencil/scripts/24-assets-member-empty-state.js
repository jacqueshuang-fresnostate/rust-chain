// Light member = empty-state demo; Dark member = holdings demo.
// Remove dual-state (holdings + empty) on light.
const del=[];
let mode=0;
Get((n,c)=>{
  if(n.id==="p61z2Q"){mode=1;return;}
  if(n.id==="Q4JYj"){mode=0;return;}
  if(mode===1){
    if(n.name&&(n.name.indexOf("Holding ")===0||n.name.indexOf("Coin ")===0||n.name==="Holdings Empty"||n.name==="Empty Icon"||n.name==="Empty Main"||n.name==="Empty Sub"||n.name==="Empty Deposit"||n.name==="Empty Deposit Icon"||n.name==="Empty Deposit Label"||n.name==="Empty Kicker")){
      del.push(n.id);
    }
  }
});
// delete leaves first-ish by reversing
for(let i=del.length-1;i>=0;i--){try{Delete(del[i]);}catch(e){}}

function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}

// find light holdings section after deletes
const lightHoldings=[];
let m=0;
Get((n,c)=>{
  if(n.id==="p61z2Q"){m=1;return;}
  if(n.id==="Q4JYj"){m=0;return;}
  if(m===1&&n.name==="Assets Holdings")lightHoldings.push(n.id);
});
if(!lightHoldings[0])throw new Error("light holdings missing");

// update header meta for empty
let moreId=null;
m=0;
Get((n,c)=>{
  if(n.id==="p61z2Q"){m=1;return;}
  if(n.id==="Q4JYj"){m=0;return;}
  if(m===1&&n.name==="Holdings More")moreId=n.id;
});
if(moreId)Update(moreId,{content:"0 个币种"});

const s=lightHoldings[0];
// flat optimized empty — no heavy card, no dual kicker, aligned with page canvas
const e=Insert(s,{type:"frame",name:"Holdings Empty",layout:"vertical",width:"fill_container",gap:12,padding:[36,20,28,20],justifyContent:"center",alignItems:"center"});
const plate=Insert(e,{type:"frame",name:"Empty Plate",layout:"horizontal",width:56,height:56,cornerRadius:28,fill:"$surface",stroke:"$border",strokeWidth:1,strokeAlignment:"inner",justifyContent:"center",alignItems:"center"});
I(plate,"Empty Icon","wallet-cards","$muted",26);
T(e,"Empty Main","暂无持仓",15,"650","$text");
T(e,"Empty Sub","余额、估值与可用额度连接账户接口后展示",11,"450","$muted",280);
const b=Insert(e,{type:"frame",name:"Empty Deposit",layout:"horizontal",width:"fill_container",height:48,gap:8,justifyContent:"center",alignItems:"center",fill:"$mint",cornerRadius:4});
I(b,"Empty Deposit Icon","arrow-down-to-line","#07110D",17);
T(b,"Empty Deposit Label","去充币",13,"650","#07110D");

// ensure overview/page bg still unified
Update("p61z2Q",{fill:"$canvas"});
Update("Q4JYj",{fill:"#000000"});
const overview=[];
Get((n,c)=>{if(n.name==="Portfolio Member Overview")overview.push(n.id);});
for(const id of overview)Update(id,{fill:"#00000000",stroke:"#00000000",strokeWidth:0,padding:[8,16,4,16]});

Print("EMPTY_STATE deleted="+del.length+" holdings="+lightHoldings[0]);
