// Copyright 2018-2021 Cargill Incorporated
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

import sjcl from 'sjcl';
import protos from '@/protobuf';

import {
  Secp256k1Context,
  Secp256k1PrivateKey,
  Secp256k1Signer,
} from 'transact-sdk-javascript';

const CRYPTO_CONTEXT = new Secp256k1Context();

/**
 * Returns the SHA-256 hash of the provided salt and data.
 *
 * @param salt - The salt used in the creation of the hash
 * @param data - The data to be hashed
 * @returns The SHA-256 hash of the salt and data
 */
export function hashSHA256(salt: string, data: string): string {
  const out = sjcl.hash.sha256.hash(salt + data);
  return sjcl.codec.hex.fromBits(out);
}

/**
 * Creates a new secp256k1 private key.
 * @returns The new hex-encoded private key.
 */
export function createPrivateKey(): string {
  const privateKey = CRYPTO_CONTEXT.newRandomPrivateKey();
  return privateKey.asHex();
}

/**
 * Derives an secp256k1 public key from a hex-encoded private key.
 * @param privateKey The hex-encoded private key.
 * @returns The hex-encoded public key.
 */
export function getPublicKey(privateKey: string) {
  try {
    const privKey = Secp256k1PrivateKey.fromHex(privateKey);
    const signer = new Secp256k1Signer(privKey);
    return signer.getPublicKey().asHex();
  } catch (err) {
    console.error(err);
    throw new Error('Unable to generate public key from the provided private key');
  }
}

/**
 * Encrypts a private key.
 * @param password - Encryption key.
 * @param privateKey - Unencrypted private key.
 */
export function encrypt(password: string, privateKey: string): string {
  return JSON.stringify(sjcl.encrypt(password, privateKey));
}

/**
 * Decrypts a private key.
 * @param password - Encryption key.
 * @param encryptedPrivateKey - Encrypted private key.
 */
export function decrypt(password: string, encryptedPrivateKey: string): string {
  return sjcl.decrypt(password, JSON.parse(encryptedPrivateKey));
}

/**
 * Fills out, signs, and encodes an incomplete CircuitManagementPayload.
 *
 * @param payload - The incomplete CircuitManagementPayload.
 * @param signer - Wrapper containing the user's keys.
 */
export function signPayload(payload: Uint8Array, privateKey: string): Uint8Array {
  const privKey = Secp256k1PrivateKey.fromHex(privateKey);
  const signer = new Secp256k1Signer(privKey);

  const message = protos.CircuitManagementPayload.decode(payload);
  const header = protos.CircuitManagementPayload.Header.decode(message.header);

  const pubKey = signer.getPublicKey().asBytes();
  header.requester = pubKey;
  message.signature = hexToBytes(signer.sign(header));
  message.header = protos.CircuitManagementPayload.Header.encode(header).finish();
  const signedPayload = protos.CircuitManagementPayload.encode(message).finish();
  return signedPayload;
}

function hexToBytes(hex: string) {
    const bytes = [];
    let i;
    for (i = 0; i < hex.length; i += 2) {
        bytes.push(parseInt(hex.substr(i, 2), 16));
    }
    return bytes;
}
