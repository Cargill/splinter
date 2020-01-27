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

import {
  registerConfigSapling,
  getUser,
  registerApp,
  setUser,
  getSharedConfig
} from 'canopyjs';
import { createBrowserHistory } from 'history';
import axios from 'axios';
import { sha256 } from 'js-sha256';
import { html } from './html';

const history = createBrowserHistory();

interface FormEventHandler {
  (this: HTMLFormElement, event: Event): void;
}

registerConfigSapling('login', () => {
  const canopyUser = window.sessionStorage.getItem('canopy_user');

  if (!getUser() && canopyUser) {
    setUser(JSON.parse(canopyUser));
    return;
  }

  if (!getUser()) {
    const canopyUrl = new URL(window.location.href);
    canopyUrl.pathname =
      canopyUrl.pathname === '/login' ? '/' : canopyUrl.pathname;

    history.push('/login');
    registerApp(domNode => {
      const div = domNode as HTMLDivElement;
      div.innerHTML = html;

      const tabs = Array.from(div.querySelectorAll('.tab-box-option'));
      const forms = Array.from(div.querySelectorAll('form'));

      const errorMessageNode = div.querySelector(
        '#login-register-error-message'
      );

      const [loginForm, registerForm] = forms;
      const panels = Array.from(div.querySelectorAll('.tab-box-content'));

      function formSumbitEventToFormData(event: Event): FormData {
        event.preventDefault();
        return new FormData(event.target as HTMLFormElement);
      }

      function showErrorResponse(message): void {
        errorMessageNode.innerHTML = message;
      }

      function createFormActionCapture(
        action: 'register' | 'login'
      ): FormEventHandler {
        return async function captureForm(
          this: HTMLFormElement,
          event: Event
        ): Promise<void> {
          const formData = formSumbitEventToFormData(event);

          const user = {
            displayName: formData.get('username') as string
          };

          const hash = sha256.hmac.create(formData.get('username') as string);
          hash.update(formData.get('password') as string);
          const hashedPassword = hash.hex();

          const http = axios.create({
            baseURL: getSharedConfig().canopyConfig.splinterEndpoint
          });

          const target = event.target as HTMLFormElement;
          const formParent = target.parentNode as HTMLDivElement;

          const progressNode = document.createElement('div');
          progressNode.setAttribute(
            'style',
            'width: 100%; height: 100%; display: flex; justify-content: center; align-items: center;'
          );

          formParent.replaceChild(progressNode, target);

          let animationRequest = null;

          function doProgress(ts = 0): void {
            const step = (Math.sin(ts / 500) + 1) / 2;
            progressNode.innerHTML = `<progress class='progress' max="${1000}" value="${step *
              1000}"/>`;
            animationRequest = window.requestAnimationFrame(doProgress);
          }
          doProgress();

          tabs.forEach(tab => {
            tab.setAttribute('disabled', 'true');
          });

          try {
            const jsonPayload = {
              username: formData.get('username') as string,
              // eslint-disable-next-line @typescript-eslint/camelcase
              hashed_password: hashedPassword
            };

            if (action === 'register') {
              await http.post(`/biome/register`, jsonPayload);
            }

            const response = await http.post(`/biome/login`, jsonPayload);
            delete response.data.message;
            window.sessionStorage.setItem(
              'canopy_user',
              JSON.stringify({ ...user, ...response.data })
            );
            window.location.href = canopyUrl.href;
          } catch (err) {
            switch (err.response.status) {
              case 404:
                showErrorResponse('Splinter Node could not be found');
                break;
              default:
                showErrorResponse(
                  'Unknown error communicating with Splinter Node'
                );
            }
            tabs.forEach(tab => {
              tab.removeAttribute('disabled');
            });
            window.cancelAnimationFrame(animationRequest);
            formParent.replaceChild(target, progressNode);
          }
        };
      }

      const handleRegisterEvent = createFormActionCapture('register');
      const handleLoginEvent = createFormActionCapture('login');

      registerForm.addEventListener('submit', handleRegisterEvent);

      loginForm.addEventListener('submit', handleLoginEvent);

      forms.forEach(form => {
        form.addEventListener('submit', e => {
          e.preventDefault();
        });
      });

      function setSelectedTab(tabIndex): void {
        tabs.forEach((tab, i) => {
          const selected = i === tabIndex;
          tab.setAttribute('tabindex', selected ? '0' : '-1');
          tab.setAttribute('aria-selected', selected ? 'true' : 'false');
          tab.setAttribute(
            'class',
            selected ? 'tab-box-option active' : 'tab-box-option'
          );
        });

        panels.forEach((panel, i) => {
          if (i === tabIndex) {
            panel.removeAttribute('hidden');
          } else {
            panel.setAttribute('hidden', 'true');
          }
        });
      }

      tabs.forEach((tab, index) => {
        tab.addEventListener('click', () => {
          setSelectedTab(index);
        });
      });
    });
  }
});
