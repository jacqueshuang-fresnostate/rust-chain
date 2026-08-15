// 39b transfer asset picker: documented after the boards were written into the .pen.
// Boards: tPkL1 (light) / tPkD1 (dark). Backdrop = faux transfer + dim + Asset Sheet.
// Re-running is a no-op unless those IDs are missing.
const need=["tPkL1","tPkD1"];
const have={};
Get((n,c)=>{c.skipChildren();if(n.type==="frame"&&need.indexOf(n.id)>=0)have[n.id]=true;});
if(have.tPkL1&&have.tPkD1){Print("ASSET_PICKER_EXISTS light=tPkL1 dark=tPkD1");}
else{Print("ASSET_PICKER_MISSING "+JSON.stringify(have));throw new Error("39b boards missing; restore from hippo-mobile-uiux.pen");}
