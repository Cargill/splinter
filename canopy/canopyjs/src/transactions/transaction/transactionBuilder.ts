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
import { Transaction, TransactionHeader } from '../../compiled_protos';
import { Signer } from '../signing';
import { MissingFieldError } from './errors';

export class TransactionBuilder {
  batcherPublicKey: string | null;

  dependencies: string[];

  familyName: string;

  familyVersion: string;

  inputs: string[];

  outputs: string[];

  nonce: string | null;

  payload: Uint8Array;

  constructor() {
    this.batcherPublicKey = null;
    this.dependencies = [];
    this.familyName = '';
    this.familyVersion = '';
    this.inputs = [];
    this.outputs = [];
    this.nonce = null;
    this.payload = new Uint8Array();
  }

  withBatcherPublicKey(batcherPublicKey: string): TransactionBuilder {
    this.batcherPublicKey = batcherPublicKey;
    return this;
  }

  withDependencies(dependencies: string[]): TransactionBuilder {
    this.dependencies = dependencies;
    return this;
  }

  withFamilyName(familyName: string): TransactionBuilder {
    this.familyName = familyName;
    return this;
  }

  withFamilyVersion(self, familyVersion: string): TransactionBuilder {
    this.familyVersion = familyVersion;
    return this;
  }

  withInputs(inputs: string[]): TransactionBuilder {
    this.inputs = inputs;
    return this;
  }

  withOutputs(outputs: string[]): TransactionBuilder {
    this.outputs = outputs;
    return this;
  }

  withNonce(nonce: string): TransactionBuilder {
    this.nonce = nonce;
    return this;
  }

  withPayload(payloadBytes: Uint8Array): TransactionBuilder {
    this.payload = payloadBytes;
    return this;
  }

  buildTransactionHeader(signer: Signer): Uint8Array {
    const batcherPublicKey = this.batcherPublicKey
      ? this.batcherPublicKey
      : signer.getPublicKey();

    if (this.familyName === '') {
      throw new MissingFieldError('Family Name');
    }

    if (this.familyVersion === '') {
      throw new MissingFieldError('Family Version');
    }

    if (!this.inputs.length) {
      throw new MissingFieldError('Inputs');
    }

    if (!this.outputs.length) {
      throw new MissingFieldError('Outputs');
    }

    if (!this.payload.length) {
      throw new MissingFieldError('Payload');
    }

    return TransactionHeader.encode({
      familyName: this.familyName,
      familyVersion: this.familyVersion,
      inputs: this.inputs,
      outputs: this.outputs,
      signerPublicKey: signer.getPublicKey(),
      batcherPublicKey,
      dependencies: this.dependencies,
      nonce: this.nonce,
      payloadSha512: crypto
        .createHash('sha512')
        .update(this.payload)
        .digest('hex')
    }).finish();
  }

  build(signer: Signer): Transaction {
    const header = this.buildTransactionHeader(signer);

    const headerSignature = signer.sign(header);

    return Transaction.create({
      header,
      headerSignature,
      payload: this.payload
    });
  }
}
