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
import classnames from 'classnames';
import PropTypes from 'prop-types';

import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {faPencilAlt} from '@fortawesome/free-solid-svg-icons';

import './KeyCard.scss';

export function KeyCard( { userKey, isActive, editFn, activateFn } ) {
  const setActive = (e) => {
    e.preventDefault();
    console.log(`setting ${userKey.name} as active key`);
  }
  return (
    <div className={classnames('key-card', isActive && 'active')}>
        <div className="header">
          <span className="name">{userKey.name}</span>
          <FontAwesomeIcon icon={faPencilAlt} className="edit" onClick={editFn} />
        </div>
        <div className="keys">
          <label htmlFor={`public-key-${userKey.id}`}>Public key:</label>
          <span className="key" id={`public-key-${userKey.id}`}>{userKey.publicKey}</span>
        </div>
        {!isActive &&
          <button className="set-active" onClick={activateFn}>Set active</button>
        }
        {isActive &&
          <div className="active">Active</div>
        }
    </div>
  )
}

KeyCard.defaultProps = {
  isActive: false,
}

KeyCard.propTypes = {
  userKey: PropTypes.object.isRequired,
  isActive: PropTypes.bool,
  editFn: PropTypes.func.isRequired,
  activateFn: PropTypes.func.isRequired
}
