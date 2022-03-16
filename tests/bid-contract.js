const anchor = require("@project-serum/anchor");
const { PublicKey, Transaction, SystemProgram } = anchor.web3;
const { TOKEN_PROGRAM_ID, Token } = require("@solana/spl-token");
const assert = require("assert");

describe("bid-contract", () => {
  const provider = anchor.Provider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.BidContract;

  const BID_AMOUNT = 1000000;
  const bidder = anchor.web3.Keypair.generate();
  const owner = anchor.web3.Keypair.generate();
  const mintAuthority = anchor.web3.Keypair.generate();

  let escrow;
  let bump_escrow;
  let vault;
  let bump_vault;
  let mint;
  let tokenPubkey;

  it("Init & Cancel Bid", async () => {
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(bidder.publicKey, 1000000000),
      "confirmed"
    );

    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(owner.publicKey, 1000000000),
      "confirmed"
    );

    mint = await Token.createMint(
      provider.connection,
      owner,
      mintAuthority.publicKey,
      null,
      0,
      TOKEN_PROGRAM_ID
    );

    tokenPubkey = await mint.createAccount(owner.publicKey);
    await mint.mintTo(tokenPubkey, mintAuthority.publicKey, [mintAuthority], 1);

    [escrow, bump_escrow] = await PublicKey.findProgramAddress(
      [
        Buffer.from("bidescrow"),
        bidder.publicKey.toBuffer(),
        tokenPubkey.toBuffer(),
      ],
      program.programId
    );
    [vault, bump_vault] = await PublicKey.findProgramAddress(
      [Buffer.from("bidvault"), bidder.publicKey.toBuffer()],
      program.programId
    );
    await program.rpc.initBid(
      new anchor.BN(BID_AMOUNT),
      bump_escrow,
      bump_vault,
      {
        accounts: {
          bidder: bidder.publicKey,
          mint: mint.publicKey,
          token: tokenPubkey,
          escrow,
          vault,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [bidder],
      }
    );

    const vaultAccount = await provider.connection.getAccountInfo(vault);
    assert.ok(vaultAccount.lamports == BID_AMOUNT + 1000);
    const escrowAccount = await program.account.bidEscrowAccount.fetch(escrow);
    assert.ok(escrowAccount.bidder.equals(bidder.publicKey));
    assert.ok(escrowAccount.mint.equals(mint.publicKey));
    assert.ok(escrowAccount.token.equals(tokenPubkey));
    assert.ok(escrowAccount.bidAmount.toNumber() == BID_AMOUNT);

    await program.rpc.cancelBid(bump_vault, {
      accounts: {
        bidder: bidder.publicKey,
        mint: mint.publicKey,
        token: tokenPubkey,
        escrow,
        vault,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [bidder],
    });

    const bidderAccount = await provider.connection.getAccountInfo(
      bidder.publicKey
    );
    assert.ok(bidderAccount.lamports == 1000000000 - 1000);
  });
});
