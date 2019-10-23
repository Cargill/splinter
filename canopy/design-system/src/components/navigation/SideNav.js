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
import { NavLink } from 'react-router-dom';
import PropTypes from 'prop-types';

import logo from '../../logo.svg';
import './SideNav.scss';

export default class SideNav extends React.Component {
  render() {
    const { tabs } = this.props;
    return (
      <div className="side-nav">
        <div id="brand">
          <img src={logo} className="app-logo" alt="logo" />
          <span>
            Canopy
            <sup>design</sup>
          </span>
        </div>
        <nav id="tabs" className="marginTop-m">
          <ul>
            {tabs.map(tab => (
              <li className="tab paddingTop-s paddingBottom-s" key={tab.name}>
                <NavLink to={tab.route}>{tab.name}</NavLink>
              </li>
            ))}
          </ul>
        </nav>
      </div>
    );
  }
}

SideNav.propTypes = {
  tabs: PropTypes.arrayOf(PropTypes.object)
};

SideNav.defaultProps = {
  tabs: []
};
