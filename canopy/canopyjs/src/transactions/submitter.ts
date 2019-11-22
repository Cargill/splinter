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
interface BatchStatus {
  statusType: string;
  message: BatchMessage[];
}

interface BatchMessage {
  transaction_id: string;
  error_message: string;
  error_data: number[];
}

interface BatchInfo {
  id: string;
  status: BatchStatus;
}

async function http(
  method: string,
  url: string,
  data: Uint8Array,
  headerFn: (request: XMLHttpRequest) => void
): Promise<string> {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest();
    request.open(method, url);
    if (headerFn) {
      headerFn(request);
    }
    request.onload = () => {
      if (request.status >= 200 && request.status < 300) {
        resolve(request.response);
      } else {
        console.error(request);
        if (request.status >= 400 && request.status < 500) {
          reject('Failed to send request. Contact the administrator for help.');
        } else {
          reject(
            'The server has encountered an error. Please contact the administrator.'
          );
        }
      }
    };
    request.onerror = () => {
      console.error(request);
      reject(
        'The server has encountered an error. Please contact the administrator.'
      );
    };
    request.send(data);
  });
}

export async function submitBatch(
  url: string,
  batch: Uint8Array
): Promise<BatchInfo[]> {
  return await http('POST', url, batch, (request: XMLHttpRequest) => {
    request.setRequestHeader('Content-Type', 'application/octet-stream');
  })
    .catch(err => {
      throw new Error(err);
    })
    .then(body => {
      return JSON.parse(body).data as BatchInfo[];
    });
}
