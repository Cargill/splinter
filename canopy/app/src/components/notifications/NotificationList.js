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
import PropTypes from 'prop-types';
import { useSelector } from 'react-redux';

import NotificationListItem from 'components/notifications/NotificationListItem';

function NotificationList({ isOpen }) {
  const notifications = useSelector(state => state.notifications);

  return (
    <div
      className={`notification-list flexDirection-column ${
        isOpen ? 'display-flex' : 'display-none'
      }`}
    >
      <h2>Notifications</h2>
      {notifications.map(notification => (
        <NotificationListItem
          key={notification.id}
          notification={notification}
        />
      ))}
    </div>
  );
}

NotificationList.propTypes = {
  isOpen: PropTypes.bool.isRequired
};

export default NotificationList;
