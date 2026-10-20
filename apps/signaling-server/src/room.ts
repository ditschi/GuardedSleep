import type { WebSocket } from "ws";
import type { UnitRole } from "@guardedsleep/protocol";

export type RoomPeer = {
  id: string;
  role: UnitRole;
  socket: WebSocket;
};

export type AddPeerResult = "ok" | "baby_taken" | "parent_cap";

export class SignalingRoom {
  readonly roomId: string;
  /** Protocol version of the first member; reset when the room empties. */
  protocolVersion: number | undefined;
  private readonly peers = new Map<string, RoomPeer>();
  private readonly maxParents: number;

  constructor(roomId: string, maxParents: number) {
    this.roomId = roomId;
    this.maxParents = maxParents;
  }

  add(peer: RoomPeer): AddPeerResult {
    if (peer.role === "baby") {
      for (const p of this.peers.values()) {
        if (p.role === "baby") return "baby_taken";
      }
    } else {
      let parents = 0;
      for (const p of this.peers.values()) {
        if (p.role === "parent") parents += 1;
      }
      if (parents >= this.maxParents) return "parent_cap";
    }
    this.peers.set(peer.id, peer);
    return "ok";
  }

  remove(clientId: string): void {
    this.peers.delete(clientId);
    if (this.peers.size === 0) this.protocolVersion = undefined;
  }

  /** Other peer when exactly one counterpart exists (legacy 1:1 hint). */
  getPeer(exceptId: string): RoomPeer | undefined {
    for (const [id, p] of this.peers) {
      if (id !== exceptId) return p;
    }
    return undefined;
  }

  getPeerById(clientId: string): RoomPeer | undefined {
    return this.peers.get(clientId);
  }

  has(id: string): boolean {
    return this.peers.has(id);
  }

  allPeers(): RoomPeer[] {
    return [...this.peers.values()];
  }

  listMembers(): { clientId: string; role: UnitRole }[] {
    return [...this.peers.values()].map((p) => ({ clientId: p.id, role: p.role }));
  }

  get size(): number {
    return this.peers.size;
  }
}

export class RoomRegistry {
  private readonly rooms = new Map<string, SignalingRoom>();
  private readonly maxParents: number;

  constructor(maxParents: number) {
    this.maxParents = maxParents;
  }

  peek(roomId: string): SignalingRoom | undefined {
    return this.rooms.get(roomId);
  }

  getOrCreate(roomId: string): SignalingRoom {
    let r = this.rooms.get(roomId);
    if (!r) {
      r = new SignalingRoom(roomId, this.maxParents);
      this.rooms.set(roomId, r);
    }
    return r;
  }

  deleteIfEmpty(roomId: string): void {
    const r = this.rooms.get(roomId);
    if (r && r.size === 0) this.rooms.delete(roomId);
  }
}
