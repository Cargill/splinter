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
import './App.scss';
import { ActionList } from './ActionList';
import { KeyCard } from './KeyCard';
import { ChangePasswordForm } from './forms/ChangePasswordForm';
import { AddKeyForm } from './forms/AddKeyForm';
import { ChangeUsernameForm } from './forms/ChangeUsernameForm';
import { UpdateKeyForm } from './forms/UpdateKeyForm';
import { PinValidateForm } from './forms/PinValidateForm';
import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { OverlayModal } from './OverlayModal';


function App() {
  let [modalActive, setModalActive] = useState(false);
  let [form, setForm] = useState(null);
  let user = {
    id: "1",
    user_id: "xkcd1234567",
    public_key: "MFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEPdyWXbCBMxH0E9d2bYeJvBZbYNYo2afCtPVRR4jWMrOIAeXagkt2p/D48WoM74te7UwHHTarLBmnxqcxkgLw",
    private_key: "MHQCAQEEICOOwNqqvqPAFiEd8EzbeAN7KwsLFX6Rjn1I2K9sDa98oAcGBSuBBAAKoUQDQgAEPdyWXbCBMxH0E9d2bYeJvBZbYNYo2afCt+PVRR4jWMrOIAeXagkt2pD48WoM74te7UwHHTarLBmnxqcxkgLw",
    display_name: "Bobby Beans"
  }

  // const keys = canopyjs.getKeys();
  const keys = [
    {
      id: '1',
      name: 'key1',
      publicKey: "MHQCAQEEICOOwNqqvqPAFiEd8EzbeAN7KwsLFX6Rjn1I2K9sDa98oAcGBSuBBAAKoUQDQgAEPdyWXbCBMxH0E9d2bYeJvBZbYNYo2afCt+PVRR4jWMrOIAeXagkt2pD48WoM74te7UwHHTarLBmnxqcxkgLw"
    },
    {
      id: '2',
      name: 'key2',
      publicKey: "MHQCAQEEICOOwNqqvqPAFiEd8EzbeAN7KwsLFX6Rjn1I2K9sDa98oAcGBSuaabAKoUQDQgAEPdyWXbCBMxH0E9d2bYeJvBZbYNYo2afCt+PVRR4jWMrOIAeXagkt2pD48WoM74te7UwHHTarLBmnxqcxkgLw"
    },
    {
      id: '3',
      name: 'key3',
      publicKey: "MHQCAQEEICOOwNqqvqPAFiEd8EzbeAN7KwsLFX6Rjn1I2K9sDa98oAcGBSuBBAAKoUQDQgAEPdyWXbCBMxH0E9d2bYeJvBZbYNYo2afCt+PVRR4jWMrOIAeXagkt2pD48WoM74te7UwHHTarLBmnxq123456"
    },
    {
      id: '4',
      name: 'key4',
      publicKey: "MHQCAQEEICOOwNqqvqPAFiEd8EzbeAN7KwsLFX6Rjn1I2K9sDa98oAcGBSuBBAAKoUQDQgAEPdyWXbCBMxH0E9d2bYeJvBZbYNYo2afCt+PVRR4jWMrOIAeXagkt2pD48WoM74te7UwHHTarLBmnxq987654"
    }
  ]

  const activeKey = "MHQCAQEEICOOwNqqvqPAFiEd8EzbeAN7KwsLFX6Rjn1I2K9sDa98oAcGBSuaabAKoUQDQgAEPdyWXbCBMxH0E9d2bYeJvBZbYNYo2afCt+PVRR4jWMrOIAeXagkt2pD48WoM74te7UwHHTarLBmnxqcxkgLw"

  const openModalForm = (form) => {
    setForm(form);
    setModalActive(true);
  }
  return (
    <div id="profile">
      <section className="user-info">
        <div className="display-name info-field">
          <div className="info">
            <h1 className="value">{user.display_name}</h1>
          </div>
        </div>
        <ActionList className="user-actions">
          <button className="flat" onClick={() => openModalForm(ChangeUsernameForm)}>Update username</button>
          <button className="flat" onClick={() => openModalForm(ChangePasswordForm)}>Change password</button>
        </ActionList>
      </section>
      <section className="user-keys">
        <h3>Keys</h3>
        <div className="key-list">
          {keys.map(key =>
            <KeyCard key={key.id} userKey={key} isActive={key.publicKey === activeKey} editFn={() => openModalForm(UpdateKeyForm)} activateFn={() => openModalForm(PinValidateForm)} />
          )}
        </div>
        <button className="fab add-key" onClick={() => openModalForm(AddKeyForm)}>
          <FontAwesomeIcon icon={faPlus} className="icon" />
        </button>
      </section>
      <OverlayModal open={modalActive} closeFn={() => setModalActive(false)}>
        {form}
      </OverlayModal>
    </div>
  );
}

export default App;
