# bid contract

## Install dependencies

`cargo install --git https://github.com/project-serum/anchor --tag v0.17.0 anchor-cli --locked`

`npm i`


## Run test
testing is done in devnet:  
`npm run test`

## Deployment to Prod
`anchor build`  
`anchor deploy --provider.cluster mainnet`  
first time idl init:  
`anchor idl init -f ./target/idl/bid_contract.json <mainnet_program_id> --provider.cluster mainnet`  
subsequent idl update:  
`anchor idl upgrade -f ./target/idl/bid_contract.json <mainnet_program_id> --provider.cluster mainnet`  




skype : live:.cid.9c92f8e196d2aef5
telegram : @Gold_Strategist
