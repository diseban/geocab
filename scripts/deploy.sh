#!/bin/bash

# ------------- #
# Configuration #
# ------------- #

# Load variables from .env file
set -o allexport
source scripts/.env
set +o allexport

# Helper constants
DEPLOYMENT_TX_DATA_FILE=deployment_tx_data
ACTIVATION_TX_DATA_FILE=activation_tx_data
DEPLOY_CONTRACT_RESULT_FILE=create_contract_result

# -------------- #
# Initial checks #
# -------------- #
if [ -z "$PRIVATE_KEY" ] || [ -z "$ADDRESS" ]
then
    echo "You need to provide the PRIVATE_KEY and the ADDRESS of the deployer"
    exit 0
fi

# -------------------- #
# Deployment of Geocab #
# -------------------- #
echo ""
echo "----------------------"
echo "Deploying Geocab contract"
echo "----------------------"

# Prepare transactions data
cargo stylus deploy -e $RPC_URL --private-key $PRIVATE_KEY --dry-run --output-tx-data-to-dir .

# Get contract bytecode
bytecode=$(cat $DEPLOYMENT_TX_DATA_FILE | od -An -v -tx1 | tr -d ' \n')
rm $DEPLOYMENT_TX_DATA_FILE

# Send transaction to blockchain
echo "Sending contract creation transaction..."
cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY --create $bytecode > $DEPLOY_CONTRACT_RESULT_FILE

# Get contract address
geocab_contract_address_str=$(cat $DEPLOY_CONTRACT_RESULT_FILE | sed -n 4p)
geocab_contract_address_array=($geocab_contract_address_str)
geocab_contract_address=${geocab_contract_address_array[1]}
rm $DEPLOY_CONTRACT_RESULT_FILE

# Send activation transaction
echo "Sending activation transaction..."
if [ -f ./$ACTIVATION_TX_DATA_FILE ]; then
    cast send --rpc-url $RPC_URL --private-key $PRIVATE_KEY 0x0000000000000000000000000000000000000071 "activateProgram(address)" $geocab_contract_address > /dev/null
    rm $ACTIVATION_TX_DATA_FILE
else
    echo "Not needed, contract already activated"
fi

# Final result
echo "Geocab contract deployed and activated at address: $geocab_contract_address"
