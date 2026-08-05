const roots={"p61z2Q":1,"Q4JYj":2};
let mode=0;const headerIds=[],navIds=[],eyeIds=[];let grabFor=null;
Get((n,c)=>{
  if(roots[n.id]){mode=roots[n.id];return;}
  if(mode===0){c.skipChildren();return;}
  if(n.name==="Assets Header"&&!headerIds[mode-1]){headerIds[mode-1]=n.id;grabFor=n.id;return;}
  if(grabFor&&n.type==="icon"&&n.name==="Header Action"){eyeIds.push(n.id);grabFor=null;return;}
  if(grabFor&&n.type!=="text"){grabFor=null;}
  if(n.name==="Bottom Navigation"&&!navIds[mode-1])navIds[mode-1]=n.id;
});
for(const h of headerIds)Update(h,{fill:"#00000000"});
for(const id of eyeIds)Delete(id);
for(const n of navIds)Update(n,{layout:"none"});
Print("FIX headers="+headerIds.length+" eyes="+eyeIds.length+" navs="+navIds.length);
