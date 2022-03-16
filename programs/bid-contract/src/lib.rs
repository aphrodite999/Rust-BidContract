pub mod metadata;
use anchor_lang::{prelude::*, solana_program::system_program};
use anchor_spl::token::{self, Mint, TokenAccount, Transfer};

declare_id!("bidPUtgGxbSqVTv1zYtAysAvPW4J5LqcUfhp3xNJECB");

const SALES_TAX_RECIPIENT_INTERNAL: &str = "3iYf9hHQPciwgJ1TCjpRUp1A3QW4AfaK7J6vCmETRMuu";
const SALES_TAX: u64 = 250;

const PREFIX_BID: &str = "bid";
const PREFIX_VAULT: &str = "bidvault";

#[program]
pub mod bid_contract {
    use super::*;
    use crate::metadata::{get_metadata_account, BidError, Metadata};
    use anchor_lang::solana_program::{
        program::{invoke, invoke_signed},
        system_instruction,
    };

    pub fn add_to_vault(ctx: Context<AddToVault>, amount: u64, _bump_vault: u8) -> ProgramResult {              // function to send bid amount to NFT vault.
        invoke(
            &system_instruction::transfer(
                ctx.accounts.bidder.key,                                                                        // This is bidder key
                &ctx.accounts.vault.key(),                                                                      // value of bidder key
                amount,
            ),
            &[
                ctx.accounts.bidder.clone(),
                ctx.accounts.vault.clone(),
                ctx.accounts.system_program.clone(),
            ],
        )?;
        Ok(())
    }

    pub fn withdraw_from_vault(                                                                                    // function to withdraw own bid amount from NFT Vault
        ctx: Context<WithdrawFromVault>,
        amount: u64,
        bump_vault: u8,
    ) -> ProgramResult {
        let authority_seeds = [
            PREFIX_VAULT.as_bytes(),
            ctx.accounts.bidder.key.as_ref(),
            &[bump_vault],
        ];
        invoke_signed(                                                                                              // invoke sign transaction to withdraw from NFT vault to bidder. 
            &system_instruction::transfer(
                &ctx.accounts.vault.key(),
                ctx.accounts.bidder.key,
                amount,
            ),
            &[
                ctx.accounts.vault.clone(),
                ctx.accounts.bidder.clone(),
                ctx.accounts.system_program.clone(),
            ],
            &[&authority_seeds],
        )?;
        Ok(())
    }

    pub fn init_bid(ctx: Context<InitBid>, bid_amount: u64, bump_bid: u8) -> ProgramResult {    // This is function to initialize bid.
        let bid = &mut ctx.accounts.bid;    //  ctx.accounts.bid is indicate bid instance to initialize bid.
        bid.bidder = *ctx.accounts.bidder.key;  //  
        bid.mint = *ctx.accounts.mint.to_account_info().key;
        bid.bid_amount = bid_amount;
        bid.bump = bump_bid;

        Ok(())
    }

    pub fn update_bid(ctx: Context<UpdateBid>, bid_amount: u64) -> ProgramResult {  // This is function to update with new new amount in old bid.
        let bid = &mut ctx.accounts.bid;        //  ctx.accounts.bid is indicate bid instance to update bid.
        bid.bid_amount = bid_amount;            //  bid_amount is new bid amount to update bid.
        Ok(())
    }

    pub fn cancel_bid(_ctx: Context<CancelBid>) -> ProgramResult {  // This is function to cancel old bid. I think this function was not completed yet.
        Ok(())
    }

    pub fn accept_bid<'a, 'b, 'c, 'info>(
        ctx: Context<'_, '_, '_, 'info, AcceptBid<'info>>,
        bid_amount: u64,
        bump_vault: u8,
    ) -> ProgramResult {                                            // This is function to accept bid
        let price = ctx.accounts.bid.bid_amount;                    // bid price to accept
        if price != bid_amount {                                    // when price is different from bid amount, return error value 
            return Err(BidError::BidAmountMismatch.into());
        }
        let authority_seeds = [
            PREFIX_VAULT.as_bytes(),
            ctx.accounts.bidder.key.as_ref(),
            &[bump_vault],
        ];
        invoke_signed(                                                          // function to sign transaction
            &system_instruction::transfer(ctx.accounts.vault.key, ctx.accounts.owner.key, price),           // key of vault and owner key that make highest bid
            &[
                ctx.accounts.vault.clone(),
                ctx.accounts.owner.clone(),
                ctx.accounts.system_program.clone(),
            ],
            &[&authority_seeds],
        )?;

        token::transfer(ctx.accounts.into_transfer_to_bidder_context(), 1)?;

        // sales tax
        let tax_amount = (SALES_TAX * price) / 10000;                           // consider to tax amount for sale.
        invoke(                                                                 // make transaction to send token from owner to NFT Taxes
            &system_instruction::transfer(
                ctx.accounts.owner.key,
                ctx.accounts.sales_tax_recipient.key,
                tax_amount,
            ),
            &[
                ctx.accounts.owner.clone(),
                ctx.accounts.sales_tax_recipient.clone(),
                ctx.accounts.system_program.clone(),
            ],
        )?;

        // royalty for creators
        let metadata = &ctx.accounts.metadata;
        let correct_metadata = get_metadata_account(ctx.accounts.mint.to_account_info().key);                   // confirm between account metadata and old bidder own metadata
        if &correct_metadata != metadata.key {                                                                  // if these are not match, return error value
            msg!(
                "Mint-derived metadata account {:?} doesn't match passed metadata account {:?}",
                &correct_metadata,
                metadata.key
            );
            return Err(BidError::InvalidMetadata.into());
        }

        let creators = &ctx.remaining_accounts;
        let meta_res = Metadata::from_u8(&metadata.data.borrow_mut());
        let mut royalty_total: u64 = 0;
        if meta_res.is_ok() {
            let md = meta_res.unwrap();
            if md.data.seller_fee_basis_points as u64 + SALES_TAX > 10000 {                 
                return Err(BidError::InvalidRoyaltyFee.into());
            }
            royalty_total = (md.data.seller_fee_basis_points as u64 * price) / 100000;

            msg!("Distributing creator royalties");

            // TODO check verified status
            match md.data.creators {
                Some(md_creators) => {
                    if md_creators.len() != creators.len() {                                                                                                        // compare account metadata with existing accounts in nft.
                        msg!("number of creators in metadata {:?} doesn't match number of creators passed {:?}", md_creators.len(), creators.len());
                        return Err(BidError::CreatorMismatch.into());
                    }
                    for (i, mcreator) in md_creators.iter().enumerate() {
                        let creator = &creators[i];
                        if mcreator.address != *creator.key {
                            msg!(
                                "creator {:?} in metadata {:?} doesn't match creator passed {:?}",
                                i,
                                mcreator.address,
                                creator.key
                            );
                            return Err(BidError::CreatorMismatch.into());
                        }

                        let creator_royalty = (mcreator.share as u64 * royalty_total) / 100;                            // confirm royalty amount when owner is buy NFT.

                        invoke(
                            &system_instruction::transfer(                                                              // make transaction from owner to creator with royalty amount. 
                                ctx.accounts.owner.key,
                                creator.key,
                                creator_royalty,
                            ),
                            &[
                                ctx.accounts.owner.clone(),
                                creator.clone(),
                                ctx.accounts.system_program.clone(),
                            ],
                        )?;
                    }
                }
                None => msg!("no creators => no payouts"),
            }
        } else {                                                                                        // meta responsive is not OKAY, return error value
            if let Err(e) = meta_res {
                msg!(
                    "no metadata found or metadata invalid, skipping royalties: {:?}",
                    e
                );
            }
        }
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(amount: u64, _bump_vault: u8)]
pub struct AddToVault<'info> {
    #[account(mut, signer)]
    bidder: AccountInfo<'info>,
    #[account(
        mut,
        seeds=[PREFIX_VAULT.as_bytes(), bidder.key().as_ref()],
        bump=_bump_vault,
    )]
    vault: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
}


#[derive(Accounts)]
#[instruction(amount: u64, bump_vault: u8)]
pub struct WithdrawFromVault<'info> {
    #[account(mut, signer)]
    bidder: AccountInfo<'info>,
    #[account(
        mut,
        seeds=[PREFIX_VAULT.as_bytes(), bidder.key().as_ref()],
        bump=bump_vault,
    )]
    vault: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(bid_amount: u64, bump_bid: u8)]
pub struct InitBid<'info> {
    #[account(mut, signer)]
    bidder: AccountInfo<'info>,
    mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        seeds=[PREFIX_BID.as_bytes(), bidder.key().as_ref(), mint.key().as_ref()],
        bump=bump_bid,
        payer=bidder,
        space=8+32*3+8+1)]
    bid: ProgramAccount<'info, BidAccount>,
    #[account(address = system_program::ID)]
    system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(bid_amount: u64)]
pub struct UpdateBid<'info> {
    #[account(mut, signer)]
    bidder: AccountInfo<'info>,
    mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        seeds=[PREFIX_BID.as_bytes(), bidder.key().as_ref(), mint.key().as_ref()],
        bump=bid.bump,
        )]
    bid: ProgramAccount<'info, BidAccount>,
    #[account(address = system_program::ID)]
    system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CancelBid<'info> {
    #[account(mut, signer)]
    bidder: AccountInfo<'info>,
    mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        has_one = bidder,
        has_one = mint,
        seeds = [PREFIX_BID.as_bytes(), bidder.key().as_ref(), mint.key().as_ref()],
        bump = bid.bump,
        close = bidder
    )]
    bid: ProgramAccount<'info, BidAccount>,
    #[account(address = system_program::ID)]
    system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(bid_amount: u64, bump_vault: u8)]
pub struct AcceptBid<'info> {
    #[account(mut, signer)]
    owner: AccountInfo<'info>,
    mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        // has_one = mint,
        constraint = token.mint == mint.key(),
    )]
    token: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    bidder: AccountInfo<'info>,
    #[account(
        mut,
        // has_one = mint,
        constraint = bidder_token.owner == bidder.key(),
        constraint = bidder_token.mint == mint.key(),
    )]
    bidder_token: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        has_one = bidder,
        has_one = mint,
        seeds = [PREFIX_BID.as_bytes(), bidder.key().as_ref(), mint.key().as_ref()],
        bump = bid.bump,
        close = bidder
    )]
    bid: ProgramAccount<'info, BidAccount>,
    #[account(
        mut,
        seeds=[PREFIX_VAULT.as_bytes(), bidder.key().as_ref()],
        bump=bump_vault,
    )]
    vault: AccountInfo<'info>,
    #[account(mut, constraint = sales_tax_recipient.key.to_string() == SALES_TAX_RECIPIENT_INTERNAL)]
    pub sales_tax_recipient: AccountInfo<'info>,
    pub metadata: AccountInfo<'info>,
    #[account(address = system_program::ID)]
    system_program: AccountInfo<'info>,
    #[account(address = spl_token::id())]
    pub token_program: AccountInfo<'info>,
}

#[account]
pub struct BidAccount {
    pub bidder: Pubkey,
    pub mint: Pubkey,
    pub bid_amount: u64,
    pub bump: u8,
}

impl<'info> AcceptBid<'info> {
    fn into_transfer_to_bidder_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.token.to_account_info().clone(),
            to: self.bidder_token.to_account_info().clone(),
            authority: self.owner.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}
