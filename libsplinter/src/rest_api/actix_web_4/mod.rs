// Copyright 2018-2022 Cargill Incorporated
//

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

pub mod api;
pub mod protocol_version;

use actix_web_4::Resource;

pub trait ResourceProvider: Send {
    fn resources(&self) -> Vec<Resource>;
}
