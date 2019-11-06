// Copyright 2019 Cargill Incorporated
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

import axios from 'axios';
import {
  GameroomNotification,
  GameroomProposal,
  UserRegistration,
  UserCredentials,
  UserAuthResponse,
  NewGameroomProposal,
  Member,
  Node,
  Gameroom,
  Ballot,
  Game,
  Player,
  BatchInfo,
} from './models';

import { hashGameName } from '@/utils/xo-games';

export const gameroomAPI = axios.create({
  baseURL: '/api',
});

gameroomAPI.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response) {
      switch (error.response.status) {
        case 401:
          throw new Error('Incorrect username or password.');
        case 500:
          throw new Error(
            'The Gameroom server has encountered an error. Please contact the administrator.',
          );
        case 503:
          throw new Error('The Gameroom server is unavailable. Please contact the administrator.');
        default:
          throw new Error(error.response.data.message);
      }
    }
  },
);

async function http(
  method: string,
  url: string,
  data: Uint8Array,
  headerFn: (request: XMLHttpRequest) => void,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest();
    request.open(method, `/api${url}`);
    if (headerFn) {
      headerFn(request);
    }
    request.onload = () => {
      if (request.status >= 200 && request.status < 300) {
        resolve(request.response);
      } else {
        console.error(request);
        if (request.status >= 400 && request.status < 500) {
          reject('Failed to send request. Contact the administrator for help.');
        } else {
          reject('The Gameroom server has encountered an error. Please contact the administrator.');
        }
      }
    };
    request.onerror = () => {
      console.error(request);
      reject('The Gameroom server has encountered an error. Please contact the administrator.');
    };
    request.send(data);
  });
}

// Users
export async function userCreate(
  user: UserRegistration,
): Promise<UserAuthResponse> {
  const response = await gameroomAPI.post('/users', user);
  return response.data.data as UserAuthResponse;
}

export async function userAuthenticate(
  userCredentials: UserCredentials,
): Promise<UserAuthResponse> {
    const response = await gameroomAPI.post('/users/authenticate', userCredentials);
    return response.data.data as UserAuthResponse;
}

// Gamerooms
export async function gameroomPropose(
  gameroomProposal: NewGameroomProposal,
): Promise<Uint8Array> {
  const response = await gameroomAPI.post('/gamerooms/propose', gameroomProposal);
  return response.data.data.payload_bytes as Uint8Array;
}

export async function listGamerooms(): Promise<Gameroom[]> {
  const response = await gameroomAPI.get('/gamerooms?limit=1000');
  const gamerooms = response.data.data.map((gameroom: any) => {
    const members = gameroom.members.map(async (member: any) => {
      const node = await getNode(member.node_id);
      member.organization = node.metadata.organization;
      return member as Member;
    });
    Promise.all(members).then((m) => gameroom.members = m);
    return gameroom as Gameroom;
  });
  return Promise.all(gamerooms);
}

export async function fetchGameroom(circuitID: string): Promise<Gameroom> {
  const response = await gameroomAPI.get(`/gamerooms/${circuitID}`);
  const gameroom = response.data;
  const members = gameroom.members.map(async (member: any) => {
    const node = await getNode(member.node_id);
    member.organization = node.metadata.organization;
    return member as Member;
  });
  Promise.all(members).then((m) => gameroom.members = m);
  return gameroom as Gameroom;
}

// Nodes
export async function listNodes(): Promise<Node[]> {
  const response = await gameroomAPI.get('/nodes?limit=1000');
  return response.data.data as Node[];
}


// Game information
export async function fetchPlayerInformation(publicKey: string): Promise<Player> {
  const response = await gameroomAPI.get(`/keys/${publicKey}`);
  const player: Player = {
    name: response.data.data.metadata['gameroom/first-name'],
    publicKey: response.data.data.public_key,
    organization: response.data.data.metadata['gameroom/organization'],
  };
  return player;
}

export async function listGames(circuitID: string): Promise<Game[]> {
  const response = await gameroomAPI.get(`/xo/${circuitID}/games`);
  const games = response.data.data.map(async (game: any) => {
    if (game.player_1 !== '') {
      const player1 = await fetchPlayerInformation(game.player_1);
      Promise.all([player1]).then((p1) => game.player_1 = player1);
    }
    if (game.player_2 !== '') {
      const player2 = await fetchPlayerInformation(game.player_2);
      Promise.all([player2]).then((p2) => game.player_2 = player2);
    }
    game.committed = true;
    game.game_name_hash = hashGameName(game.game_name);

    return game as Game;
  });
  return Promise.all(games);
}

// Payloads
export async function submitPayload(payload: Uint8Array): Promise<void> {
  await http('POST', '/submit', payload, (request: XMLHttpRequest) => {
    request.setRequestHeader('Content-Type', 'application/octet-stream');
  }).catch((err) => {
    throw new Error(err);
  });
}

export async function submitBatch(payload: Uint8Array, circuitID: string): Promise<BatchInfo[]> {
  return await http('POST', `/gamerooms/${circuitID}/batches`, payload, (request: XMLHttpRequest) => {
    request.setRequestHeader('Content-Type', 'application/octet-stream');
  }).catch((err) => {
    throw new Error(err);
  }).then((rawBody) => {
    const jsonBody = JSON.parse(rawBody);
    const batchesInfo = jsonBody.data as BatchInfo[];
    return batchesInfo;
  });
}

// Proposals
export async function listProposals(): Promise<GameroomProposal[]> {
  const response = await gameroomAPI.get('/proposals?limit=1000');

  const getMembers = async (member: any) => {
    const node = await getNode(member.node_id);
    member.organization = node.metadata.organization;
    return member as Member;
  };

  const combineProposal = async (proposal: any) => {
    const gameroom = await fetchGameroom(proposal.circuit_id);
    proposal.status = gameroom.status;

    const requester = await getNode(proposal.requester_node_id);
    proposal.requester_org = requester.metadata.organization;

    const members = await Promise.all(
      proposal.members.map((member: any) => getMembers(member)));
    proposal.members = members;
    return proposal;
  };

  return await Promise.all(
    response.data.data.map((proposal: GameroomProposal) => combineProposal(proposal)));
}

async function getNode(id: string): Promise<Node> {
    const response = await gameroomAPI.get(`/nodes/${id}`);
    return response.data.data as Node;
}

export async function proposalVote(ballot: Ballot, proposalID: string,
): Promise<Uint8Array> {
  const response = await gameroomAPI.post(`/proposals/${proposalID}/vote`, ballot);
  return response.data.data.payload_bytes as Uint8Array;
}

// Notifications
const getOrgName = async (notif: any) => {
  const node = await getNode(notif.node_id);
  notif.requester_org = node.metadata.organization;
  return notif as GameroomNotification;
};

export async function listNotifications(publicKey: string): Promise<GameroomNotification[]> {
  const isDisplayed = (value: GameroomNotification): boolean => {
    const displayedNotifs = ['gameroom_proposal', 'circuit_active'];
    if (displayedNotifs.includes(value.notification_type) || value.notification_type.match('^new_game_created')) {
      if (value.notification_type === 'gameroom_proposal'
          && value.requester === publicKey) {
        return false;
      }
      return true;
    } else { return false; }
  };

  const response = await gameroomAPI.get('/notifications?limit=1000');
  const notifications = response.data.data as GameroomNotification[];
  const filtered = notifications.filter(isDisplayed);
  return await Promise.all(filtered.map((notif: any) => getOrgName(notif)));
}

export async function markRead(id: string): Promise<GameroomNotification> {
  const response = await gameroomAPI.patch(`/notifications/${id}/read`);
  const notif = response.data.data;
  const node = await getNode(notif.node_id);
  notif.requester_org = node.metadata.organization;
  return notif as GameroomNotification;
}
