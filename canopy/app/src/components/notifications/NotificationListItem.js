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

import { useDispatch } from 'react-redux';
import { readNotification } from 'actions';

import 'components/notifications/NotificationListItem.scss';

function NotificationListItem({ notification }) {
  const dispatch = useDispatch();

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => dispatch(readNotification(notification.id))}
      onKeyPress={e => {
        if (e.key === 'Enter') {
          dispatch(readNotification(notification.id));
        }
      }}
      className={`notification-list-item ${
        notification.isRead ? 'is-read' : ''
      }`}
    >
      {notification.text}
    </div>
  );
}

NotificationListItem.propTypes = {
  notification: PropTypes.shape({
    id: PropTypes.number.isRequired,
    text: PropTypes.string.isRequired,
    link: PropTypes.string,
    isRead: PropTypes.bool.isRequired
  }).isRequired
};

export default NotificationListItem;
