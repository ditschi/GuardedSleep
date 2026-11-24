//! A simulated peer connection.

use super::*;

pub struct Pc {
    pub fp: String,
    /// The certificate fingerprint DTLS authenticates the remote against: what the real
    /// `a=fingerprint` line of the applied remote SDP says. The platform reports it to the core.
    pub dtls_fp: Option<String>,
    pub local: bool,
    pub remote: bool,
    pub peer: Option<(usize, u32)>,
    pub closed: bool,
    pub connected: bool,
    pub failed: bool,
    pub connecting: bool,
    pub epoch: u32,
    pub unhealthy_since: Option<Time>,
    pub fail_after: u64,
}

impl Pc {
    pub(super) fn fresh(fp: String, epoch: u32, fail_after: u64) -> Self {
        Pc {
            fp,
            dtls_fp: None,
            local: false,
            remote: false,
            peer: None,
            closed: false,
            connected: false,
            failed: false,
            connecting: false,
            epoch,
            unhealthy_since: None,
            fail_after,
        }
    }

    /// Neither closed nor failed.
    pub(super) fn is_live(&self) -> bool {
        !self.closed && !self.failed
    }

    /// Connected and still live.
    pub(super) fn is_up(&self) -> bool {
        self.connected && self.is_live()
    }

    /// Both descriptions are applied and no connect is under way.
    pub(super) fn ready_to_connect(&self) -> bool {
        self.local && self.remote && self.is_live() && !self.connecting && !self.connected
    }

    pub(super) fn peer_ref(&self) -> Option<PcRef> {
        self.peer.map(PcRef::from)
    }
}
