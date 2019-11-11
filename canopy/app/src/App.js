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

import React, { useRef, useLayoutEffect, useState } from 'react';
import { library } from '@fortawesome/fontawesome-svg-core';
import { faBell } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

import NotificationList from 'components/notifications/NotificationList';
import { loadAllSaplings } from './loadSaplings';

import 'App.scss';

const canopyState = {
  configSaplings: {},
  bootstrapApp: null
};

function invokeConfigSaplings() {
  const configSaplings = Object.values(canopyState.configSaplings);
  if (configSaplings.length === 0) {
    throw new Error('No Config Saplings registered');
  }
  configSaplings.forEach(bootstrapConfigSapling => {
    bootstrapConfigSapling();
  });
}

function invokeRegisteredApp(domNode) {
  if (canopyState.registeredApp === null) {
    throw new Error('No Sapling registered');
  }
  canopyState.bootstrapApp(domNode);
}

window.$CANOPY = {
  registerApp: bootStrapFunction => {
    // exposed via CanopyJS
    canopyState.bootstrapApp = bootStrapFunction;
  },
  registerConfigSapling: (namespace, bootStrapFunction) => {
    // exposed via CanopyJS
    canopyState.configSaplings[namespace] = bootStrapFunction;
  }
};

library.add(faBell);

function App() {
  const saplingNode = useRef(null);
  const [userSaplings, setUserSaplings] = useState([]);
  const [notificationListIsOpen, setNotificationListIsOpen] = useState(false);

  useLayoutEffect(() => {
    (async function invokeSaplings() {
      const {
        saplingIsLoaded,
        configSaplingsAreLoaded,
        userSaplingsResponse
      } = await loadAllSaplings();

      setUserSaplings(userSaplingsResponse);

      if (configSaplingsAreLoaded) {
        invokeConfigSaplings();
      }

      if (saplingIsLoaded) {
        invokeRegisteredApp(saplingNode.current);
      }
    })();
  }, []);

  return (
    <div className="app">
      <nav className="side-nav">
        <button
          type="button"
          onClick={() => setNotificationListIsOpen(!notificationListIsOpen)}
        >
          <FontAwesomeIcon icon="bell" />
        </button>
        {userSaplings.map(({ displayName, namespace }) => {
          return (
            <a href={`/${namespace}`} key={namespace}>
              {displayName}
            </a>
          );
        })}
      </nav>
      <NotificationList
        className="notification-list"
        isOpen={notificationListIsOpen}
      />
      <div className="view" ref={saplingNode} />
    </div>
  );
}

export default App;
