import type { ProjectCenter } from '../projectModel';
export function center(stage='preheat', id=7): ProjectCenter {
  return { configuration_version:'snapshot-v1', subscription_count:0,pending_manual_count:0,issuance_editable:stage==='preheat',next_lifecycle_status:({preheat:'subscription',subscription:'distribution',distribution:'listed'} as Record<string,string>)[stage]??null,lifecycle_block_reason:null,
    project:{id,asset_id:11,symbol:'HIP',lifecycle_status:stage,status:'active',quote_asset_id:3,total_supply:'100',issue_price:'2.5',reserved_supply:'0',allocated_supply:'0',remaining_supply:'100',unlock_type:'fixed_time',listed_at:1794309753000,actual_listed_at:stage==='listed'?1794309753250:null,fixed_unlock_at:1794409753250,relative_unlock_seconds:null,unlock_fee_enabled:true,unlock_fee_rate:'0.04',unlock_fee_basis:'market_value',unlock_fee_asset:3,post_listing_purchase_enabled:false,post_listing_pair_id:null} };
}
export const manualOrder={id:91,project_id:7,user_id:42,settlement_mode:'manual_distribution',status:'pending',issue_price:'2.5',quote_amount:'25',frozen_quote_amount:'25',requested_quantity:'10',quote_asset:3};
