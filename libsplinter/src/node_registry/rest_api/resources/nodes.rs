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

use std::collections::HashMap;

use crate::node_registry::Node;
use crate::rest_api::paging::Paging;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListNodesResponse<'a> {
    pub data: Vec<NodeResponse<'a>>,
    pub paging: Paging,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NodeResponse<'a> {
    pub identity: &'a str,
    pub endpoints: &'a [String],
    pub display_name: &'a str,
    pub keys: &'a [String],
    pub metadata: &'a HashMap<String, String>,
}

impl<'a> From<&'a Node> for NodeResponse<'a> {
    fn from(node: &'a Node) -> Self {
        Self {
            identity: &node.identity,
            endpoints: &node.endpoints,
            display_name: &node.display_name,
            keys: &node.keys,
            metadata: &node.metadata,
        }
    }
}
