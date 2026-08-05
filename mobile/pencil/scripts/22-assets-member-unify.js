const overview=[],emptyNodes=[],emptyKickers=[],coinMarks=[],darkRoot=[];
Get((n,c)=>{
  if(n.id==="Q4JYj")darkRoot.push(n.id);
  if(n.name==="Portfolio Member Overview")overview.push(n.id);
  if(n.name==="Holdings Empty"||n.name==="Empty Deposit"||n.name==="Empty Icon"||n.name==="Empty Main"||n.name==="Empty Sub"||n.name==="Empty Deposit Icon"||n.name==="Empty Deposit Label")emptyNodes.push(n.id);
  if(n.name==="Empty Kicker")emptyKickers.push(n.id);
  if(n.name&&n.name.indexOf("Coin ")===0&&n.type==="frame")coinMarks.push(n.id);
});
// page bg: dark member matches Assets Dark / Profile Dark / Orders Dark
for(const id of darkRoot)Update(id,{fill:"#000000"});
// remove surface band around immersive hero — sit on canvas like CUK3y
for(const id of overview)Update(id,{fill:"#00000000",stroke:"#00000000",strokeWidth:0,padding:[8,16,4,16]});
// coin marks: pure lucide, no plate/image look
for(const id of coinMarks)Update(id,{fill:"#00000000",stroke:"#00000000",strokeWidth:0,width:22,height:22,cornerRadius:0});
// remove stacked empty state under populated holdings
for(const id of emptyKickers)Delete(id);
for(const id of emptyNodes){try{Delete(id);}catch(e){}}
// rebuild a clean empty-only block on light member as holdings empty replacement
// (only if light still needs empty coverage — light currently had dual state; now holdings-only)
Print("UNIFY overview="+overview.length+" coins="+coinMarks.length+" emptyDeleted="+emptyNodes.length+" dark="+darkRoot.length);
