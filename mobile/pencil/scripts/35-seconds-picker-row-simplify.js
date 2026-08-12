// Simplify picker pair rows: drop "秒合约·现货钱包结算" tag and right-side payout.
// Scoped strictly to Pair Sheet subtrees inside the two picker boards.
const boards=new Set(["vONcc","kLXCs"]);
const tops=new Set();
Get((n,c)=>{c.skipChildren();if(n.type==="frame")tops.add(n.id);});
let board=null,inSheet=false;
const del=[];const notes=[];
Get((n,c)=>{
  if(tops.has(n.id)){board=boards.has(n.id)?n.id:null;inSheet=false;return;}
  if(!board)return;
  if(n.name==="Pair Sheet"){inSheet=true;return;}
  if(!inSheet)return;
  if(n.name==="Pair Tag"||n.name==="Pair Payout")del.push(n.id);
  if(n.name==="Sheet Note")notes.push(n.id);
});
for(const id of del){try{Delete(id);}catch(e){}}
for(const id of notes)Update(id,{content:"最新价来自行情接口，不做占位虚构。"});
Print("ROW_SIMPLIFY deleted="+del.length+" notes="+notes.length);
