/**
 * Copyright 2018-2020 Cargill Incorporated
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

import React, { useState, createContext, useContext, useEffect } from 'react';
import { get } from './request';

import {
  mountCurrentSapling,
  mountSaplingStyles,
  mountConfigSaplings
} from './loadSaplings';

export const CanopyContext = createContext({});

const fetchUserSaplings = async saplingURL => {
  const response = await get(`${saplingURL}/userSaplings`);
  const userSaplingsResponse = response.json;
  return userSaplingsResponse;
};

const fetchConfigSaplings = async saplingURL => {
  const response = await get(`${saplingURL}/configSaplings`);
  const configSaplingsResponse = response.json;
  return configSaplingsResponse;
};

export function CanopyProvider({ saplingURL, splinterURL, children }) {
  const [userSaplings, setUserSaplings] = useState([]);
  const [configSaplings, setConfigSaplings] = useState({});

  const sessionUser = window.sessionStorage.getItem('CANOPY_USER');
  const [user, setUser] = useState(
    sessionUser ? JSON.parse(sessionUser) : null
  );

  window.$CANOPY.getSharedConfig = () => {
    return {
      canopyConfig: {
        splinterURL,
        saplingURL
      }
    };
  };
  window.$CANOPY.setUser = canopyUser => {
    window.sessionStorage.setItem('CANOPY_USER', JSON.stringify(canopyUser));
    setUser(canopyUser);
  };

  window.$CANOPY.getUser = () => user;

  useEffect(() => {
    window.$CANOPY = window.$CANOPY || {};
    window.$CANOPY.registerApp = bootStrapFunction => {
      bootStrapFunction(document.querySelector('#sapling-container'));
    };

    fetchConfigSaplings(saplingURL).then(saplings => {
      mountConfigSaplings(saplings);
      mountSaplingStyles(saplings);
    });
  }, [saplingURL]);

  useEffect(() => {
    window.$CANOPY.registerConfigSapling = (namespace, bootStrapFunction) => {
      bootStrapFunction();
      return setConfigSaplings(currentConfigSaplings => {
        return { ...currentConfigSaplings, [namespace]: bootStrapFunction };
      });
    };
  }, []);

  useEffect(() => {
    fetchUserSaplings(saplingURL).then(saplings => {
      mountSaplingStyles(saplings);
      mountCurrentSapling(saplings);
      setUserSaplings(saplings);
    });
  }, [saplingURL]);

  return (
    <CanopyContext.Provider value={{ configSaplings, userSaplings, user }}>
      {children}
    </CanopyContext.Provider>
  );
}

export function useUserSaplings() {
  const context = useContext(CanopyContext);
  return context.userSaplings;
}

export function useConfigSaplings() {
  const context = useContext(CanopyContext);
  return context.userSaplings;
}

export function useUser() {
  const context = React.useContext(CanopyContext);
  return context.user;
}
