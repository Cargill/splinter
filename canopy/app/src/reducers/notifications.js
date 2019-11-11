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

import { READ_NOTIFICATION } from 'actions';

const mockNotifications = [
  {
    id: 1,
    text: 'You have a new friend request!',
    isRead: false
  },
  {
    id: 2,
    text: 'You just lost the game.',
    isRead: false
  }
];

export default (state = mockNotifications, action) => {
  switch (action.type) {
    case READ_NOTIFICATION:
      return state.map(notification => {
        if (action.id === notification.id) {
          return { ...notification, isRead: true };
        }
        return notification;
      });
    default:
      return state;
  }
};
