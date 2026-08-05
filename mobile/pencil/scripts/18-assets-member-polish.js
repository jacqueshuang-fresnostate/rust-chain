const swap=[],ovl=[],cards=[],bg264=[],empty=[];
Get((n,c)=>{
  if(n.name==="Card BG Image"&&n.fill&&n.fill.url==="images/generated-1785687482899.png")swap.push(n.id);
  if(n.name==="Card BG Overlay"&&n.fill==="#00000026")ovl.push(n.id);
  if(n.name==="Member Hero")cards.push(n.id);
  if((n.name==="Card BG Image"||n.name==="Card BG Overlay")&&n.height===264)bg264.push(n.id);
  if(n.name==="Holdings Empty")empty.push(n.id);
});
for(const id of swap)Update(id,{fill:{type:"image",enabled:true,url:"images/generated-1785687557714.png",mode:"fill"}});
for(const id of ovl)Update(id,{fill:"#00000040"});
for(const id of cards)Update(id,{height:236,clip:true});
for(const id of bg264)Update(id,{height:236});
for(const id of empty)Update(id,{padding:[14,16]});
Print("POLISHED swap="+swap.length+" overlay="+ovl.length+" cards="+cards.length+" bg="+bg264.length+" empty="+empty.length);
