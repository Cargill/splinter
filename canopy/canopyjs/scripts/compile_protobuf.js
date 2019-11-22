// Copyright 2019 Cargill Incorporated
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

'use strict';

const fs = require('fs');
const pbjs = require('protobufjs/cli/pbjs');
const pbts = require('protobufjs/cli/pbts');
const process = require('process');

const path = require('path');

const protoDir = process.argv[2];

const files = fs.readdirSync(protoDir).map(f => path.resolve(protoDir, f));

pbjs.main(
  [
    '-t',
    'static-module',
    '-w',
    'commonjs',
    '-o',
    'src/compiled_protos.js',
    ...files
  ],
  function handleJsCompileError(err) {
    if (err) {
      console.error(`Error compiling protobuf bundle: ${err}`);
    } else {
      console.log('Sucessfully created protobuf bundle');
    }
  }
);

pbts.main(
  ['-o', 'src/compiled_protos.d.ts', 'src/compiled_protos.js'],
  function handleJsCompileError(err) {
    if (err) {
      console.error(`Error compiling protobuf typings: ${err}`);
    } else {
      console.log('Sucessfully created protobuf typings');
    }
  }
);
