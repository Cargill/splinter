// Copyright 2018-2022 Cargill Incorporated
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

/// An authorization handler's decision about whether to allow, deny, or pass on the request
pub enum AuthorizationHandlerResult {
    /// The authorization handler has granted the requested permission
    Allow,
    /// The authorization handler has denied the requested permission
    Deny,
    /// The authorization handler is not able to determine if the requested permission should be
    /// granted or denied
    Continue,
}
