// clean collapsed empty shells left by previous pass
for(const id of ["I8CQyv","NOS89","tM9Aj","oJDOk","u2052w","zYznb","EXbD7","Tm627"]){try{Delete(id);}catch(e){}}

function T(p,n,c,z,w,f,wd,ff){const o={type:"text",name:n,content:c,fontFamily:ff||"$font-sans",fontSize:z,fontWeight:w||"400",fill:f||"$text",lineHeight:1.25};if(wd){o.textGrowth="fixed-width";o.width=wd;}return Insert(p,o);}
function I(p,n,k,f,z){return Insert(p,{type:"icon",name:n,library:"lucide",icon:k,width:z||20,height:z||20,fill:f||"$text"});}

// locate light holdings section and ensure pure lucide coin icons (scoped)
const lightHoldings=[];const coinFrames=[];const coinIcons=[];
let mode=0;
Get((n,c)=>{
  if(n.id==="p61z2Q"){mode=1;return;}
  if(n.id==="Q4JYj"){mode=2;return;}
  if(mode===1){
    if(n.name==="Assets Holdings")lightHoldings.push(n.id);
    if(n.name&&n.name.indexOf("Coin ")===0&&n.type==="frame")coinFrames.push(n.id);
    if(n.name==="Coin Icon"&&n.type==="icon")coinIcons.push(n.id);
  }
  if(mode===2){
    if(n.name&&n.name.indexOf("Coin ")===0&&n.type==="frame")coinFrames.push(n.id);
    if(n.name==="Coin Icon"&&n.type==="icon")coinIcons.push(n.id);
  }
});

// pure icon treatment: no plate, mint icon at 20px
for(const id of coinFrames)Update(id,{fill:"#00000000",stroke:"#00000000",strokeWidth:0,width:22,height:22,cornerRadius:0,justifyContent:"center",alignItems:"center"});
for(const id of coinIcons)Update(id,{width:20,height:20,fill:"$mint-strong"});

// optimized empty under light holdings only: flat, no heavy card, no dual kicker
if(lightHoldings[0]){
  const s=lightHoldings[0];
  const e=Insert(s,{type:"frame",name:"Holdings Empty",layout:"vertical",width:"fill_container",gap:10,padding:[20,0,8,0],justifyContent:"center",alignItems:"center"});
  I(e,"Empty Icon","wallet-cards","$muted",28);
  T(e,"Empty Main","暂无持仓",14,"650","$text");
  T(e,"Empty Sub","充币后展示币种数量、估值与可用余额",11,"450","$muted",280);
  const b=Insert(e,{type:"frame",name:"Empty Deposit",layout:"horizontal",width:200,height:44,gap:8,justifyContent:"center",alignItems:"center",fill:"$mint",cornerRadius:4});
  I(b,"Empty Deposit Icon","arrow-down-to-line","#07110D",17);
  T(b,"Empty Deposit Label","去充币",13,"650","#07110D");
}

// dark root already set; ensure light root stays $canvas
Update("p61z2Q",{fill:"$canvas"});
Update("Q4JYj",{fill:"#000000"});

Print("EMPTY_ICONS lightHoldings="+lightHoldings.length+" coins="+coinFrames.length+" icons="+coinIcons.length);
