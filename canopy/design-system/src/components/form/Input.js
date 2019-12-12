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

import React, { useState } from 'react';
import PropTypes from 'prop-types';
import { faEye, faEyeSlash } from '@fortawesome/free-regular-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

export function Input({ id, required, type, children }) {
  const [visible, setVisible] = useState(!(type === 'password'));
  let inputType = type === 'password' && visible ? 'text' : type;
  return (
    <div className="canopy-input">
      <input
        type={inputType}
        id={id}
        placeholder=" "
        aria-label={`${id}-field`}
        required={required}
      />
      <label htmlFor={id}>{`${children}${required ? ' *': ''}`}</label>
      {type === 'password' && (
        <FontAwesomeIcon
          icon={visible ? faEyeSlash : faEye}
          onClick={() => setVisible(!visible)}
          className="toggle-password"
        />
      )}
    </div>
  );
}

Input.defaultProps = {
  required: false,
  type: 'text',
  children: ''
};

Input.propTypes = {
  required: PropTypes.bool,
  type: PropTypes.oneOf(['text', 'password', 'number']),
  id: PropTypes.string.isRequired,
  children: PropTypes.string
};
