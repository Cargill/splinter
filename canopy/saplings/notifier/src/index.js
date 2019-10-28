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

import { notify } from 'canopyjs';

const feedBackElement = document.querySelector('#feedback');

document.querySelector('#notify-form').addEventListener('submit', event => {
  event.preventDefault();
  const { target } = event;
  const formData = new FormData(target);
  const notificationBody = formData.get('notification');
  notify(notificationBody)
    .then(() => {
      feedBackElement.innerHTML = `<span style="color:green">Posted your notification</span>`;
    })
    .catch(() => {
      feedBackElement.innerHTML = `<span style="color:red">You're probably not running in a Canopy</span>`;
    })
    .finally(() => {
      target.reset();
    });
});
