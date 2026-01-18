//! Server state management for the WASM signaling server
//!
//! This module manages connected peers, rooms, and message routing.
//! State is persisted to a JSON file between requests.

use crate::error::SignalingError;
use matchbox_protocol::{JsonPeerEvent, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Path to the state file
const STATE_FILE: &str = "matchbox_state.json";

/// Room identifier
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub String);

/// Peer state with pending events
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct PeerState {
    room: RoomId,
    /// Pending events to be delivered to this peer
    events: VecDeque<String>,
}

/// Serializable state
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct InnerState {
    /// Map of peer ID -> peer state
    peers: HashMap<PeerId, PeerState>,
    /// Map of room -> peers in that room
    rooms: HashMap<RoomId, HashSet<PeerId>>,
}

impl InnerState {
    /// Load state from file, or create default if file doesn't exist
    fn load() -> Self {
        match std::fs::read_to_string(STATE_FILE) {
            Ok(contents) => {
                serde_json::from_str(&contents).unwrap_or_default()
            }
            Err(_) => Self::default(),
        }
    }

    /// Save state to file
    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(STATE_FILE, json);
        }
    }
}

/// The main server state - loads/saves to file
#[derive(Default, Clone)]
pub struct ServerState;

impl ServerState {
    /// Create a new server state handle
    pub fn new() -> Self {
        Self
    }

    /// Join a room or poll for events
    /// 
    /// If peer_id is None, creates a new peer and joins the room.
    /// Returns (peer_id, pending_events)
    pub fn join_or_poll(&self, room_id: RoomId, peer_id: Option<PeerId>) -> (PeerId, Vec<String>) {
        let mut state = InnerState::load();
        
        let result = match peer_id {
            Some(id) => {
                // Existing peer - poll for events
                if let Some(peer_state) = state.peers.get_mut(&id) {
                    let events: Vec<String> = peer_state.events.drain(..).collect();
                    (id, events)
                } else {
                    // Peer not found - create new one
                    self.join_room_inner(&mut state, room_id)
                }
            }
            None => {
                // New peer - join room
                self.join_room_inner(&mut state, room_id)
            }
        };

        state.save();
        result
    }

    /// Join a room as a new peer (internal helper)
    fn join_room_inner(&self, state: &mut InnerState, room_id: RoomId) -> (PeerId, Vec<String>) {
        let peer_id: PeerId = uuid::Uuid::new_v4().into();
        
        // Get existing peers in the room before adding new peer
        let existing_peers: Vec<PeerId> = state
            .rooms
            .get(&room_id)
            .map(|peers| peers.iter().cloned().collect())
            .unwrap_or_default();

        // Create peer state with initial events
        let mut peer_state = PeerState {
            room: room_id.clone(),
            events: VecDeque::new(),
        };

        // Queue IdAssigned event
        let id_event = JsonPeerEvent::IdAssigned(peer_id).to_string();
        peer_state.events.push_back(id_event);

        // Queue NewPeer events for all existing peers
        for existing_id in &existing_peers {
            let new_peer_event = JsonPeerEvent::NewPeer(*existing_id).to_string();
            peer_state.events.push_back(new_peer_event);
        }

        // Add peer to state
        state.peers.insert(peer_id, peer_state);

        // Add peer to room
        state.rooms.entry(room_id.clone()).or_default().insert(peer_id);

        // Notify existing peers about the new peer
        let new_peer_event = JsonPeerEvent::NewPeer(peer_id).to_string();
        for existing_id in &existing_peers {
            if let Some(existing_peer) = state.peers.get_mut(existing_id) {
                existing_peer.events.push_back(new_peer_event.clone());
            }
        }

        // Return peer ID and initial events
        let events: Vec<String> = state
            .peers
            .get_mut(&peer_id)
            .map(|p| p.events.drain(..).collect())
            .unwrap_or_default();

        (peer_id, events)
    }

    /// Queue an event for a peer
    pub fn queue_event(&self, peer_id: PeerId, event: String) -> Result<(), SignalingError> {
        let mut state = InnerState::load();
        
        let result = if let Some(peer_state) = state.peers.get_mut(&peer_id) {
            peer_state.events.push_back(event);
            Ok(())
        } else {
            Err(SignalingError::UnknownPeer)
        };

        state.save();
        result
    }

    /// Remove a peer from the server
    pub fn remove_peer(&self, peer_id: &PeerId) {
        let mut state = InnerState::load();
        
        if let Some(peer_state) = state.peers.remove(peer_id) {
            // Remove from room and collect other peer IDs
            let other_peer_ids: Vec<PeerId> = if let Some(room_peers) = state.rooms.get_mut(&peer_state.room) {
                room_peers.remove(peer_id);
                room_peers.iter().cloned().collect()
            } else {
                Vec::new()
            };
            
            // Notify other peers in room about disconnect
            let peer_left_event = JsonPeerEvent::PeerLeft(*peer_id).to_string();
            for other_id in other_peer_ids {
                if let Some(other_peer) = state.peers.get_mut(&other_id) {
                    other_peer.events.push_back(peer_left_event.clone());
                }
            }
        }

        state.save();
    }

    /// Get all peers in a room
    pub fn get_room_peers(&self, room_id: &RoomId) -> Vec<PeerId> {
        let state = InnerState::load();
        state
            .rooms
            .get(room_id)
            .map(|peers| peers.iter().cloned().collect())
            .unwrap_or_default()
    }
}
