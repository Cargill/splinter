/**
 * Copyright 2019 Cargill Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
import crypto from 'crypto';
import secp256k1 from 'secp256k1';

function toHex(buffer: Uint8Array): string {
  return Array.from(buffer)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

interface IContext {
  sign: (message: Uint8Array, privateKey: PrivateKey) => string;
  getPublicKey: (privateKey: PrivateKey) => PublicKey;
}

interface IPrivateKey {
  asHex: () => string;
  asBytes: () => Buffer;
}

interface IPublicKey {
  publicKey: Uint8Array;
  asHex: () => string;
  asBytes: () => Buffer;
}

class Context implements IContext {
  sign(message: Uint8Array, privateKey: PrivateKey): string {
    const hash = crypto
      .createHash('sha512')
      .update(message)
      .digest();

    const result = secp256k1.sign(hash, Buffer.from(privateKey.asBytes()));
    return toHex(result.signature);
  }

  getPublicKey(privateKey: PrivateKey): PublicKey {
    return new PublicKey(secp256k1.publicKeyCreate(privateKey.asBytes()));
  }
}

class PrivateKey implements IPrivateKey {
  privateKey: Uint8Array;

  constructor(privateKey: Uint8Array) {
    this.privateKey = privateKey;
  }

  asHex() {
    return toHex(this.privateKey);
  }

  asBytes() {
    return Buffer.from(this.privateKey);
  }

  static fromHex(privateKey: string) {
    let buffer = Buffer.from(privateKey, 'hex');
    return new PrivateKey(Uint8Array.from(buffer));
  }
}

class PublicKey implements IPublicKey {
  publicKey: Uint8Array;

  constructor(publicKey: Uint8Array) {
    this.publicKey = publicKey;
  }

  asHex() {
    return toHex(this.publicKey);
  }

  asBytes() {
    return Buffer.from(this.publicKey);
  }
}

class Secp256k1PrivateKey extends PrivateKey {
  constructor(privateKey: Uint8Array) {
    super(privateKey);
  }

  asHex() {
    return toHex(this.privateKey);
  }

  asBytes() {
    return Buffer.from(this.privateKey);
  }

  static fromHex(privateKey: string) {
    let buffer = Buffer.from(privateKey, 'hex');
    return new Secp256k1PrivateKey(Uint8Array.from(buffer));
  }
}

class Secp256k1PublicKey extends PublicKey {
  constructor(publicKey: Uint8Array) {
    super(publicKey);
  }
}

class Secp256k1Context implements Context {
  sign(message: Uint8Array, privateKey: Secp256k1PrivateKey): string {
    const hash = crypto
      .createHash('sha256')
      .update(message)
      .digest();

    const result = secp256k1.sign(hash, Buffer.from(privateKey.asBytes()));
    return toHex(result.signature);
  }

  getPublicKey(privateKey: Secp256k1PrivateKey): Secp256k1PublicKey {
    return new Secp256k1PublicKey(
      secp256k1.publicKeyCreate(privateKey.asBytes())
    );
  }
}

export interface ISigner {
  context: Secp256k1Context;
  privateKey: Secp256k1PrivateKey;
  publicKey: Secp256k1PublicKey;
}

export class Signer implements ISigner {
  context: Secp256k1Context;

  privateKey: Secp256k1PrivateKey;

  publicKey: Secp256k1PublicKey;

  sign(message: Uint8Array): string {
    return this.context.sign(message, this.privateKey);
  }

  getPublicKey(): string {
    return this.publicKey.asHex();
  }

  constructor(privateKey: string) {
    this.privateKey = Secp256k1PrivateKey.fromHex(privateKey);
    this.context = new Secp256k1Context();
    this.publicKey = this.context.getPublicKey(this.privateKey);
  }
}
