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

import {
  Transaction,
  Batch,
  BatchHeader,
  BatchList
} from '../../compiled_protos';
import { Signer } from '../signing';
import { MissingFieldError } from './errors';

export class BatchBuilder {
  transactions: Transaction[];

  trace: boolean;

  constructor() {
    this.transactions = [];
    this.trace = false;
  }

  withTransactions(transactions: Transaction[]): BatchBuilder {
    this.transactions = [...this.transactions, ...transactions];
    return this;
  }

  withTrace(trace: boolean): BatchBuilder {
    this.trace = trace;
    return this;
  }

  buildHeader(signer: Signer): Uint8Array {
    if (!this.transactions.length) {
      throw new MissingFieldError('transactions');
    }

    const transactionIds = this.transactions.map(
      (txn: Transaction) => txn.headerSignature
    );

    return BatchHeader.encode({
      signerPublicKey: signer.getPublicKey(),
      transactionIds
    }).finish();
  }

  build(signer: Signer): Uint8Array {
    const header = this.buildHeader(signer);

    const headerSignature = signer.sign(header);

    const batch = Batch.create({
      header,
      headerSignature,
      transactions: this.transactions,
      trace: this.trace
    });

    return BatchList.encode({
      batches: [batch]
    }).finish();
  }
}
