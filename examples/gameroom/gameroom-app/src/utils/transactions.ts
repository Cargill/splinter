// Copyright 2018-2020 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import {
  Secp256k1Signer,
  SabreTransactionBuilder,
  BatchBuilder,
  Secp256k1PrivateKey,
} from 'transact-sdk-javascript';

import protos from '@/protobuf';
import { User } from '@/store/models';
import { XO_FAMILY_NAME, XO_FAMILY_VERSION, XO_FAMILY_PREFIX } from '@/utils/addressing';
import {
  calculateNamespaceRegistryAddress,
  computeContractAddress,
  computeContractRegistryAddress,
} from '@/utils/addressing';

const crypto = require('crypto');

// The Sawtooth Sabre transaction family name (sabre)
const SABRE_FAMILY_NAME = 'sabre';
// The Sawtooth Sabre transaction family version (0.5)
const SABRE_FAMILY_VERSION = '0.5';


export function createTransaction(
  payloadBytes: Uint8Array,
  inputs: string[],
  outputs: string[],
  user: User,
) {
  const privateKey = Secp256k1PrivateKey.fromHex(user.privateKey);
  const signer = new Secp256k1Signer(privateKey);

  return new SabreTransactionBuilder({
    name: 'xo',
    version: XO_FAMILY_VERSION,
    prefix: XO_FAMILY_PREFIX,
  })
    .withBatcherPublicKey(signer.getPublicKey())
    .withFamilyName('xo')
    .withFamilyVersion(XO_FAMILY_PREFIX)
    .withInputs(inputs)
    .withOutputs(outputs)
    .withPayload(payloadBytes)
    .build(signer);
}


export function createBatch(transactions: any, user: User) {
  const privateKey = Secp256k1PrivateKey.fromHex(user.privateKey);
  const signer = new Secp256k1Signer(privateKey);

  const batchListBytes = new BatchBuilder()
    .withTransactions(transactions)
    .build(signer);

  return batchListBytes;
}

function prepare_inputs(contractAddresses: string[]) {
  const returnAddresses = [
    computeContractRegistryAddress(XO_FAMILY_NAME),
    computeContractAddress(XO_FAMILY_NAME, XO_FAMILY_VERSION),
    calculateNamespaceRegistryAddress(XO_FAMILY_PREFIX),
  ];

  return returnAddresses.concat(contractAddresses);

}
