// Remove "秒合约产品由接口返回" subtitle from picker sheet head (both boards).
const boards=new Set(["vONcc","kLXCs"]);
const tops=new Set();
Get((n,c)=>{c.skipChildren();if(n.type==="frame")tops.add(n.id);});
let b=null;const del=[];
Get((n,c)=>{
  if(tops.has(n.id)){b=boards.has(n.id)?n.id:null;return;}
  if(b&&n.name==="Sheet Sub")del.push(n.id);
});
for(const id of del)Delete(id);
Print("SUB_REMOVED="+del.length);
