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
import React from 'react';
// import { Input } from '../node_modules/canopy-design-system/src/components/form/Input';

const submitUpdateKey = (e) => {
  e.preventDefault();
  console.log(document.querySelector('#change-key').value);
}

export function UpdateKeyForm() {
  return (
    <div className="wrapper">
      <h2>Update key name</h2>
      <form id="change-key-form" aria-label="Change-key-form">
        <div className="canopy-input">
          <input type="text" id="change-key" aria-label="change-key-field" required />
          <label htmlFor="change-key">New key name</label>
        </div>
        {/* <Input type="password" label="change password" id="change-password" required>new password</Input> */}
        <button className="submit" onClick={submitUpdateKey}>Submit</button>
      </form>
    </div>
  );
}
