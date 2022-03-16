import { createMetadata, Data, Creator, Metadata } from "./metadata/metadata";
import assert from "assert";

import { TOKEN_PROGRAM_ID, Token } from "@solana/spl-token";
import * as anchor from "@project-serum/anchor";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  Keypair,
} from "@solana/web3.js";

const BID_AMOUNT = 100000;

const salesTaxRecipientPubkey = new PublicKey(
  "3iYf9hHQPciwgJ1TCjpRUp1A3QW4AfaK7J6vCmETRMuu"
);

describe("bid-contract", () => {
  // Address of the deployed program.
  const programId = new anchor.web3.PublicKey(
    "79wHQurshPcvuFAe7shqsp5xQEhMXpudeJ8twwmTgHbx"
  );
  const idl = JSON.parse(
    require("fs").readFileSync("./target/idl/bid_contract.json", "utf8")
  );

  const myWallet = anchor.web3.Keypair.fromSecretKey(
    new Uint8Array(
      JSON.parse(require("fs").readFileSync(process.env.MY_WALLET, "utf8"))
    )
  );

  const connection = new anchor.web3.Connection(
    "https://api.devnet.solana.com/",
    "confirmed"
  );

  const walletWrapper = new anchor.Wallet(myWallet);

  const provider = new anchor.Provider(connection, walletWrapper, {
    preflightCommitment: "recent",
  });
  const program = new anchor.Program(idl, programId, provider);

  const bidder = anchor.web3.Keypair.fromSecretKey(
    new Uint8Array(
      JSON.parse(require("fs").readFileSync("./tests/keys/bidder.json", "utf8"))
    )
  );

  const owner = anchor.web3.Keypair.fromSecretKey(
    new Uint8Array(
      JSON.parse(require("fs").readFileSync("./tests/keys/owner.json", "utf8"))
    )
  );

  const creator1 = anchor.web3.Keypair.fromSecretKey(
    new Uint8Array(
      JSON.parse(
        require("fs").readFileSync("./tests/keys/creator1.json", "utf8")
      )
    )
  );

  let bid: PublicKey;
  let bump_bid: number;
  let vault: PublicKey;
  let bump_vault: number;
  let mint: Token;
  let tokenPubkey: PublicKey;

  const creator2 = anchor.web3.Keypair.generate();

  let metadata: PublicKey;

  it("Add to Vault", async () => {
    [vault, bump_vault] = await PublicKey.findProgramAddress(
      [Buffer.from("bidvault"), bidder.publicKey.toBuffer()],
      program.programId
    );

    await program.rpc.addToVault(new anchor.BN(BID_AMOUNT * 4), bump_vault, {
      accounts: {
        bidder: bidder.publicKey,
        vault,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [bidder],
    });

    const vaultAccount = await provider.connection.getAccountInfo(vault);
    console.log(vaultAccount.lamports);
  });

  it("Withdraw from Vault", async () => {
    [vault, bump_vault] = await PublicKey.findProgramAddress(
      [Buffer.from("bidvault"), bidder.publicKey.toBuffer()],
      program.programId
    );

    await program.rpc.withdrawFromVault(new anchor.BN(BID_AMOUNT), bump_vault, {
      accounts: {
        bidder: bidder.publicKey,
        vault,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [bidder],
    });

    const vaultAccount = await provider.connection.getAccountInfo(vault);
    console.log(vaultAccount.lamports);
  });

  it("Init Bid", async () => {
    mint = await Token.createMint(
      provider.connection,
      owner,
      owner.publicKey,
      null,
      0,
      TOKEN_PROGRAM_ID
    );

    tokenPubkey = await mint.createAccount(owner.publicKey);
    await mint.mintTo(tokenPubkey, owner.publicKey, [owner], 1);

    [bid, bump_bid] = await PublicKey.findProgramAddress(
      [
        Buffer.from("bid"),
        bidder.publicKey.toBuffer(),
        mint.publicKey.toBuffer(),
      ],
      program.programId
    );
    await program.rpc.initBid(new anchor.BN(BID_AMOUNT), bump_bid, {
      accounts: {
        bidder: bidder.publicKey,
        mint: mint.publicKey,
        bid,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [bidder],
    });

    const bidAccount = await program.account.bidAccount.fetch(bid);
    assert.ok(bidAccount.bidder.equals(bidder.publicKey));
    assert.ok(bidAccount.mint.equals(mint.publicKey));
    assert.ok(bidAccount.bidAmount.toNumber() == BID_AMOUNT);
  });

  it("Update Bid", async () => {
    await program.rpc.updateBid(new anchor.BN(BID_AMOUNT * 2), {
      accounts: {
        bidder: bidder.publicKey,
        mint: mint.publicKey,
        bid,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [bidder],
    });
    const bidAccount = await program.account.bidAccount.fetch(bid);
    assert.ok(bidAccount.bidAmount.toNumber() == BID_AMOUNT * 2);
  });

  it("Accept Bid", async () => {
    const signers = [creator1, owner];
    let instructions = [];
    metadata = await createMetadata(
      new Data({
        name: "somename",
        symbol: "SOME",
        uri: "https://somelink.come/someid",
        sellerFeeBasisPoints: 500,
        creators: [
          new Creator({
            address: creator1.publicKey,
            verified: true,
            share: 80,
          }),
          new Creator({
            address: creator2.publicKey,
            verified: false,
            share: 20,
          }),
        ],
      }),
      creator1.publicKey, // update authority
      mint.publicKey,
      owner.publicKey, // mint authority
      instructions,
      creator1.publicKey
    );
    const transaction = new Transaction();
    instructions.forEach((instruction) => transaction.add(instruction));
    transaction.recentBlockhash = (
      await connection.getRecentBlockhash("singleGossip")
    ).blockhash;

    transaction.setSigners(...signers.map((s) => s.publicKey));
    // transaction.partialSign(...signers);

    await sendAndConfirmTransaction(connection, transaction, signers, {
      skipPreflight: true,
    });

    const bidderTokenPubkey = await mint.createAccount(bidder.publicKey);

    // accept bid
    console.log(creator1.publicKey.toBase58());
    console.log(creator2.publicKey.toBase58());
    await program.rpc.acceptBid(new anchor.BN(BID_AMOUNT * 2), bump_vault, {
      accounts: {
        owner: owner.publicKey,
        mint: mint.publicKey,
        token: tokenPubkey,
        bidder: bidder.publicKey,
        bidderToken: bidderTokenPubkey,
        bid,
        vault,
        salesTaxRecipient: salesTaxRecipientPubkey,
        metadata,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
      remainingAccounts: [
        { pubkey: creator1.publicKey, isWritable: true, isSigner: false },
        { pubkey: creator2.publicKey, isWritable: true, isSigner: false },
      ],
      signers: [owner],
    });

    assert.ok(
      (await mint.getAccountInfo(bidderTokenPubkey)).amount.toNumber() == 1
    );
    assert.ok((await mint.getAccountInfo(tokenPubkey)).amount.toNumber() == 0);

    // const salesTax = await connection.getAccountInfo(salesTaxRecipientPubkey);
    // console.log(salesTax.lamports);
    const creator1Account = await connection.getAccountInfo(creator1.publicKey);
    console.log(creator1Account.lamports);
    const creator2Account = await connection.getAccountInfo(creator2.publicKey);
    console.log(creator2Account.lamports);
    const salesTaxAccount = await connection.getAccountInfo(
      salesTaxRecipientPubkey
    );
    console.log(salesTaxAccount.lamports);
  });

  it("Init & Cancel Bid", async () => {
    await program.rpc.initBid(new anchor.BN(BID_AMOUNT), bump_bid, {
      accounts: {
        bidder: bidder.publicKey,
        mint: mint.publicKey,
        bid,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [bidder],
    });

    await program.rpc.cancelBid({
      accounts: {
        bidder: bidder.publicKey,
        mint: mint.publicKey,
        bid,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [bidder],
    });
  });
});
