// Copyright 2018-2020 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A memory-backed implementation of the [OAuthUserSessionStore]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::error::{
    ConstraintViolationError, ConstraintViolationType, InternalError, InvalidArgumentError,
    InvalidStateError,
};

use super::{
    InsertableOAuthUserSession, OAuthUser, OAuthUserSession, OAuthUserSessionStore,
    OAuthUserSessionStoreError,
};

/// A memory-backed implementation of the [OAuthUserSessionStore]
#[derive(Default, Clone)]
pub struct MemoryOAuthUserSessionStore {
    internal: Arc<Mutex<Internal>>,
}

impl MemoryOAuthUserSessionStore {
    /// Creates a new memory-backed OAuth user session store
    pub fn new() -> Self {
        Self::default()
    }
}

impl OAuthUserSessionStore for MemoryOAuthUserSessionStore {
    fn add_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let mut internal = self.internal.lock().map_err(|_| {
            OAuthUserSessionStoreError::Internal(InternalError::with_message(
                "Cannot access OAuth user session store: mutex lock poisoned".to_string(),
            ))
        })?;

        if internal
            .sessions
            .contains_key(session.splinter_access_token())
        {
            return Err(OAuthUserSessionStoreError::ConstraintViolation(
                ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
            ));
        }

        if !internal.users.contains_key(session.subject()) {
            internal.users.insert(
                session.subject().to_string(),
                OAuthUser::new(session.subject().to_string()),
            );
        }

        internal
            .sessions
            .insert(session.splinter_access_token().into(), session.into());

        Ok(())
    }

    fn update_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let mut internal = self.internal.lock().map_err(|_| {
            OAuthUserSessionStoreError::Internal(InternalError::with_message(
                "Cannot access OAuth user session store: mutex lock poisoned".to_string(),
            ))
        })?;

        match internal.sessions.get(session.splinter_access_token()) {
            Some(existing_session) => {
                if session.subject() != existing_session.subject {
                    Err(OAuthUserSessionStoreError::InvalidArgument(
                        InvalidArgumentError::new(
                            "session".to_string(),
                            "Cannot update the 'subject' field for an OAuth user session".into(),
                        ),
                    ))
                } else {
                    internal
                        .sessions
                        .insert(session.splinter_access_token().into(), session.into());
                    Ok(())
                }
            }
            None => Err(OAuthUserSessionStoreError::InvalidState(
                InvalidStateError::with_message(
                    "An OAuth user session for the given Splinter access token does not exist"
                        .to_string(),
                ),
            )),
        }
    }

    fn remove_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<(), OAuthUserSessionStoreError> {
        self.internal
            .lock()
            .map_err(|_| {
                OAuthUserSessionStoreError::Internal(InternalError::with_message(
                    "Cannot access OAuth user session store: mutex lock poisoned".to_string(),
                ))
            })?
            .sessions
            .remove(splinter_access_token)
            .map(|_| ())
            .ok_or_else(|| {
                OAuthUserSessionStoreError::InvalidState(InvalidStateError::with_message(
                    "An OAuth user session for the given Splinter access token does not exist"
                        .to_string(),
                ))
            })
    }

    fn get_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<Option<OAuthUserSession>, OAuthUserSessionStoreError> {
        let internal = self.internal.lock().map_err(|_| {
            OAuthUserSessionStoreError::Internal(InternalError::with_message(
                "Cannot access OAuth user session store: mutex lock poisoned".to_string(),
            ))
        })?;

        internal
            .sessions
            .get(splinter_access_token)
            .cloned()
            .map(|session| {
                let InternalOAuthUserSession {
                    splinter_access_token,
                    subject,
                    oauth_access_token,
                    oauth_refresh_token,
                    last_authenticated,
                } = session;

                let user = internal.users.get(&subject).cloned().ok_or_else(|| {
                    OAuthUserSessionStoreError::Internal(InternalError::with_message(
                        "Unknown session subject".to_string(),
                    ))
                })?;

                Ok(OAuthUserSession {
                    splinter_access_token,
                    user,
                    oauth_access_token,
                    oauth_refresh_token,
                    last_authenticated,
                })
            })
            .transpose()
    }

    fn get_user(&self, subject: &str) -> Result<Option<OAuthUser>, OAuthUserSessionStoreError> {
        Ok(self
            .internal
            .lock()
            .map_err(|_| {
                OAuthUserSessionStoreError::Internal(InternalError::with_message(
                    "Cannot access OAuth user session store: mutex lock poisoned".to_string(),
                ))
            })?
            .users
            .get(subject)
            .cloned())
    }

    fn clone_box(&self) -> Box<dyn OAuthUserSessionStore> {
        Box::new(self.clone())
    }
}

#[derive(Default)]
struct Internal {
    /// Map of subject identifier -> user
    pub users: HashMap<String, OAuthUser>,
    /// Map of splinter access token -> session
    pub sessions: HashMap<String, InternalOAuthUserSession>,
}

#[derive(Clone)]
struct InternalOAuthUserSession {
    pub splinter_access_token: String,
    pub subject: String,
    pub oauth_access_token: String,
    pub oauth_refresh_token: Option<String>,
    pub last_authenticated: SystemTime,
}

impl From<InsertableOAuthUserSession> for InternalOAuthUserSession {
    fn from(session: InsertableOAuthUserSession) -> Self {
        let InsertableOAuthUserSession {
            splinter_access_token,
            subject,
            oauth_access_token,
            oauth_refresh_token,
        } = session;
        Self {
            splinter_access_token,
            subject,
            oauth_access_token,
            oauth_refresh_token,
            last_authenticated: SystemTime::now(),
        }
    }
}
